from __future__ import annotations

import argparse
from pathlib import Path


ORIGINAL_TWO_QUBIT_ERROR_PROBABILITY = 0.001
PER_EVENT_PROBABILITY = 1.0 - (1.0 - ORIGINAL_TWO_QUBIT_ERROR_PROBABILITY) ** (1.0 / 3.0)
PER_EVENT_PROBABILITY_TEXT = "0.0003334445062"
BASELINE_DEPOLARIZE2_PREFIX = "DEPOLARIZE2(0.001) "


def transform_circuit(text: str) -> str:
    lines = text.splitlines()
    transformed: list[str] = []
    two_qubit_layers = 0
    index = 0
    while index < len(lines):
        line = lines[index]
        instruction = line.lstrip()
        if not instruction.startswith("CX "):
            if instruction.startswith("DEPOLARIZE2("):
                raise ValueError(f"orphan DEPOLARIZE2 layer at line {index + 1}")
            transformed.append(line)
            index += 1
            continue

        next_instruction = lines[index + 1].lstrip() if index + 1 < len(lines) else ""
        if not next_instruction.startswith(BASELINE_DEPOLARIZE2_PREFIX):
            raise ValueError(f"CX layer at line {index + 1} is not followed by DEPOLARIZE2(0.001)")
        indentation = line[: len(line) - len(instruction)]
        targets = instruction.removeprefix("CX ")
        depolarize_targets = next_instruction.removeprefix(BASELINE_DEPOLARIZE2_PREFIX)
        if targets != depolarize_targets:
            raise ValueError(f"CX and DEPOLARIZE2 targets do not match at line {index + 1}")

        transformed.extend(
            [
                line,
                f"{indentation}LOSS({PER_EVENT_PROBABILITY_TEXT}) {targets}",
                f"{indentation}DEPOLARIZE2({PER_EVENT_PROBABILITY_TEXT}) {targets}",
            ]
        )
        two_qubit_layers += 1
        index += 2

    if two_qubit_layers == 0:
        raise ValueError("fixture contains no CX / DEPOLARIZE2 layers")
    return "\n".join(transformed) + "\n"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Build the paired atom-loss sample benchmark fixture.")
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    source = args.input.read_text(encoding="utf-8")
    args.output.write_text(transform_circuit(source), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
