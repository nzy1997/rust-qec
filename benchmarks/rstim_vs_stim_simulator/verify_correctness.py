from __future__ import annotations

import math
import random
from collections.abc import Sequence


STATUS_PASS = "pass"
STATUS_MISMATCH = "statistical_mismatch"
STATUS_STIM_FAILED = "stim_failed"
STATUS_RSTIM_FAILED = "rstim_failed"
STATUS_SKIPPED = "skipped"


def parse_01_samples(stdout: str, *, expected_bits: int, expected_shots: int) -> list[list[int]]:
    lines = [line.strip() for line in stdout.splitlines() if line.strip()]
    if len(lines) != expected_shots:
        raise ValueError(f"expected {expected_shots} shots, got {len(lines)}")

    samples: list[list[int]] = []
    for shot_index, line in enumerate(lines):
        if len(line) != expected_bits:
            raise ValueError(f"shot {shot_index}: expected {expected_bits} bits, got {len(line)}")
        if any(ch not in "01" for ch in line):
            raise ValueError(f"shot {shot_index}: output contains non-01 data")
        samples.append([1 if ch == "1" else 0 for ch in line])
    return samples


def inject_bitflip(samples: list[list[int]], *, rate: float, seed: int) -> list[list[int]]:
    if not 0.0 <= rate <= 1.0:
        raise ValueError("rate must be between 0 and 1")

    rng = random.Random(seed)
    mutated: list[list[int]] = [row.copy() for row in samples]
    for row in mutated:
        for index, bit in enumerate(row):
            if rng.random() < rate:
                row[index] = 1 - bit
    return mutated


def select_columns(bit_count: int, *, observable_count: int, limit: int = 16) -> list[int]:
    if bit_count <= 0:
        return []
    if observable_count < 0:
        raise ValueError("observable_count must be non-negative")
    if limit <= 0:
        return []

    observable_start = max(0, bit_count - observable_count)
    selected: set[int] = set()

    def add(index: int) -> None:
        if 0 <= index < bit_count:
            selected.add(index)

    add(0)
    for index in range(observable_start, bit_count):
        add(index)

    middle_stop = observable_start if observable_count else bit_count
    middle_count = max(0, limit - len(selected))
    if middle_count:
        span = max(0, middle_stop - 1)
        if span > 0:
            for step in range(1, middle_count + 1):
                index = round(step * span / (middle_count + 1))
                add(index)
        else:
            add(0)

    if len(selected) < limit:
        for index in range(bit_count):
            add(index)
            if len(selected) >= limit:
                break

    return sorted(selected)[:limit]


def select_pairs(
    columns: list[int], *, bit_count: int, observable_count: int, limit: int = 16
) -> list[tuple[int, int]]:
    if limit <= 0 or bit_count <= 1 or len(columns) < 2:
        return []
    if observable_count < 0:
        raise ValueError("observable_count must be non-negative")

    selected = sorted({index for index in columns if 0 <= index < bit_count})
    observable_start = max(0, bit_count - observable_count)
    pairs: list[tuple[int, int]] = []
    seen: set[tuple[int, int]] = set()

    def add(left: int, right: int) -> None:
        if left == right:
            return
        pair = (left, right) if left < right else (right, left)
        if pair in seen:
            return
        seen.add(pair)
        pairs.append(pair)

    for left, right in zip(selected, selected[1:]):
        add(left, right)
        if len(pairs) >= limit:
            return pairs[:limit]

    first_detector = next((index for index in selected if index < observable_start), None)
    if first_detector is not None:
        for observable in selected:
            if observable >= observable_start:
                add(first_detector, observable)
                if len(pairs) >= limit:
                    return pairs[:limit]

    return pairs[:limit]


def _validate_rectangular(samples: Sequence[Sequence[int]], label: str) -> int:
    if not samples:
        return 0
    width = len(samples[0])
    for row_index, row in enumerate(samples):
        if len(row) != width:
            raise ValueError(f"{label} row {row_index} has width {len(row)}; expected {width}")
    return width


def _bit_rate(samples: Sequence[Sequence[int]], column: int) -> tuple[int, int, float]:
    hits = sum(1 for row in samples if row[column])
    total = len(samples)
    return hits, total, (hits / total if total else 0.0)


def _pair_rate(samples: Sequence[Sequence[int]], left: int, right: int) -> tuple[int, int, float]:
    hits = sum(1 for row in samples if row[left] and row[right])
    total = len(samples)
    return hits, total, (hits / total if total else 0.0)


def _tolerance(
    stim_hits: int,
    stim_total: int,
    rstim_hits: int,
    rstim_total: int,
    *,
    z_score: float,
    floor: float,
) -> float:
    if stim_total <= 0 or rstim_total <= 0:
        return float("inf")
    pooled = (stim_hits + rstim_hits) / (stim_total + rstim_total)
    variance = pooled * (1 - pooled) * (1 / stim_total + 1 / rstim_total)
    return min(z_score * math.sqrt(max(0.0, variance)) + floor, 1.0 - floor)


def compare_sample_sets(
    stim_samples: list[list[int]],
    rstim_samples: list[list[int]],
    *,
    columns: list[int],
    pairs: list[tuple[int, int]],
    z_score: float = 6.0,
    floor: float = 0.01,
) -> dict[str, object]:
    stim_width = _validate_rectangular(stim_samples, "stim_samples")
    rstim_width = _validate_rectangular(rstim_samples, "rstim_samples")
    failure_reasons: list[str] = []

    if stim_samples and rstim_samples and stim_width != rstim_width:
        failure_reasons.append(
            f"sample widths differ: stim={stim_width}, rstim={rstim_width}"
        )

    sample_count = min(len(stim_samples), len(rstim_samples))
    if len(stim_samples) != len(rstim_samples):
        failure_reasons.append(
            f"sample counts differ: stim={len(stim_samples)}, rstim={len(rstim_samples)}"
        )

    marginals: list[dict[str, object]] = []
    pair_stats: list[dict[str, object]] = []
    max_delta = 0.0
    max_tolerance = 0.0

    for column in columns:
        if column < 0:
            continue
        if column >= stim_width or column >= rstim_width:
            failure_reasons.append(f"column {column} is out of range for one of the sample sets")
            continue
        stim_hits, stim_total, stim_rate = _bit_rate(stim_samples[:sample_count], column)
        rstim_hits, rstim_total, rstim_rate = _bit_rate(rstim_samples[:sample_count], column)
        delta = abs(stim_rate - rstim_rate)
        tolerance = _tolerance(
            stim_hits,
            stim_total,
            rstim_hits,
            rstim_total,
            z_score=z_score,
            floor=floor,
        )
        max_delta = max(max_delta, delta)
        max_tolerance = max(max_tolerance, tolerance)
        if delta > tolerance:
            failure_reasons.append(
                f"column {column} exceeds tolerance: delta={delta:.6f}, tolerance={tolerance:.6f}"
            )
        marginals.append(
            {
                "column": column,
                "stim_rate": stim_rate,
                "rstim_rate": rstim_rate,
                "delta": delta,
                "tolerance": tolerance,
                "stim_hits": stim_hits,
                "rstim_hits": rstim_hits,
                "sample_count": sample_count,
            }
        )

    for left, right in pairs:
        if left < 0 or right < 0:
            continue
        if left >= stim_width or right >= stim_width or left >= rstim_width or right >= rstim_width:
            failure_reasons.append(
                f"pair ({left}, {right}) is out of range for one of the sample sets"
            )
            continue
        stim_hits, stim_total, stim_rate = _pair_rate(stim_samples[:sample_count], left, right)
        rstim_hits, rstim_total, rstim_rate = _pair_rate(rstim_samples[:sample_count], left, right)
        delta = abs(stim_rate - rstim_rate)
        tolerance = _tolerance(
            stim_hits,
            stim_total,
            rstim_hits,
            rstim_total,
            z_score=z_score,
            floor=floor,
        )
        max_delta = max(max_delta, delta)
        max_tolerance = max(max_tolerance, tolerance)
        if delta > tolerance:
            failure_reasons.append(
                f"pair ({left}, {right}) exceeds tolerance: delta={delta:.6f}, tolerance={tolerance:.6f}"
            )
        pair_stats.append(
            {
                "pair": [left, right],
                "stim_rate": stim_rate,
                "rstim_rate": rstim_rate,
                "delta": delta,
                "tolerance": tolerance,
                "stim_hits": stim_hits,
                "rstim_hits": rstim_hits,
                "sample_count": sample_count,
            }
        )

    status = STATUS_PASS if not failure_reasons else STATUS_MISMATCH
    return {
        "status": status,
        "sample_count": sample_count,
        "marginals": marginals,
        "pairs": pair_stats,
        "max_delta": max_delta,
        "max_tolerance": max_tolerance,
        "failure_reasons": failure_reasons,
    }
