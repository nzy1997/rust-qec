"""Roundtrip and corruption controls for lossless publication encoding."""
import gzip
from pathlib import Path
import random
import tempfile
from evidence_io import read_event_bytes, write_event_bytes


def require(value, message):
    if not value:
        raise ValueError(message)


def rejects(call):
    try:
        call()
    except (ValueError, gzip.BadGzipFile, EOFError):
        return
    raise ValueError('corrupt or ambiguous event stream accepted')


def main():
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        wire = '{"index":0,"value":"记录"}\n'.encode()
        plain = root / 'plain'
        plain.mkdir()
        (plain / 'events.jsonl').write_bytes(wire)
        require(read_event_bytes(plain) == wire, 'plain roundtrip failed')
        (plain / 'events.jsonl.gz').write_bytes(gzip.compress(wire, mtime=0))
        rejects(lambda: read_event_bytes(plain))
        rejects(lambda: write_event_bytes(plain, wire))

        small = root / 'small'
        small.mkdir()
        write_event_bytes(small, wire)
        require(read_event_bytes(small) == wire, 'gzip roundtrip failed')
        rejects(lambda: write_event_bytes(small, wire))

        split = root / 'split'
        split.mkdir()
        # Split a multibyte UTF-8 character, not a JSON line boundary.
        cut = wire.index('记'.encode()) + 1
        (split / 'events.part-00000.jsonl.gz').write_bytes(gzip.compress(wire[:cut], mtime=0))
        (split / 'events.part-00001.jsonl.gz').write_bytes(gzip.compress(wire[cut:], mtime=0))
        require(read_event_bytes(split) == wire, 'split UTF-8 roundtrip failed')
        (split / 'events.part-00001.jsonl.gz').rename(split / 'events.part-00002.jsonl.gz')
        rejects(lambda: read_event_bytes(split))
        (split / 'events.part-00002.jsonl.gz').rename(split / 'events.part-00001.jsonl.gz')
        (split / 'events.jsonl.gz').write_bytes(gzip.compress(wire, mtime=0))
        rejects(lambda: read_event_bytes(split))
        (split / 'events.jsonl.gz').unlink()
        (split / 'events.part-00001.jsonl.gz').write_bytes(b'not gzip')
        rejects(lambda: read_event_bytes(split))

        large = root / 'large'
        large.mkdir()
        # Incompressible data exercises bounded Git blobs rather than a tiny
        # repeated fixture that always falls below the single-file threshold.
        data = random.Random(749).randbytes(11 * 1024 * 1024)
        write_event_bytes(large, data)
        parts = list(large.glob('events.part-*.jsonl.gz'))
        require(len(parts) > 1, 'large stream did not use parts')
        require(all(p.stat().st_size < 10 * 1024 * 1024 for p in parts), 'oversized Git blob')
        require(all(p.read_bytes()[4:8] == b'\0' * 4 for p in parts), 'gzip mtime is not reproducible')
        require(read_event_bytes(large) == data, 'large stream roundtrip failed')
    print('PASS lossless plain/gzip/parts, bounded blobs and corrupt encoding controls')


if __name__ == '__main__':
    main()
