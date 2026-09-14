"""Hand-derived probe definitions shared by sampling and report validation.

Pure Python data/formulas: no sampler, decoder or scientific runtime imports.
"""
import itertools

CASES = {
    'lost_control_skips_cx': 'R 0 1\nX 0\nLOSS(1) 0\nCX 0 1\nML 0 1',
    'lost_target_skips_cx': 'R 0 1\nH 0\nLOSS(1) 1\nCX 0 1\nH 0\nML 0 1',
    'reset_restores_wire': 'R 0 1\nLOSS(1) 0\nR 0\nX 0\nCX 0 1\nML 0 1',
    'readout_reset_restores_wire': 'R 0\nLOSS(1) 0\nMRL 0\nML 0',
    'bell_partner_marginal': 'R 0 1\nH 0\nCX 0 1\nLOSS(1) 0\nML 0 1',
    'loss_at_two_times': 'R 0 1\nH 0\nLOSS(0.2) 0\nCX 0 1\nLOSS(0.3) 1\nML 0 1',
    'two_losses': 'R 0 1\nLOSS(0.3) 0 1\nCX 0 1\nML 0 1',
    'pauli_and_loss': 'R 0 1\nH 0\nCX 0 1\nDEPOLARIZE2(0.17) 0 1\nLOSS(0.2) 0\nX_ERROR(0.11) 1\nML 0 1',
    'persistent_loss_multiple_gates': 'R 0 1 2\nX 0\nLOSS(0.4) 0\nCX 0 1\nCX 0 2\nML 0 1 2',
    'repeat_delayed_readout': 'R 0 1\nREPEAT 3 {\nH 0\nCX 0 1\nLOSS(0.1) 0\n}\nML 0 1',
    'ordinary_lost_measurement': 'R 0 1\nX 0\nLOSS(0.4) 0\nCX 0 1\nM 0 1',
    'no_loss_bell': 'R 0 1\nH 0\nCX 0 1\nLOSS(0) 0\nM 0 1',
}
KNOWN = {
    'lost_control_skips_cx': [1, 1, 0, 0],
    'lost_target_skips_cx': [0, 0, 1, 1],
    'reset_restores_wire': [0, 1, 0, 1],
    'readout_reset_restores_wire': [1, 1, 0, 0],
}


def bell_text(noise, wires):
    data = list(range(wires))
    pairs = ' '.join(f'{q} {q+wires}' for q in data)
    return '\n'.join(['R ' + ' '.join(map(str, range(2*wires))),
                      'H ' + ' '.join(map(str, data)), 'CX ' + pairs, noise,
                      'CX ' + pairs, 'H ' + ' '.join(map(str, data)),
                      'M ' + pairs])


def basis_ops(bases, inverse=False):
    lines = []
    for q, basis in enumerate(bases):
        if basis == 'X': lines.append(f'H {q}')
        elif basis == 'Y':
            lines.extend([f'S_DAG {q}', f'H {q}'] if inverse else [f'H {q}', f'S {q}'])
    return '\n'.join(lines)


def distribution_specs():
    probes = []
    def add(name, text, columns, expected, channel):
        probes.append(dict(name=name, text=text, columns=columns, expected=expected, channel=channel))
    for wires in [1, 2]:
        channel = f'DEPOLARIZE{wires}'
        for p in [0., .17, .6, 1.]:
            noise = f'{channel}({p}) ' + ' '.join(map(str, range(wires)))
            size = 4**wires
            add(f'{channel}_bell_p{p}', bell_text(noise, wires), list(range(2*wires)),
                [1-p] + [p/(size-1)]*(size-1), channel)
    p = .17
    for bases in itertools.product('XYZ', repeat=2):
        text = '\n'.join(['R 0 1', basis_ops(bases), f'DEPOLARIZE2({p}) 0 1',
                          basis_ops(bases, inverse=True), 'M 0 1'])
        add('DEPOLARIZE2_product_' + ''.join(bases), text, [0,1],
            [1-4*p/5] + [4*p/15]*3, 'DEPOLARIZE2')
    # Both loss directions, restoration, and noise before loss. Only surviving
    # bits are scored after loss; placeholders are not physical outcomes.
    for q in [0, 1]:
        for state in ['lost', 'restored', 'before_loss']:
            noise = f'DEPOLARIZE2({p}) 0 1'
            loss = f'LOSS(1) {q}'
            body = [noise, loss] if state == 'before_loss' else [loss] + ([f'R {q}'] if state == 'restored' else []) + [noise]
            columns = [0,1] if state == 'restored' else [1-q]
            expected = [1-4*p/5] + [4*p/15]*3 if state == 'restored' else (
                [1.,0.] if state == 'lost' else [1-8*p/15,8*p/15])
            add(f'DEPOLARIZE2_{state}_{q}', '\n'.join(['R 0 1', *body, 'M 0 1']), columns, expected, 'DEPOLARIZE2')
    for basis in 'XYZ':
        text = '\n'.join(['R 0', basis_ops(basis), f'DEPOLARIZE1({p}) 0',
                          basis_ops(basis, inverse=True), 'M 0'])
        add(f'DEPOLARIZE1_product_{basis}', text, [0], [1-2*p/3,2*p/3], 'DEPOLARIZE1')
    return probes


P = .17


def noise_specs():
    # Observed event is a value bit, except DEPOLARIZE2 where it is Z parity.
    for name, probability, basis in [('X_ERROR',P,''),('Y_ERROR',P,''),
                                     ('Z_ERROR',P,'H 0\n'),('DEPOLARIZE1',2*P/3,''),
                                     ('DEPOLARIZE2',8*P/15,'')]:
        target='0 1' if name=='DEPOLARIZE2' else '0'
        noise=f'{name}({P}) {target}\n'
        for state in ['alive','lost','restored']:
            before='LOSS(1) 0\n' if state!='alive' else ''
            if state=='restored': before+='R 0\n'
            text='R 0 1\n'+before+basis+noise+basis+'ML 0 1'
            # For a lost wire inspect the surviving wire: noise touching loss must skip.
            columns=[3] if state=='lost' else ([1,3] if name=='DEPOLARIZE2' else [1])
            yield f'{name}_{state}',text,columns,0. if state=='lost' else probability,name
    # Nonzero channel before loss must still affect its surviving partner.
    yield 'DEPOLARIZE2_before_loss','R 0 1\nDEPOLARIZE2(0.17) 0 1\nLOSS(1) 0\nML 0 1',[3],8*P/15,'DEPOLARIZE2'


CONFIGURED_LOSS_RATES = [.0001, .0003, .001, .003, .01]
# Mid-SWAP splits a two-qubit operation loss rate between its two targets.
LOSS_RATES = sorted({p for rate in CONFIGURED_LOSS_RATES for p in [rate, rate/2]})

def low_specs():
    probes = []
    for channel in ['X_ERROR','Y_ERROR','Z_ERROR','DEPOLARIZE1','DEPOLARIZE2']:
        wires = 2 if channel == 'DEPOLARIZE2' else 1
        size = 4**wires
        expected = [0.]*size
        expected[0] = .999
        if channel.startswith('DEPOLARIZE'):
            expected[1:] = [.001/(size-1)]*(size-1)
        else:
            expected[{'Z_ERROR':1,'X_ERROR':2,'Y_ERROR':3}[channel]] = .001
        text = bell_text(f'{channel}(0.001) ' + ' '.join(map(str,range(wires))), wires)
        probes.append((channel, text, list(range(2*wires)), expected))
    for p in LOSS_RATES:
        probes.append((f'LOSS_{p}', f'R 0\nLOSS({p}) 0\nML 0', [0], [1-p,p]))
    return probes
