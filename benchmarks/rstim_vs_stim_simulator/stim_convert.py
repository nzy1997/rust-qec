"""Convert shot data between Stim result formats via the pinned stim Python API.

The pinned Stim CLI streams one measurement record at a time and refuses to
write `ptb64` (`SAMPLE_FORMAT_PTB64 incompatible with SingleMeasurementRecord`).
This helper buffers the full shot batch through `stim.read_shot_data_file` and
`stim.write_shot_data_file` from the same pinned stim package so `ptb64`
serialization stays reproducible and pinned.
"""
from __future__ import annotations

import argparse

import stim


FORMATS = ("b8", "r8", "ptb64")


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Convert shot data between Stim result formats")
    parser.add_argument("--in", dest="in_path", required=True)
    parser.add_argument("--in_format", required=True, choices=FORMATS)
    parser.add_argument("--out", dest="out_path", required=True)
    parser.add_argument("--out_format", required=True, choices=FORMATS)
    parser.add_argument("--bits_per_shot", required=True, type=int)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    data = stim.read_shot_data_file(
        path=args.in_path,
        format=args.in_format,
        num_measurements=args.bits_per_shot,
    )
    stim.write_shot_data_file(
        data=data,
        path=args.out_path,
        format=args.out_format,
        num_measurements=args.bits_per_shot,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
