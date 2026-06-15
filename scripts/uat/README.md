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

Two ready-made programs:

- `scripts/uat/demo.prog` — minimal loopable tour (Pods, a few resource tabs,
  the help overlay, back to Pods). Good as a small example of the clip controls.
- `scripts/uat/ui.prog` — the full hero tour behind `screenshots/ui.gif`: drill
  into a pod's container logs, cycle every resource tab, show the action menu /
  port-forward / delete confirm, the Utilization gauges, theme cycling, and the
  help page.

The hero GIF needs the gauge-demo workloads so the Utilization/Context gauges
show colour, and it records its cast alongside the GIF:

```bash
kubectl apply -f test_data/gauge-demo.yaml      # green/amber/red gauges

/tmp/kdash-uat-venv/bin/python scripts/uat/harness.py \
    scripts/uat/ui.prog /tmp/out target/debug/kdash \
    --cast /tmp/ui_raw.cast --cols 150 --rows 30 -t 200

# Fast-forward the filler (scrolling, cycling) while leaving the payoff beats at
# pace. The --seg boundaries are raw timestamps from the recording above; find
# them by searching the cast for the text the app paints when a section starts.
python3 scripts/uat/speed_cast.py /tmp/ui_raw.cast screenshots/ui.cast \
    --seg 0:3 --seg 9.47:2 --seg 19.08:1 --seg 29.70:2 --seg 46.74:3 \
    --seg 55.14:1 --seg 61.35:2

agg --font-size 14 screenshots/ui.cast screenshots/ui.gif

kubectl delete -f test_data/gauge-demo.yaml     # clean up
```

The smaller `demo.prog` uses `--cols 128 --rows 34` and `agg --font-size 16`.

## Regenerating the still screenshots

The README's PNG screenshots (`screenshots/overview.png`, `logs.png`,
`describe.png`, `utilization.png`, `contexts.png`) are pulled from one cast so
they all share the same theme and geometry. `screens.prog` visits each view in
turn; `cast_frame.py` renders a single frame at a timestamp by truncating the
cast to that point, letting `agg` rebuild the screen, and grabbing the last
frame with `ffmpeg` (so it needs `agg` and `ffmpeg` on PATH).

```bash
kubectl apply -f test_data/gauge-demo.yaml      # populate kdash-test

/tmp/kdash-uat-venv/bin/python scripts/uat/harness.py \
    scripts/uat/screens.prog /tmp/sout target/debug/kdash \
    --cast /tmp/screens.cast --cols 150 --rows 30 -t 200

# Pick a settled timestamp per view (find them by searching the cast for the
# text the app paints when a view appears, e.g. "Cluster Summary").
python3 scripts/uat/cast_frame.py /tmp/screens.cast screenshots/overview.png    --at 1.5
python3 scripts/uat/cast_frame.py /tmp/screens.cast screenshots/logs.png        --at 11.0
python3 scripts/uat/cast_frame.py /tmp/screens.cast screenshots/describe.png    --at 20.0
python3 scripts/uat/cast_frame.py /tmp/screens.cast screenshots/utilization.png --at 27.5
python3 scripts/uat/cast_frame.py /tmp/screens.cast screenshots/contexts.png    --at 32.0

kubectl delete -f test_data/gauge-demo.yaml     # clean up
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
