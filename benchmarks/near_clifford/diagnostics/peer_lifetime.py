"""Isolated public peer histories; API sums exclude record conversion/drop."""
import hashlib
import json
from pathlib import Path
import resource
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'compiled_sota'))
from worker import loaded_files, prepare


def main():
    if not sys.flags.isolated:
        raise ValueError('Python -I required')
    config = json.loads(Path(sys.argv[1]).read_text())
    backend = config['backend']
    identity = loaded_files(backend)
    data = Path(config['circuit']).read_bytes()
    text = data.decode()
    histories = []
    for rep in range(config['repetitions']):
        start = time.perf_counter_ns()
        sample, rank, width = prepare(backend, text, config['batch'])
        compile_ns = time.perf_counter_ns() - start
        calls = []
        for index, request in enumerate(config['history']):
            start = time.perf_counter_ns()
            output = sample(request['shots'], 739 + rep * 100 + index)
            elapsed = time.perf_counter_ns() - start
            if output.shape != (request['shots'], width):
                raise ValueError('invalid peer record shape')
            calls.append(dict(request=request, ns=elapsed))
            del output
        sampling_ns = sum(call['ns'] for call in calls)
        histories.append(dict(compile_ns=compile_ns, prepare_ns=0,
                              sampling_ns=sampling_ns, phase_sum_ns=compile_ns+sampling_ns, calls=calls))
    if identity != loaded_files(backend):
        raise ValueError('peer import identity changed')
    rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    if sys.platform.startswith('linux'):
        rss *= 1024
    print(json.dumps(dict(status='ok', backend=backend, config=config,
        input_sha256=hashlib.sha256(data).hexdigest(), loaded_files=identity,
        isolated=True, peak_active_width=rank, width=width, histories=histories,
        peak_rss_bytes=rss, rss_scope='whole Python process including interpreter/imports')))


if __name__ == '__main__':
    main()
