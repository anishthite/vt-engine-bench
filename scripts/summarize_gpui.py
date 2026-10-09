#!/usr/bin/env python3
"""Validate window-capture samples; never turn skipped frames into fake latency."""
import csv
import statistics
import sys
from pathlib import Path


def percentile(values, percent):
    values = sorted(values)
    return values[min(len(values) - 1, (len(values) * percent + 99) // 100 - 1)]


def run(events_file, capture_file):
    events = list(csv.DictReader(open(events_file)))
    captures = list(csv.DictReader(open(capture_file)))
    assert len(events) == 120, f"expected 120 updates, got {len(events)}"
    expected = [("tmux" if i < 80 else "input", i if i < 80 else i - 80) for i in range(120)]
    assert [(e["phase"], int(e["index"])) for e in events] == expected, "event order mismatch"
    sequences = [int(c["sequence"]) for c in captures]
    assert sequences and sequences == sorted(set(sequences)) and sequences[0] == 1 and sequences[-1] == 120, "missing endpoints, duplicate, or out-of-order capture"
    times = [float(c["observed_epoch_ms"]) for c in captures]
    assert times == sorted(times), "capture timestamps out of order"
    observed = {s: t for s, t in zip(sequences, times)}
    output = []
    for phase, start, end, limit in (("tmux", 1, 80, 70), ("input", 81, 120, 36)):
        samples = [(s, observed[s] - float(events[s - 1]["sent_epoch_ms"])) for s in range(start, end + 1) if s in observed]
        assert len(samples) >= limit, f"{phase}: only {len(samples)}/{end - start + 1} frames reached capture; no valid benchmark"
        assert all(0 <= latency < 250 for _, latency in samples), f"{phase}: invalid capture latency; event/capture clocks or indexing disagree"
        intervals = [observed[s] - observed[s - 1] for s in range(start + 1, end + 1) if s in observed and s - 1 in observed]
        assert intervals, f"{phase}: no consecutive frames"
        output.append((phase, len(samples), statistics.median(v for _, v in samples), percentile([v for _, v in samples], 95), statistics.median(intervals), percentile(intervals, 95)))
    return output


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--aggregate":
        root = Path(sys.argv[2])
        writer = csv.writer(sys.stdout, lineterminator="\n")
        writer.writerow(["engine", "run", "phase", "captured", "expected", "event_to_pixel_median_ms", "event_to_pixel_p95_ms", "capture_interval_median_ms", "capture_interval_p95_ms"])
        for engine in ("alacritty", "ghostty", "vt100", "wezterm"):
            for number in (1, 2, 3):
                prefix = root / f"{engine}-{number}"
                try:
                    for phase, count, median, p95, pacing, pacing_p95 in run(Path(f"{prefix}-events.csv"), Path(f"{prefix}-captures.csv")):
                        writer.writerow([engine, number, phase, count, 80 if phase == "tmux" else 40, f"{median:.2f}", f"{p95:.2f}", f"{pacing:.2f}", f"{pacing_p95:.2f}"])
                except (AssertionError, OSError) as error:
                    sys.exit(f"INVALID capture ({prefix}): {error}")
    else:
        events, captures = map(Path, sys.argv[1:3])
        try:
            for phase, count, median, p95, pacing, pacing_p95 in run(events, captures):
                print(f"{phase}: captured {count}/{'80' if phase == 'tmux' else '40'}; event→pixel {median:.2f}ms median/{p95:.2f}ms p95; capture intervals {pacing:.2f}ms median/{pacing_p95:.2f}ms p95")
        except (AssertionError, OSError) as error:
            sys.exit(f"INVALID capture ({events.parent}): {error}")
