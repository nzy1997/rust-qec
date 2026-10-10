"""Validate the perf clock and retain an auditable warm-loop sample selection."""
from pathlib import Path
import re
import struct


def require(value, reason):
    if not value:
        raise ValueError(reason)


def record_perf_path(header):
    # perf's retained cmdline can contain only argv[0], without "record".
    lines = re.findall(r'(?m)^#[ \t]*cmdline[ \t]*:[ \t]*(/\S*/perf)(?:[ \t]+[^\n]*)?$', header)
    require(len(lines) == 1, 'one absolute record perf path required from raw header')
    return Path(lines[0])


def perf_clock(data):
    # Linux v6.17 UAPI perf_event_attr: sample_type at24, flags at40,
    # clockid at92; use_clockid is bit25. The file header uses LE u64s.
    require(len(data) >= 104 and data[:8] == b'PERFILE2', 'little-endian perf file required')
    header_size, entry_size, offset, length = struct.unpack_from('<4Q', data, 8)
    require(104 <= header_size <= len(data) and entry_size >= 112 and
            offset >= header_size and length in [entry_size, 2 * entry_size] and
            offset + length <= len(data), 'one sample event and optional bounded tracking event required')
    attributes = []
    for position in range(offset, offset + length, entry_size):
        event_type, attr_size, config = struct.unpack_from('<IIQ', data, position)
        period = struct.unpack_from('<Q', data, position + 16)[0]
        sample_type = struct.unpack_from('<Q', data, position + 24)[0]
        flags = struct.unpack_from('<Q', data, position + 40)[0]
        clock_id = struct.unpack_from('<i', data, position + 92)[0]
        require(96 <= attr_size <= entry_size - 16 and event_type == 1 and config in [0, 9],
                'only cpu-clock and non-sampling dummy tracking attributes allowed')
        require(flags & (1 << 25) and clock_id == 1, 'raw event must explicitly use CLOCK_MONOTONIC')
        if config == 0:
            required_fields = (1 << 0) | (1 << 1) | (1 << 2) | (1 << 7)
            require(flags & (1 << 5) and not flags & (1 << 4), 'user-only sampling required')
            require(sample_type & required_fields == required_fields, 'IP/TID/TIME/CPU sample fields required')
        else:
            # perf record adds this sideband tracker when --delay=-1 is set.
            # Linux SW_DUMMY never overflows; it retains exec/mmap metadata.
            require(period == 1 and not flags & (1 << 10), 'dummy tracker must not use frequency sampling')
        attributes.append(dict(event_type=event_type, config=config, clock_id=clock_id,
                               flags=flags, sample_type=sample_type, period=period))
    require([attr['config'] for attr in attributes].count(0) == 1 and
            [attr['config'] for attr in attributes].count(9) <= 1, 'exactly one cpu-clock sampler required')
    return dict(clock='CLOCK_MONOTONIC', clock_id=1, use_clockid=True, attributes=attributes)


def phase_span(phase, pid):
    require(phase.get('schema') == 'rstim.perf-warm-loop.v1' and phase.get('completed') is True,
            'completed warm-loop receipt required')
    require(type(phase.get('pid')) is int and phase['pid'] == pid and
            phase.get('clock') == 'CLOCK_MONOTONIC', 'warm-loop task/clock differs')
    begin, end = phase.get('start_ns'), phase.get('end_ns')
    require(type(begin) is int and type(end) is int and 0 < begin < end,
            'positive integer warm-loop interval required')
    return begin, end


HEADER = re.compile(r'(?m)^[ \t]*(\S+)[ \t]+(\d+)/[ \t]*(\d+)[ \t]+\[(\d+)\][ \t]+(\d+)\.(\d+):[ \t]+(\S+):')
FRAME = re.compile(r'[ \t]+[0-9a-fA-F]+[ \t]+[^\r\n]+[ \t]+\([^()\r\n]*\)')
SAMPLE_ID = re.compile(r'[ \t][+-]?[0-9]+/[ \t]*[+-]?[0-9]+[ \t]+\[[0-9]+\]')


def sample_blocks(text):
    # perf script separates event blocks with an empty line. Delimit the entire
    # raw population before applying the accepted task-header grammar. Otherwise
    # a rejected header could be swallowed as the previous valid callchain.
    blocks, block = [], []
    prefix, separated = True, False
    for line in text.splitlines(keepends=True):
        if prefix:
            if not line.strip():
                continue
            if line.startswith('#'):
                require(line.startswith('# ') or line.strip() == '#', 'unsupported perf metadata prefix')
                require(not SAMPLE_ID.search(line),
                        'sample header must not be hidden in metadata prefix')
                continue
        prefix = False
        if not line.strip():
            block.append(line)
            separated = True
        elif separated:
            blocks.append(''.join(block))
            block = [line]
            separated = False
        else:
            block.append(line)
    if block:
        blocks.append(''.join(block))
    require(blocks, 'actual raw sample blocks required')
    return blocks


def select_samples(text, phase, cpu, pid, executable, *, minimum=100):
    begin, end = phase_span(phase, pid)
    blocks = sample_blocks(text)
    selected, ledger = [], []
    counts = dict(total=0, warm=0, before=0, after=0, native=0, mapped=0, missing_leaf=0)
    for index, block in enumerate(blocks):
        lines = block.splitlines()
        match = HEADER.match(lines[0])
        require(match is not None, 'unsupported raw sample header')
        frames = [line for line in lines[1:] if line.strip()]
        # When perf cannot resolve a callchain cursor, IP/symbol/DSO remain on
        # the event-header line. Treat that supported fallback as the leaf.
        inline = lines[0][match.end():]
        if inline.strip():
            frames.insert(0, inline)
        require(all(FRAME.fullmatch(frame) and not SAMPLE_ID.search(frame) for frame in frames),
                'unsupported frame or undelimited raw sample header')
        comm, sample_pid, tid, sample_cpu, seconds, fraction, event = match.groups()
        require(len(fraction) == 9, 'perf script --ns nanosecond timestamps required')
        stamp = int(seconds) * 1_000_000_000 + int(fraction)
        require(event == 'cpu-clock:u' and int(sample_cpu) == cpu and
                int(sample_pid) == pid and int(tid) == pid, 'sample event/CPU/process/thread differs')
        require(comm == Path(executable).name[:15], 'enabled-window sample must be the native probe')
        region = 'before' if stamp < begin else 'after' if stamp >= end else 'warm'
        counts['total'] += 1
        counts[region] += 1
        if region == 'warm':
            selected.append(block)
            counts['native'] += 1
            counts['mapped'] += any(d in [executable, Path(executable).name]
                                    for d in re.findall(r'\(([^()\n]+)\)', block))
            counts['missing_leaf'] += not frames or '[unknown]' in frames[0]
        ledger.append(dict(index=index, timestamp_ns=stamp, region=region))
    require(counts['native'] >= minimum and counts['mapped'] >= minimum,
            'insufficient native executable-mapped warm samples')
    require(len(blocks) == len(ledger) == counts['total'] == counts['before'] + counts['warm'] + counts['after'],
            'every raw sample must be accounted for')
    return ''.join(selected), dict(schema='rstim.perf-warm-selection.v1',
        interval='[start_ns,end_ns)', start_ns=begin, end_ns=end, counts=counts, samples=ledger,
        scope='warm observation loop including RNG, output drops, timer and bookkeeping; excludes cold setup/final serialization',
        performance_valid=False)


def finite_payload(data, kind):
    # Whole-process RSS and elapsed measurements are not finite-output witnesses.
    if kind == 'structural':
        keys = ['status', 'input_sha256', 'arithmetic', 'shots', 'call_shots', 'call_kind',
                'width', 'measurements', 'continuation']
    else:
        keys = ['backend', 'status', 'input_sha256', 'arithmetic', 'shots', 'call_shots',
                'width', 'measurements', 'exact_native_counts_rng']
    require(all(key in data for key in keys), 'finite witness fields missing')
    payload = {key: data[key] for key in keys}
    if kind == 'counts':
        rows = data['observations']
        require(len(rows) == 1, 'one finite counts observation required')
        keys = ['calls', 'attempted', 'accepted', 'discarded', 'logical_errors']
        payload['counts'] = {key: rows[0][key] for key in keys}
    return payload
