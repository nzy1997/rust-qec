#!/usr/bin/env python3
"""Regenerate the Stim-derived loss-visible decoder fixtures.

Produces the third-party-structured conformance fixtures under
rustqec-cli/tests/fixtures/:

- stim_rotated_memory_z_d3_r2.stim: the unmodified, flattened circuit emitted
  by Google's Stim (pure Stim dialect).
- stim_rotated_memory_z_d3_r2.dem: the detector error model computed by Stim
  itself for that circuit (flat, not decomposed), pinned for DEM-parity
  tests. Decomposed forms legitimately differ in granularity between
  implementations, so the parity check compares undecomposed models.
- stim_rotated_memory_z_d3_r2_loss_visible.stim: the same circuit annotated
  into the loss-visible circuit subset v1
  (docs/specs/loss-visible-circuit-subset-v1.md) by this script:

  * mid-round ancilla ``MR`` -> ``MRL`` (loss-visible readout with reset),
  * terminal data-qubit ``M`` -> ``ML``,
  * ``LOSS(p_op)`` after every CX layer on the participating qubits and
    ``LOSS(p_meas)`` before every loss-visible readout,
  * ``rec[-k]`` references rewritten to ``rec[-(2k-1)]`` because every
    converted readout inserts a flag record before its value record.

Requires: ``pip install stim==1.16.0``.

Usage:

    python3 tools/annotate_loss_visible.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

import stim

DISTANCE = 3
ROUNDS = 2
PAULI_P = 0.001
OPERATION_LOSS_P = 0.01
MEASUREMENT_LOSS_P = 0.02

FIXTURE_DIR = Path(__file__).resolve().parent.parent / "rustqec-cli" / "tests" / "fixtures"
BASE_NAME = f"stim_rotated_memory_z_d{DISTANCE}_r{ROUNDS}"

REC_REF = re.compile(r"rec\[-(\d+)\]")


def shift_rec_refs(line: str) -> str:
    """Rewriting rule: every readout becomes flag+value, so old index i moves
    to 2i+1 and a relative reference rec[-k] becomes rec[-(2k-1)]."""
    return REC_REF.sub(lambda match: f"rec[-{2 * int(match.group(1)) - 1}]", line)


def annotate(circuit_text: str) -> str:
    out: list[str] = []
    converted = 0
    for raw in circuit_text.splitlines():
        line = raw.strip()
        if not line:
            continue
        name = line.split("(", 1)[0].split()[0]
        if name == "CX":
            targets = line.split(" ", 1)[1]
            out.append(line)
            out.append(f"LOSS({OPERATION_LOSS_P}) {targets}")
        elif name == "MR":
            targets = line.split(" ", 1)[1]
            out.append(f"LOSS({MEASUREMENT_LOSS_P}) {targets}")
            out.append(f"MRL {targets}")
            converted += len(targets.split())
        elif name == "M":
            targets = line.split(" ", 1)[1]
            out.append(f"LOSS({MEASUREMENT_LOSS_P}) {targets}")
            out.append(f"ML {targets}")
            converted += len(targets.split())
        elif name in ("DETECTOR", "OBSERVABLE_INCLUDE"):
            out.append(shift_rec_refs(line))
        elif name == "R" and converted == 0 and not any("RSTIM_LOGICAL_FLIP_POINT" in entry for entry in out):
            out.append(line)
            out.append("# RSTIM_LOGICAL_FLIP_POINT")
        else:
            out.append(line)
    if converted == 0:
        raise SystemExit("no measurements were converted; rec rewrite rule would be invalid")
    return "\n".join(out) + "\n"


def main() -> None:
    circuit = stim.Circuit.generated(
        "surface_code:rotated_memory_z",
        distance=DISTANCE,
        rounds=ROUNDS,
        after_clifford_depolarization=PAULI_P,
    ).flattened()
    circuit_text = str(circuit)

    FIXTURE_DIR.mkdir(parents=True, exist_ok=True)
    (FIXTURE_DIR / f"{BASE_NAME}.stim").write_text(circuit_text)
    dem = circuit.detector_error_model(decompose_errors=False)
    (FIXTURE_DIR / f"{BASE_NAME}.dem").write_text(str(dem))
    annotated = annotate(circuit_text)
    (FIXTURE_DIR / f"{BASE_NAME}_loss_visible.stim").write_text(annotated)
    print(f"wrote fixtures to {FIXTURE_DIR}", file=sys.stderr)


if __name__ == "__main__":
    main()
