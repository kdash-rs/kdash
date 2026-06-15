# KDash TUI test harness

A small, reusable driver for end-to-end / UAT testing of the KDash terminal UI.
It launches `kdash` inside a pseudo-terminal, renders the live screen with a
terminal emulator ([pyte]), drives it with scripted keystrokes, and asserts on
the rendered text. Useful for manual UAT runs and as a base for automated TUI
smoke tests.

## Why this exists

KDash is a full-screen `ratatui`/`crossterm` app, so you can't assert on its
output by piping stdout. This harness gives a real PTY, answers crossterm's
cursor-position query (`ESC[6n`) so the app doesn't abort, and reconstructs the
on-screen grid so steps can `expect:`/`refute:` substrings and save screenshots.

## Requirements

Python 3.9+ with `pyte` and `pexpect`. A throwaway venv keeps it off the system
Python:

```bash
python3 -m venv /tmp/kdash-uat-venv
/tmp/kdash-uat-venv/bin/pip install pyte pexpect
```

## Usage

```bash
# build first
cargo build

# run a program file against the debug binary
/tmp/kdash-uat-venv/bin/python scripts/uat/harness.py <program> <outdir> [binary] [args...]

# example
/tmp/kdash-uat-venv/bin/python scripts/uat/harness.py \
    scripts/uat/example.prog /tmp/out target/debug/kdash -t 200
```

- `program` — a step file (see below).
- `outdir` — where `snap:` writes `<label>.txt` screenshots.
- `binary` — defaults to `target/debug/kdash`.
- `args...` — extra CLI args (default `-t 200`).

Exit code is non-zero if any `expect`/`refute` failed, so it slots into CI.

## Recording an asciinema cast

Add `--cast <path>` anywhere in the args to also record the whole driven
session as an [asciinema] v2 cast. It tees the raw PTY bytes the harness already
reads, so the recording is exactly what the app painted, driven by your scripted
keystrokes (deterministic, not a hand-recorded session).

```bash
/tmp/kdash-uat-venv/bin/python scripts/uat/harness.py \
    scripts/uat/example.prog /tmp/out target/debug/kdash --cast /tmp/out/demo.cast -t 200

asciinema play /tmp/out/demo.cast      # replay in the terminal
agg /tmp/out/demo.cast /tmp/out/demo.gif   # render a GIF (e.g. for the README)
```

`--cast` works alongside `expect`/`refute`/`snap` — one run both asserts and
records.

### A tight, loopable README GIF

By default the cast covers the whole session (incl. the slow load and the final
quit). For a clean clip, drive a smaller terminal with `--cols/--rows` and
bracket the interesting part with `startcast`/`stopcast` steps:

- `startcast` drops everything captured so far and re-bases the clock to now. It
  also nudges the window size to force a full repaint, because ratatui only
  redraws changed cells — without that the clip would open on a blank grid.
- `stopcast` finalizes the recording, so the quit at the end is excluded.

`scripts/uat/demo.prog` is a ready-made tour (Pods, a few resource tabs, the
help overlay, back to Pods so it loops). It produced `screenshots/demo.gif`:

```bash
/tmp/kdash-uat-venv/bin/python scripts/uat/harness.py \
    scripts/uat/demo.prog /tmp/out target/debug/kdash \
    --cast /tmp/out/kdash.cast --cols 128 --rows 34 -t 200

agg --font-size 16 /tmp/out/kdash.cast screenshots/demo.gif
```

[asciinema]: https://asciinema.org/

## Program steps

One step per line; blank lines and `#` comments are ignored.

| Step | Effect |
|------|--------|
| `spawn:<args>` | (Re)spawn kdash with extra CLI args |
| `key:<name>` | Send named key(s), space-separated (see below) |
| `type:<text>` | Type literal characters |
| `wait:<seconds>` | Sleep while pumping PTY output into the screen |
| `settle` | Wait the default settle interval |
| `snap:<label>` | Save the current screen to `<outdir>/<label>.txt` |
| `expect:<substr>` | Assert the screen contains `substr` (PASS/FAIL) |
| `refute:<substr>` | Assert the screen does not contain `substr` |
| `iexpect:<substr>` | Case-insensitive `expect` |
| `comment:<text>` | Print a comment line |
| `startcast` | (Re)start the `--cast` clip here, dropping earlier frames |
| `stopcast` | Finalize the `--cast` clip here, excluding later frames |

### Key names

`enter esc tab backtab space up down left right home end pageup pagedown`
`backspace ctrl-c ctrl-d ctrl-h ctrl-r alt-t`

Plain characters (letters, digits, `?`, `/`, `-`) are sent with `type:`.
Shift+letter is just the uppercase letter, e.g. `type:U` for `Shift+u`.

## Example program

```
comment: open help, confirm two-column layout, then quit
wait: 3
type:?
settle
snap: help
expect: General
expect: Resource Views
type:?
```
