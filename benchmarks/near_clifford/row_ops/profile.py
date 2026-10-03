"""Profile verified entangled row-operation evidence with bound tableau inputs."""
import argparse
import importlib.util
import json
from pathlib import Path
import sys
from verify import verify, sha

HERE = Path(__file__).resolve().parent
BASE = HERE.parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('results', type=Path)
    parser.add_argument('--scratch', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = json.loads(args.results.read_text())
    verify(result, args.scratch, git_sources=True)
    if result['row_ops_suite'] != 'entangled':
        parser.error('this profile entry uses the entangled suite')
    sys.path.insert(0, str(BASE / 'entangled'))
    spec = importlib.util.spec_from_file_location('row_ops_profile', BASE / 'entangled/profile.py')
    entry = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(entry)
    sys.argv = [sys.argv[0], str(args.results), '--scratch', str(args.scratch), '--output', str(args.output),
                '--fixtures', 'brick_16_16_3', 'parity_12_129_3', 'rounds_8_129_4']
    entry.main()
    path = args.output / 'metadata.json'
    metadata = json.loads(path.read_text())
    metadata['row_ops_entry_sha256'] = sha(Path(__file__))
    metadata['near_clifford_source_sha256'] = result['sources']['candidate']['near_clifford_source_sha256']
    metadata['tableau_source_sha256'] = result['sources']['candidate']['tableau_source_sha256']
    path.write_text(json.dumps(metadata, indent=2) + '\n')


if __name__ == '__main__':
    main()
