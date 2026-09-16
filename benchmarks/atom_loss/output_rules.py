"""Standard-library-only replay of decoder output publication semantics."""


def evaluate_run(record):
    """Derive output-rule problems from raw fields in one executed case."""
    problems = []
    expected = record['expected']
    if expected['outcome'] == 'success':
        if record['exit_code'] != 0:
            problems.append('expected success, observed exit ' + str(record['exit_code']))
        if not record['predictions']['installed']:
            problems.append('predictions missing for a successful run')
        if record['completed_shots'] != record['shots']:
            problems.append(
                f"completed shots {record['completed_shots']} != declared {record['shots']}"
            )
    else:
        if record['exit_code'] != expected['exit_code']:
            problems.append(
                f"expected exit {expected['exit_code']}, observed {record['exit_code']}"
            )
        if record.get('error_code') != expected['error_code']:
            problems.append(
                f"expected error {expected['error_code']}, "
                f"observed {record.get('error_code')!r}"
            )
        if record['completed_shots'] != 0:
            problems.append('a rejected/incomplete run must report zero completed shots')
        if record['predictions']['installed'] and not record['predictions'].get('pre_existing'):
            problems.append(
                'a failed run installed a prediction file that did not exist before; '
                'truncated or partial predictions must never be published'
            )
        if (record['predictions'].get('pre_existing')
                and not record['predictions'].get('unchanged')):
            problems.append('a pre-existing prediction file was modified by a failed run')
        want_stats = expected.get('stats_written', False)
        if record['stats_written'] != want_stats:
            problems.append(
                f"stats_written expected {want_stats}, observed {record['stats_written']}"
            )
    return problems
