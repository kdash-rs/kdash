#!/usr/bin/env python3
"""Render a single still PNG from an asciinema v2 cast at a given timestamp.

The screen at time T is the result of replaying every output event from 0..T
(ratatui paints incrementally), so we truncate the cast to T, let `agg` rebuild
the frames into a gif, and pull the last frame out with ffmpeg.

  python3 scripts/uat/cast_frame.py in.cast out.png --at 47.0 [--font-size 14]

Requires `agg` and `ffmpeg` on PATH.
"""
import json
import subprocess
import sys
import tempfile


def main():
    args = sys.argv[1:]

    def take(name, default):
        if name in args:
            i = args.index(name)
            val = args[i + 1]
            del args[i : i + 2]
            return val
        return default

    at = float(take("--at", "0"))
    font_size = take("--font-size", "14")
    src, out_png = args[0], args[1]

    lines = open(src).read().splitlines()
    header = lines[0]
    events = [json.loads(l) for l in lines[1:] if l.strip()]
    kept = [e for e in events if e[0] <= at]
    if not kept:
        sys.exit(f"no events at or before {at}s (cast starts at {events[0][0]}s)")
    # Hold the final frame for a beat so agg emits it as a real frame.
    last = kept[-1]
    kept.append([round(last[0] + 1.0, 6), "o", ""])

    with tempfile.NamedTemporaryFile("w", suffix=".cast", delete=False) as tf:
        tf.write(header + "\n")
        for e in kept:
            tf.write(json.dumps(e) + "\n")
        cast_path = tf.name
    gif_path = cast_path[:-5] + ".gif"

    subprocess.run(
        ["agg", "--font-size", font_size, cast_path, gif_path],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    # Last gif frame = screen state at T. `reverse` then take one frame.
    subprocess.run(
        ["ffmpeg", "-y", "-i", gif_path, "-vf", "reverse", "-frames:v", "1", out_png],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    print(f"{src} @ {at}s -> {out_png}")


if __name__ == "__main__":
    main()
