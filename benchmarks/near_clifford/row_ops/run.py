"""Run unchanged scale/entangled campaigns with explicit tableau source binding."""
import argparse
import hashlib
import importlib.util
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
BASE = HERE.parent


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def bind(campaign, suite):
    build = campaign.build
    save = campaign.atomic_save

    def bound_build(scratch, label, revision, diagnostic=False):
        binary, metadata = build(scratch, label, revision, diagnostic)
        folder = label + ('-diagnostic' if diagnostic else '')
        metadata['tableau_source_sha256'] = sha(scratch / folder / 'source/rstim/src/sim/tableau.rs')
        return binary, metadata

    def bound_save(path, result):
        result['row_ops_schema'] = 'near-clifford.row-ops.v1'
        result['row_ops_suite'] = suite
        result['row_ops_entry_sha256'] = sha(Path(__file__))
        save(path, result)

    campaign.build = bound_build
    campaign.atomic_save = bound_save


def main():
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument('--suite', choices=['scale', 'entangled'], required=True)
    parser.add_argument('--scratch', type=Path, required=True)
    options, remaining = parser.parse_known_args()
    if options.scratch.exists():
        parser.error('scratch must be a fresh nonexistent directory')
    sys.argv = [sys.argv[0], '--scratch', str(options.scratch), *remaining]
    if options.suite == 'scale':
        sys.path.insert(0, str(BASE / 'scale'))
        import run_unified
        bind(run_unified.campaign, 'scale')
        run_unified.main()
    else:
        spec = importlib.util.spec_from_file_location('entangled_entry', BASE / 'entangled/run.py')
        entry = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(entry)
        load = entry.load

        def bound_load(path, name):
            module = load(path, name)
            if path == BASE / 'scale/run.py':
                bind(module, 'entangled')
            return module

        entry.load = bound_load
        entry.main()


if __name__ == '__main__':
    main()
