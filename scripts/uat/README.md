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
