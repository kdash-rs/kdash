#!/usr/bin/env python3
"""Speed up sections of an asciinema v2 cast by rescaling time per region.

The harness records in real time, so repetitive stretches (scrolling a list,
cycling tabs/themes) play as slowly as the payoff moments. Unlike a single
global --speed, this applies different factors to time ranges of the recording,
so you can fast-forward the filler while leaving the interesting beats at pace.

  python3 scripts/uat/speed_cast.py in.cast out.cast \
      --seg 0:3 --seg 9.5:2 --seg 19:1 --seg 29.7:2 --seg 46.7:3 --seg 61.3:2

--seg T:F   from raw timestamp T (seconds, in the SOURCE cast) until the next
            --seg, divide inter-event deltas by F (>1 = faster). Repeatable;
            time before the first --seg plays at 1x.
--max-gap S clamp any single gap to at most S seconds BEFORE scaling, to kill
            dead air (default: no clamp).

Find the T boundaries by searching the cast for the text the app paints when a
section starts (e.g. "Containers", "Cluster Summary", a theme name). The header
is copied verbatim; event timestamps are rewritten monotonically from 0.
"""
import bisect
import json
import sys


def main():
    args = sys.argv[1:]

    def take(name, default):
        if name in args:
            i = args.index(name)
            val = args[i + 1]
            del args[i : i + 2]
            return val
        return default

    def take_all(name):
        vals = []
        while name in args:
            i = args.index(name)
            vals.append(args[i + 1])
            del args[i : i + 2]
        return vals

    max_gap = float(take("--max-gap", "inf"))
    segs = sorted(
        (float(t), float(f)) for t, f in (s.split(":") for s in take_all("--seg"))
    )
    src, dst = args[0], args[1]

    starts = [t for t, _ in segs]
    factors = [f for _, f in segs]

    def factor_at(t):
        # 1x before the first segment; otherwise the factor of the region the
        # gap started in.
        if not segs or t < starts[0]:
            return 1.0
        return factors[bisect.bisect_right(starts, t) - 1]

    lines = open(src).read().splitlines()
    header = lines[0]
    events = [json.loads(l) for l in lines[1:] if l.strip()]

    out = [header]
    prev_raw = 0.0
    clock = 0.0
    for ev in events:
        t, kind, data = ev[0], ev[1], ev[2]
        gap = min(t - prev_raw, max_gap)
        clock += max(gap, 0.0) / factor_at(prev_raw)
        prev_raw = t
        out.append(json.dumps([round(clock, 6), kind, data]))

    with open(dst, "w") as f:
        f.write("\n".join(out) + "\n")

    print(f"{src} ({events[-1][0]:.1f}s) -> {dst} ({clock:.1f}s), {len(events)} events")
    for t, fac in segs:
        print(f"  from {t:6.2f}s  x{fac:g}")


if __name__ == "__main__":
    main()
