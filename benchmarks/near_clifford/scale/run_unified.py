"""Run the retained matrix with the unified-CLI dependency lock.

Historical runner, entry, driver and lock files stay byte-identical so their
retained evidence remains verifiable. Only the Cargo input lock is selected here.
"""
import argparse
import hashlib
from pathlib import Path
import subprocess
import sys
import run as campaign
from run_pair import report

HERE = Path(__file__).resolve().parent


class HarnessInputs:
    """Provide unchanged driver/fixtures and the separately retained new lock."""
    def __truediv__(self, name):
        return HERE / ('Cargo.unified.lock' if name == 'Cargo.lock' else name)


def main():
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument('--baseline', required=True)
    parser.add_argument('--candidate', default='HEAD')
    options, remaining = parser.parse_known_args()
    campaign.REVISIONS = {
        label: subprocess.check_output(['git', 'rev-parse', revision + '^{commit}'],
                                       cwd=campaign.ROOT, text=True).strip()
        for label, revision in [('baseline', options.baseline), ('candidate', options.candidate)]
    }
    if campaign.REVISIONS['baseline'] == campaign.REVISIONS['candidate']:
        parser.error('revisions must differ')
    if '--scratch' not in remaining:
        parser.error('use a fresh explicit --scratch directory')
    campaign.HARNESS = HarnessInputs()
    save = campaign.atomic_save
    def save_with_entry(path, result):
        result['campaign_entry_sha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
        result['campaign_entry'] = 'run_unified.py'
        save(path, result)
    campaign.atomic_save = save_with_entry
    campaign.report = report
    sys.argv = [sys.argv[0], *remaining]
    campaign.main()


if __name__ == '__main__':
    main()
