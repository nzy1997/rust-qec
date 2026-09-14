"""Independently validate retained three/five-wire decoder observations.

Only the standard library is used. Adjacent repetition parity checks have two
complementary corrections, so their costs can be derived without the producer's
exhaustive enumeration, graph exporter, or a matching backend.
"""
import math

RAW_BACKENDS = ('native', 'pymatching', 'ignored_conditioning', 'flipped_native')
CASE_FIELDS = {
    'wires', 'rows_checked', 'status', 'hand_derived_graph_pass', 'loss_mapping_pass',
    'placeholder_invariance_pass', 'rejected_rows', 'ignored_conditioning_rejected_rows',
    'flipped_prediction_rejected_rows', 'strict_witness', 'graph', 'raw_predictions',
}
GRAPH_FIELDS = {'edges', 'loss_edges', 'mean_weight', 'syndromes', 'losses'}


def require(condition, message):
    if not condition:
        raise ValueError('Decoder report contract: '+message)


def mapping(value, fields, label):
    require(type(value) is dict and set(value) == set(fields),
            label+' fields are incomplete or unexpected')


def integer_list(values, expected, label):
    require(type(values) is list and all(type(v) is int for v in values)
            and values == expected, label+' differs from independent calculation')


def numeric(value, expected, label):
    require(type(value) in (int, float) and math.isfinite(value)
            and math.isclose(value, expected, rel_tol=1e-12, abs_tol=1e-12),
            label+' differs from independent calculation')


def row_objective(raw, wires, conditioned):
    """Return visible flags, canonical syndrome, and costs in logical [0,1] order."""
    flags = [(raw >> (2*q)) & 1 for q in range(wires)]
    values = [1 if flags[q] else (raw >> (2*q+1)) & 1 for q in range(wires)]
    syndrome = [a ^ b for a, b in zip(values, values[1:])]
    # Given adjacent differences, choosing the first correction bit determines
    # every other bit. Twice-costs stay integral, including exact optimum ties.
    costs = [sum((1 if conditioned and flags[q] else 2)
                 * (values[q] ^ values[0] ^ logical) for q in range(wires))/2
             for logical in (0, 1)]
    return flags, syndrome, costs


def allowed(costs):
    best = min(costs)
    return [logical for logical, cost in enumerate(costs) if cost == best]


def verify_graph(graph, wires):
    mapping(graph, GRAPH_FIELDS, 'graph')
    edges = graph['edges']
    require(type(edges) is list and len(edges) == wires, 'incorrect graph edge count')
    # Wire q flips detector q-1 and q, except for the two boundary wires.
    expected = [(0, None, (0,))] + [(q-1, q, ()) for q in range(1, wires-1)] + [(wires-2, None, ())]
    observed = []
    for edge in edges:
        mapping(edge, ('u', 'v', 'observables', 'weight', 'loss_factor'), 'edge')
        require(type(edge['u']) is int and 0 <= edge['u'] < wires-1, 'invalid edge u')
        require(edge['v'] is None or (type(edge['v']) is int and 0 <= edge['v'] < wires-1), 'invalid edge v')
        obs = edge['observables']
        require(type(obs) is list and all(type(v) is int for v in obs) and obs in ([], [0]), 'invalid edge observables')
        numeric(edge['weight'], math.log(9), 'edge weight')
        numeric(edge['loss_factor'], .5, 'edge loss factor')
        u, v = edge['u'], edge['v']
        observed.append((u, v, tuple(obs)) if v is None else (min(u, v), max(u, v), tuple(obs)))
    require(set(observed) == set(expected), 'incorrect graph topology or logical labels')
    numeric(graph['mean_weight'], math.log(9), 'graph mean weight')
    numeric(graph['mean_weight'], math.fsum(e['weight'] for e in edges)/wires, 'mean of edge weights')
    mapping_rows = graph['loss_edges']
    require(type(mapping_rows) is list and len(mapping_rows) == wires, 'incomplete loss mapping')
    for q, indices in enumerate(mapping_rows):
        integer_list(indices, [observed.index(expected[q])], 'loss mapping for wire '+str(q))
    count = 1 << (2*wires)
    require(type(graph['syndromes']) is list and len(graph['syndromes']) == count, 'incomplete syndrome rows')
    require(type(graph['losses']) is list and len(graph['losses']) == count, 'incomplete loss rows')
    for raw in range(count):
        flags, syndrome, _ = row_objective(raw, wires, True)
        integer_list(graph['syndromes'][raw], syndrome, 'syndrome row '+str(raw))
        integer_list(graph['losses'][raw], [q for q, flag in enumerate(flags) if flag], 'loss row '+str(raw))


def verify_case(case, wires):
    mapping(case, CASE_FIELDS, 'case')
    count = 1 << (2*wires)
    require(type(case['wires']) is int and case['wires'] == wires, 'incorrect wire count or order')
    require(type(case['rows_checked']) is int and case['rows_checked'] == count, 'incorrect row count')
    require(case['status'] == 'PASS', 'case did not pass')
    for field in ('hand_derived_graph_pass', 'loss_mapping_pass', 'placeholder_invariance_pass'):
        require(case[field] is True, 'case failed '+field)
    verify_graph(case['graph'], wires)
    predictions = case['raw_predictions']
    mapping(predictions, RAW_BACKENDS, 'raw predictions')
    for name, values in predictions.items():
        require(type(values) is list and len(values) == count
                and all(type(v) is int and v in (0, 1) for v in values),
                name+' requires complete integer binary predictions')
    conditioned = [allowed(row_objective(raw, wires, True)[2]) for raw in range(count)]
    fixed = [allowed(row_objective(raw, wires, False)[2]) for raw in range(count)]
    rejected = {name: [raw for raw, prediction in enumerate(values) if prediction not in conditioned[raw]]
                for name, values in predictions.items()}
    mapping(case['rejected_rows'], ('native', 'pymatching'), 'rejected rows')
    for name in ('native', 'pymatching'):
        integer_list(case['rejected_rows'][name], rejected[name], name+' rejected rows')
        require(not rejected[name], name+' predictions fail the independent objective')
    require(all(value in fixed[raw] for raw, value in enumerate(predictions['ignored_conditioning'])),
            'ignore-conditioning control does not solve the fixed objective')
    flipped = predictions['native'].copy()
    flipped[0] ^= 1
    require(predictions['flipped_native'] == flipped, 'flipped control must change only native row zero')
    integer_list(case['flipped_prediction_rejected_rows'], rejected['flipped_native'], 'flipped rejected rows')
    require(0 in rejected['flipped_native'], 'flipped prediction was not rejected')
    integer_list(case['ignored_conditioning_rejected_rows'], rejected['ignored_conditioning'], 'ignore-conditioning rejected rows')
    for raw in range(count):
        flags = row_objective(raw, wires, True)[0]
        canonical = raw | sum(1 << (2*q+1) for q, flag in enumerate(flags) if flag)
        require(all(predictions[name][raw] == predictions[name][canonical] for name in ('native', 'pymatching')),
                'placeholder invariance fails')
    witness = case['strict_witness']
    if wires == 3:
        require(witness is None, 'three-wire strict witness must be absent')
        return
    mapping(witness, ('packed_row', 'logical_order', 'fixed_optimum', 'conditioned_optimum',
                     'fixed_decoder_prediction', 'native_prediction', 'pymatching_prediction',
                     'fixed_costs', 'conditioned_costs'), 'strict witness')
    require(type(witness['packed_row']) is int and witness['packed_row'] == 21, 'incorrect strict witness row')
    integer_list(witness['logical_order'], [0, 1], 'witness logical order')
    for name, objective, conditioning in [('fixed', fixed, False), ('conditioned', conditioned, True)]:
        integer_list(witness[name+'_optimum'], objective[21], name+' witness optimum')
        costs = witness[name+'_costs']
        require(type(costs) is list and len(costs) == 2, 'incomplete witness costs')
        for value, expected in zip(costs, row_objective(21, wires, conditioning)[2]):
            numeric(value, expected, name+' witness cost')
    for field, backend in [('fixed_decoder_prediction', 'ignored_conditioning'),
                           ('native_prediction', 'native'), ('pymatching_prediction', 'pymatching')]:
        require(type(witness[field]) is int and witness[field] == predictions[backend][21],
                field+' differs from raw witness prediction')
    require(fixed[21] == [0] and conditioned[21] == [1] and 21 in rejected['ignored_conditioning'],
            'strict conditioning witness did not reject fixed weights')


def verify_decoder(report):
    """Require complete, successful observations consistent with the fixed oracle."""
    mapping(report, ('status', 'method', 'scope', 'cases'), 'report')
    require(report['status'] == 'PASS', 'report did not pass')
    for key in ('method', 'scope'):
        require(type(report[key]) is str and bool(report[key].strip()), 'missing '+key)
    require(type(report['cases']) is list and len(report['cases']) == 2, 'requires both wire cases')
    for case, wires in zip(report['cases'], (3, 5)):
        verify_case(case, wires)


def compare_reports(fresh, published):
    """Revalidate both reports against the same fixed oracle, allowing legal ties.

    Graph permutations (with consistent loss mappings), tiny weight roundoff,
    and alternative equally optimal predictions do not change this experiment.
    """
    verify_decoder(fresh)
    verify_decoder(published)
    require(all(fresh[key] == published[key] for key in ('method', 'scope')),
            'Fresh decoder oracle definitions differ from published evidence')
