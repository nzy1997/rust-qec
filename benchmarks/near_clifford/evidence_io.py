"""Lossless event stream storage; closure hashes always cover decoded bytes."""
import gzip
from pathlib import Path


def read_event_bytes(out: Path) -> bytes:
    plain = out / 'events.jsonl'
    compressed = out / 'events.jsonl.gz'
    parts = sorted(out.glob('events.part-*.jsonl.gz'))
    if sum([plain.exists(), compressed.exists(), bool(parts)]) != 1:
        raise ValueError('expected exactly one event stream encoding')
    if plain.exists():
        return plain.read_bytes()
    if compressed.exists():
        return gzip.decompress(compressed.read_bytes())
    expected = [f'events.part-{index:05d}.jsonl.gz' for index in range(len(parts))]
    if [part.name for part in parts] != expected:
        raise ValueError('event stream parts must have contiguous canonical indices')
    return b''.join(gzip.decompress(part.read_bytes()) for part in parts)


def write_event_bytes(out: Path, data: bytes) -> None:
    if (out / 'events.jsonl').exists() or (out / 'events.jsonl.gz').exists() or list(out.glob('events.part-*.jsonl.gz')):
        raise ValueError('refusing to overwrite an existing event stream')
    if len(data) > 100000 * 4 * 1024 * 1024:
        raise ValueError('event stream exceeds canonical part capacity')
    encoded = gzip.compress(data, mtime=0)
    if len(encoded) < 10 * 1024 * 1024:
        (out / 'events.jsonl.gz').write_bytes(encoded)
        return
    # Bound each Git blob independently of entropy or individual JSON line size.
    # Parts may split a line; read_event_bytes concatenates before JSON parsing.
    for index, offset in enumerate(range(0, len(data), 4 * 1024 * 1024)):
        (out / f'events.part-{index:05d}.jsonl.gz').write_bytes(
            gzip.compress(data[offset:offset + 4 * 1024 * 1024], mtime=0))
