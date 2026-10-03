"""Retain full scale verification payloads while reusing the unchanged timing driver."""
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent


def main():
    spec = importlib.util.spec_from_file_location('row_ops_entry', HERE / 'run.py')
    entry = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(entry)
    bind = entry.bind

    def bind_semantics(campaign, suite):
        bind(campaign, suite)
        output = campaign.output
        save = campaign.atomic_save
        payloads = {}

        def capture(cmd, **kwargs):
            value = output(cmd, **kwargs)
            if len(cmd) == 3 and cmd[-1] == 'verify':
                label = Path(cmd[0]).parent.name
                payloads.setdefault(label, {})[cmd[1]] = json.loads(value)
            return value

        def retain(path, result):
            result['row_ops_scale_entry_sha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
            result['row_ops_verification_results'] = payloads
            save(path, result)

        campaign.output = capture
        campaign.atomic_save = retain

    entry.bind = bind_semantics
    sys.argv = [sys.argv[0], '--suite', 'scale', *sys.argv[1:]]
    entry.main()


if __name__ == '__main__':
    main()
