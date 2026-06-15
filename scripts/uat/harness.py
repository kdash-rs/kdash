#!/usr/bin/env python3
"""KDash UAT harness: drive the TUI in a pty, render with pyte, assert on screen text.

Reads a "program" (one step per line) from a file argument. Steps:
  spawn:<args>        (re)spawn kdash with extra CLI args (space separated); optional
  key:<name>          send a named key (see KEYS)
  type:<text>         type literal characters
  wait:<seconds>      sleep, pumping pty output into the screen
  settle              wait the default settle interval
  snap:<label>        render current screen to <outdir>/<label>.txt and echo a header
  expect:<substr>     assert current screen CONTAINS substr  -> PASS/FAIL line
  refute:<substr>     assert current screen does NOT contain substr -> PASS/FAIL line
  iexpect:<substr>    case-insensitive contains
  comment:<text>      print a comment line
  startcast           (re)start the cast clip here, dropping earlier frames
  stopcast            finalize the cast here, excluding later frames

Exit code is non-zero if any expect/refute failed.

Pass --cast <path> to also record the session as an asciinema v2 cast (replay
with `asciinema play <path>`, turn into a GIF with `agg <path> out.gif`).
Use --cols/--rows to drive a smaller terminal (handy for a tight README GIF);
bracket the interesting part with startcast/stopcast to skip load and quit.
"""
import json
import os
import sys
import time
import pexpect
import pyte

COLS, ROWS = 230, 56
DEFAULT_SETTLE = 1.2

KEYS = {
    "enter": "\r",
    "esc": "\x1b",
    "tab": "\t",
    "backtab": "\x1b[Z",
    "space": " ",
    "up": "\x1b[A",
    "down": "\x1b[B",
    "right": "\x1b[C",
    "left": "\x1b[D",
    "home": "\x1b[H",
    "end": "\x1b[F",
    "pageup": "\x1b[5~",
    "pagedown": "\x1b[6~",
    "backspace": "\x7f",
    "ctrl-c": "\x03",
    "ctrl-d": "\x04",
    "ctrl-h": "\x08",
    "ctrl-r": "\x12",
    "alt-t": "\x1bt",
}


class Session:
    def __init__(self, outdir, binary, base_args, cast_path=None, cols=COLS, rows=ROWS):
        self.outdir = outdir
        self.binary = binary
        self.base_args = base_args
        self.cols = cols
        self.rows = rows
        self.child = None
        self.screen = pyte.Screen(self.cols, self.rows)
        self.stream = pyte.Stream(self.screen)
        self.failures = 0
        self.passes = 0
        self.cast = None
        self.cast_start = None
        if cast_path:
            self.cast = open(cast_path, "w")
            self._write_cast_header()

    def _write_cast_header(self):
        header = {
            "version": 2,
            "width": self.cols,
            "height": self.rows,
            "timestamp": int(time.time()),
            "env": {"TERM": "xterm-256color", "SHELL": os.environ.get("SHELL", "")},
        }
        self.cast.write(json.dumps(header) + "\n")
        self.cast.flush()
        self.cast_start = time.time()

    def start_cast(self):
        """Mark the start of the recorded clip: drop everything captured so far
        (e.g. the slow initial load) and re-base timestamps to now.

        ratatui only repaints changed cells, so a truncated clip would start from
        a blank grid. Nudge the window size to force a full clear+redraw, then
        re-base, so the clip opens with a complete frame."""
        if not self.cast:
            return
        if self.child and self.child.isalive() and self.rows > 2:
            self.child.setwinsize(self.rows - 1, self.cols)
            self._pump(0.3)
            self.child.setwinsize(self.rows, self.cols)
        self.cast.seek(0)
        self.cast.truncate()
        self._write_cast_header()
        self._pump(0.4)

    def stop_cast(self):
        """Finalize the recording here, so later bytes (e.g. the quit) are
        excluded — keeps a GIF clip tight and loopable."""
        if self.cast:
            self.cast.close()
            self.cast = None

    def _cast_write(self, data):
        if not self.cast:
            return
        elapsed = round(time.time() - self.cast_start, 6)
        self.cast.write(json.dumps([elapsed, "o", data]) + "\n")
        self.cast.flush()

    def spawn(self, extra=""):
        if self.child and self.child.isalive():
            self.child.sendline("")
            self.child.close(force=True)
        args = self.base_args + ([] if not extra else extra.split())
        env = dict(os.environ)
        env["TERM"] = "xterm-256color"
        self.child = pexpect.spawn(
            self.binary, args, dimensions=(self.rows, self.cols), env=env,
            encoding="utf-8", timeout=5
        )
        self.screen = pyte.Screen(self.cols, self.rows)
        self.stream = pyte.Stream(self.screen)
        self._pump(2.5)

    def _pump(self, seconds):
        end = time.time() + seconds
        while time.time() < end:
            try:
                data = self.child.read_nonblocking(size=65536, timeout=0.2)
                if data:
                    self.stream.feed(data)
                    self._cast_write(data)
                    # Answer crossterm's cursor-position query (DSR: ESC[6n) so
                    # the TUI doesn't abort with "cursor position could not be read".
                    if "\x1b[6n" in data:
                        cy = self.screen.cursor.y + 1
                        cx = self.screen.cursor.x + 1
                        self.child.send(f"\x1b[{cy};{cx}R")
            except pexpect.TIMEOUT:
                pass
            except pexpect.EOF:
                break

    def send(self, keys):
        self.child.send(keys)

    def render(self):
        return "\n".join(self.screen.display)

    def snap(self, label):
        text = self.render()
        path = os.path.join(self.outdir, label + ".txt")
        with open(path, "w") as f:
            f.write(text)
        print(f"  [snap] {label} -> {path}")

    def expect(self, substr, negate=False, ci=False):
        text = self.render()
        hay = text.lower() if ci else text
        needle = substr.lower() if ci else substr
        found = needle in hay
        ok = (not found) if negate else found
        if ok:
            self.passes += 1
            print(f"  PASS {'refute' if negate else 'expect'}: {substr!r}")
        else:
            self.failures += 1
            print(f"  FAIL {'refute' if negate else 'expect'}: {substr!r}")

    def run(self, program):
        for raw in program:
            line = raw.rstrip("\n")
            if not line.strip() or line.lstrip().startswith("#"):
                continue
            if ":" in line:
                op, arg = line.split(":", 1)
            else:
                op, arg = line, ""
            op = op.strip()
            if op == "spawn":
                self.spawn(arg.strip())
            elif op == "key":
                for k in arg.split():
                    self.send(KEYS[k])
                    self._pump(0.25)
            elif op == "type":
                self.send(arg)
            elif op == "wait":
                self._pump(float(arg))
            elif op == "settle":
                self._pump(DEFAULT_SETTLE)
            elif op == "snap":
                self._pump(0.3)
                self.snap(arg.strip())
            elif op == "expect":
                self.expect(arg.strip())
            elif op == "refute":
                self.expect(arg.strip(), negate=True)
            elif op == "iexpect":
                self.expect(arg.strip(), ci=True)
            elif op == "comment":
                print(f"# {arg.strip()}")
            elif op == "startcast":
                self.start_cast()
            elif op == "stopcast":
                self.stop_cast()
            else:
                print(f"  ?? unknown op: {op}")

    def close(self):
        if self.child and self.child.isalive():
            self.child.send(KEYS["ctrl-c"])
            self._pump(0.5)
            self.child.close(force=True)
        self.stop_cast()


def _take_opt(argv, name):
    """Pop `--name <value>` from argv (anywhere), returning the value or None.
    Keeps the positional [args...] passthrough (e.g. -t 200) intact."""
    if name in argv:
        i = argv.index(name)
        value = argv[i + 1]
        del argv[i : i + 2]
        return value
    return None


def main():
    argv = sys.argv[1:]
    cast_path = _take_opt(argv, "--cast")
    cols = int(_take_opt(argv, "--cols") or COLS)
    rows = int(_take_opt(argv, "--rows") or ROWS)
    prog_file = argv[0]
    outdir = argv[1]
    binary = argv[2] if len(argv) > 2 else "target/debug/kdash"
    base_args = argv[3:] if len(argv) > 3 else ["-t", "200"]
    os.makedirs(outdir, exist_ok=True)
    with open(prog_file) as f:
        program = f.readlines()
    s = Session(outdir, binary, base_args, cast_path=cast_path, cols=cols, rows=rows)
    # first line may be a spawn; if not, spawn default
    if not any(l.strip().startswith("spawn") for l in program[:1]):
        s.spawn()
    try:
        s.run(program)
    finally:
        s.close()
    print(f"\n== RESULT: {s.passes} passed, {s.failures} failed ==")
    sys.exit(1 if s.failures else 0)


if __name__ == "__main__":
    main()
