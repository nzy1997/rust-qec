"""Standard-library validation of retained physical-chain oracle observations.

Recompute every prediction summary and mutation rejection from the recorded
binary rows and allowed logical answers. Independent physical derivation is the
producer's job; CI also compares a fresh producer run to the published report.
"""
import hashlib


ROWS = 1504
OBJECTIVES = ('envelope-matching', 'envelope-mle')
GRAPH_BACKENDS = ('pymatching-envelope', 'envelope-matching-offline')
BACKENDS = OBJECTIVES + GRAPH_BACKENDS
MUTATIONS = ('empty_edges', 'empty_loss_mapping', 'relative_weights')
SUMMARY_FIELDS = {
    'predictions', 'checked_rows', 'rejected_rows', 'unique_optimum_rows',
    'predicted_ones', 'constant_zero_rejected', 'constant_one_rejected',
    'flipped_prediction_rejected', 'placeholder_invariance', 'prediction_sha256',
}


def require(condition, message):
    if not condition:
        raise ValueError('Chain report contract: '+message)


def mapping(value, names, label, *, exact=True):
    require(type(value) is dict, label+' must be an object')
    require(set(value) == set(names) if exact else set(names) <= set(value),
            label+' fields are incomplete or unexpected')


def bits(values, label):
    require(type(values) is list and len(values) == ROWS, label+' must contain 1504 rows')
    require(all(type(value) is int and value in (0, 1) for value in values),
            label+' must contain integer binary predictions')


def rejected_rows(predictions, choices):
    return [index for index, (value, answers) in enumerate(zip(predictions, choices))
            if value not in answers]


def verify_chain(report):
    """Reject incomplete, internally inconsistent or failed chain evidence."""
    required = {
        'status', 'fixture_sha256', 'distance', 'rounds', 'detectors', 'rows',
        'physical_fault_traces', 'patterns', 'stim_pauli_probes',
        'independent_effects', 'independent_graph_edges', 'native_graph_edges',
        'independent_effects_candidates_and_m2d_pass',
        'compiler_output_mutations_rejected', 'backends', 'allowed_answers',
        'graph_adapter_controls', 'method', 'mle_objective', 'scope',
    }
    mapping(report, required, 'report', exact=False)
    require(report['status'] == 'PASS', 'report did not pass')
    for name, expected in {
        'distance': 3, 'rounds': 2, 'detectors': 16, 'rows': ROWS,
        'physical_fault_traces': 5996, 'patterns': 4,
    }.items():
        require(type(report[name]) is int and report[name] == expected,
                'incorrect fixed workload '+name)
    # Raw graph counts depend on valid DEM decomposition and are diagnostics,
    # not an assertion that the native and independent edge lists are identical.
    for name in ('stim_pauli_probes', 'independent_effects',
                 'independent_graph_edges', 'native_graph_edges'):
        require(type(report[name]) is int and report[name] > 0, 'invalid diagnostic '+name)
    digest = report['fixture_sha256']
    require(type(digest) is str and len(digest) == 64
            and all(c in '0123456789abcdef' for c in digest), 'invalid fixture hash')
    for name in ('method', 'mle_objective', 'scope'):
        require(type(report[name]) is str and bool(report[name].strip()), 'missing '+name)
    require(report['independent_effects_candidates_and_m2d_pass'] is True,
            'independent physical model did not pass')
    compiler = report['compiler_output_mutations_rejected']
    mapping(compiler, ('pauli_weight', 'loss_candidate'), 'compiler mutations')
    require(all(value is True for value in compiler.values()), 'compiler mutation was not rejected')

    answers = report['allowed_answers']
    mapping(answers, OBJECTIVES, 'allowed answers')
    for objective, choices in answers.items():
        require(type(choices) is list and len(choices) == ROWS,
                objective+' allowed answers must contain 1504 rows')
        for row in choices:
            require(type(row) is list and row and all(type(v) is int and v in (0, 1) for v in row)
                    and row == sorted(set(row)), objective+' invalid allowed answer set')
        require(choices[:ROWS//2] == choices[ROWS//2:], objective+' placeholder answer mismatch')
        require(any(len(row) == 1 for row in choices), objective+' lacks unique optimum witnesses')
        require(any(0 not in row for row in choices) and any(1 not in row for row in choices),
                objective+' cannot reject constant predictions')

    backends = report['backends']
    mapping(backends, BACKENDS, 'backends')
    for name, record in backends.items():
        mapping(record, SUMMARY_FIELDS, name+' prediction record')
        predictions = record['predictions']
        bits(predictions, name)
        choices = answers[name if name in OBJECTIVES else 'envelope-matching']
        rejected = rejected_rows(predictions, choices)
        singleton = [i for i, row in enumerate(choices) if len(row) == 1]
        expected = {
            'checked_rows': ROWS, 'rejected_rows': rejected,
            'unique_optimum_rows': len(singleton), 'predicted_ones': sum(predictions),
            'constant_zero_rejected': any(0 not in row for row in choices),
            'constant_one_rejected': any(1 not in row for row in choices),
            'flipped_prediction_rejected': (predictions[singleton[0]] ^ 1) not in choices[singleton[0]],
            'placeholder_invariance': predictions[:ROWS//2] == predictions[ROWS//2:],
            'prediction_sha256': hashlib.sha256(bytes(predictions)).hexdigest(),
        }
        for field, value in expected.items():
            require(type(record[field]) is type(value) and record[field] == value,
                    name+' inconsistent '+field)
        require(not rejected, name+' predictions fail the independent oracle')
        require(expected['placeholder_invariance'] and expected['flipped_prediction_rejected'],
                name+' failed positive-control invariance or sensitivity')

    controls = report['graph_adapter_controls']
    mapping(controls, MUTATIONS, 'graph mutations')
    for mutation, adapters in controls.items():
        mapping(adapters, GRAPH_BACKENDS, mutation+' adapters')
        for name, record in adapters.items():
            label = mutation+'/'+name
            mapping(record, ('outcome', 'predictions', 'rejected_rows'), label)
            if record['outcome'] == 'decoder_error':
                require(mutation == 'empty_edges', label+' requires a real oracle rejection')
                require(record['predictions'] is None and type(record['rejected_rows']) is list
                        and record['rejected_rows'] == [], label+' invalid decoder-error evidence')
            else:
                require(record['outcome'] == 'oracle_rejected', label+' mutation was accepted')
                bits(record['predictions'], label)
                rejected = rejected_rows(record['predictions'], answers['envelope-matching'])
                require(type(record['rejected_rows']) is list
                        and all(type(i) is int for i in record['rejected_rows'])
                        and record['rejected_rows'] == rejected, label+' inconsistent rejected rows')
                require(bool(rejected), label+' has no rejected oracle witness')


def compare_reports(fresh, published):
    """Require the same oracle definitions while allowing legal tied optima."""
    verify_chain(fresh)
    verify_chain(published)
    observations = {'backends', 'graph_adapter_controls'}
    require({key: value for key, value in fresh.items() if key not in observations}
            == {key: value for key, value in published.items() if key not in observations},
            'Fresh chain oracle definitions differ from published evidence')
