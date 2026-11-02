#!/usr/bin/env python3
"""Capture the complete original sequential range protocol and exact weights."""

import argparse
import ast
from fractions import Fraction
import hashlib
import json
from pathlib import Path
from random import Random
import sys
import unittest

HERE = Path(__file__).resolve().parent
if str(HERE) not in sys.path:
    sys.path.insert(0, str(HERE))

import reference
from witness_environment import native_reference
from witness_paths import admit_destinations


TEST_FILE = "tests/core/test_weighted_noise.py"
TEST_CLASS = "NativeNoiseStreamTests"
TEST_METHOD = "test_original_randrange_stream_and_exact_weights"
TEST_ID = "test_weighted_noise." + TEST_CLASS + "." + TEST_METHOD


def digest_json(value):
    return hashlib.sha256(json.dumps(value, separators=(",", ":")).encode()).hexdigest()


def reference_draw(sites, random, *, physical_rate=None, proposal_scale=None):
    """Independent Fraction implementation retaining occurrence and I choices."""
    choices, faults, flips = [], [], []
    physical_mass = proposal_mass = Fraction(1)
    for site in sites:
        p = site.probability if physical_rate is None else Fraction(physical_rate)
        q = p if proposal_scale is None or p in (0, 1) else p * Fraction(proposal_scale)
        if random.randrange(q.denominator) >= q.numerator:
            choices.append(None)
            physical_mass *= 1 - p
            proposal_mass *= 1 - q
            continue
        if site.weights is None:
            channel = random.randrange(len(site.channels))
            chance = Fraction(1, len(site.channels))
        else:
            remaining = Fraction(1)
            for channel, chance in enumerate(site.weights):
                conditional = chance / remaining
                if conditional and random.randrange(conditional.denominator) < conditional.numerator:
                    break
                remaining -= chance
            else:
                raise RuntimeError("unit-sum reference channel weights did not select a channel")
        choices.append(channel)
        physical_mass *= p * chance
        proposal_mass *= q * chance
        action = site.channels[channel]
        if action[0] == "REPORT":
            flips.append(action[1])
        else:
            faults.extend(dict(after=site.after, qubit=qubit, pauli=pauli)
                          for qubit, pauli in zip(site.qubits, action) if pauli != "I")
    return dict(channels=choices, faults=faults, flips=flips,
                physical_probability=str(physical_mass), proposal_probability=str(proposal_mass),
                weight=str(physical_mass / proposal_mass))


def result_tuple(draw):
    return tuple(draw["faults"]), tuple(draw["flips"]), Fraction(draw["weight"])


class TapeRange:
    def __init__(self, tape):
        self.tape, self.position = tape, 0

    def randrange(self, upper):
        if self.position >= len(self.tape):
            raise RuntimeError("native range tape ended before reference protocol")
        requested, value = self.tape[self.position]
        self.position += 1
        if int(requested) != upper or not 0 <= int(value) < upper:
            raise RuntimeError("native range request/result differs from reference protocol")
        return int(value)


def run_original(test_source, noise_site, noise_model):
    """Execute the unchanged original class AST, without its module imports."""
    streams, cases, reference_positions = [], {}, {}

    class RecordingRandom(Random):
        def __init__(self, seed):
            self.serial, self.tape, self.states = len(streams), [], []
            super().__init__(seed)
            streams.append(self)

        def randrange(self, upper):
            value = super().randrange(upper)
            self.tape.append([str(upper), str(value)])
            return value

        def getstate(self):
            state = super().getstate()
            self.states.append(digest_json(state))
            return state

    class RecordingModel:
        def __init__(self, sites):
            self.sites, self.native = sites, noise_model(sites)

        def draw(self, random, *, physical_rate=None, proposal_scale=None):
            if random.serial % 2 or random.serial + 1 >= len(streams):
                raise RuntimeError("original native/reference random source pairing changed")
            index = random.serial // 2
            start = len(random.tape)
            actual = self.native.draw(random, physical_rate=physical_rate,
                                      proposal_scale=proposal_scale)
            segment = random.tape[start:]
            replay = TapeRange(segment)
            expected = reference_draw(self.sites, replay, physical_rate=physical_rate,
                                      proposal_scale=proposal_scale)
            if replay.position != len(segment) or actual != result_tuple(expected):
                raise RuntimeError("wrong native oracle: result or range protocol differs")
            reference_random = streams[random.serial + 1]
            prior = reference_positions.get(index, 0)
            if reference_random.tape[prior:] != segment:
                raise RuntimeError("ordered native/reference range requests/results differ")
            reference_positions[index] = len(reference_random.tape)
            key = dict(physical_rate=None if physical_rate is None else str(physical_rate),
                       proposal_scale=None if proposal_scale is None else str(proposal_scale))
            if index not in cases:
                cases[index] = dict(**key, seed=814, draws=[])
            if any(cases[index][name] != value for name, value in key.items()):
                raise RuntimeError("original configuration changed within a random stream")
            expected.update(range_start=start, range_end=len(random.tape),
                            post_state_sha256=digest_json(Random.getstate(random)))
            cases[index]["draws"].append(expected)
            return actual

    parsed = ast.parse(test_source)
    classes = [node for node in parsed.body
               if isinstance(node, ast.ClassDef) and node.name == TEST_CLASS]
    if len(classes) != 1:
        raise RuntimeError("original noise-stream class missing or repeated")
    original = classes[0]
    methods = [node for node in original.body if isinstance(node, ast.FunctionDef)]
    if len(methods) != 1 or methods[0].name != TEST_METHOD:
        raise RuntimeError("original noise-stream finite method inventory changed")
    scope = dict(__name__="_pinned_noise_stream_witness", unittest=unittest,
                 Fraction=Fraction, Random=RecordingRandom,
                 NoiseSite=noise_site, NoiseModel=RecordingModel)
    # The operation_hooks import elsewhere in the upstream test module can
    # load a build-capable test helper. Only this literal original class runs.
    exec(compile(ast.Module(body=[original], type_ignores=[]), TEST_FILE, "exec"), scope)
    assertions = [0]

    class CountedOriginal(scope[TEST_CLASS]):
        def assertEqual(self, first, second, msg=None):
            assertions[0] += 1
            return super().assertEqual(first, second, msg)

    result = unittest.TestResult()
    CountedOriginal(TEST_METHOD).run(result)
    if not result.wasSuccessful():
        raise RuntimeError("original finite noise-stream assertion failed: "
                           + repr(result.errors + result.failures))
    if result.testsRun != 1 or assertions[0] != 960 or len(streams) != 8 or set(cases) != set(range(4)):
        raise RuntimeError("complete original 480-draw assertion inventory changed")
    calls = 0
    for index, case in cases.items():
        native, independent = streams[2 * index:2 * index + 2]
        if (len(case["draws"]) != 120 or native.tape != independent.tape
                or len(native.states) != 120 or native.states != independent.states):
            raise RuntimeError("incomplete original per-draw random state or range equality")
        if [row["post_state_sha256"] for row in case["draws"]] != native.states:
            raise RuntimeError("native post-draw state differs from original state assertion")
        case["range_tape"] = native.tape
        calls += len(native.tape)
    if calls != 3076:
        raise RuntimeError("complete original 3076-call range inventory changed")
    return [cases[index] for index in range(4)], dict(
        original_methods=1, original_assertions=960, draws=480, range_calls=3076,
        native_reference_ranges_equal=True, all_per_draw_python_states_equal=True,
        original_module_imported=False, original_build_loader_or_test_helper_imported=False)


def capture(source):
    source = Path(source).resolve()
    manifest = reference.load_manifest()
    before = reference.verify_source(source, manifest)
    captured = None
    binary = None
    try:
        with native_reference(source) as captured:
            binary = Path(captured.native.__file__).resolve()
            from magna.noise import NoiseModel, NoiseSite
            test_bytes = (source / TEST_FILE).read_bytes()
            test_hash = hashlib.sha256(test_bytes).hexdigest()
            if test_hash != manifest["upstream"]["files"][TEST_FILE]:
                raise reference.ReferenceError("original noise-stream test Source hash differs")
            cases, proof = run_original(test_bytes.decode(), NoiseSite, NoiseModel)
            portable = dict(schema="rmagna.original-noise-stream.v1", source=dict(
                upstream_commit=manifest["upstream"]["commit"],
                reference_manifest_sha256=captured.manifest_sha256,
                test_id=TEST_ID, test_file=TEST_FILE, test_source_sha256=test_hash,
                test_source=test_bytes.decode()), cases=cases)
            host = dict(schema="rmagna.original-noise-stream-host.v1", proof=proof,
                        producer=dict(path=str(Path(__file__).resolve()),
                                      sha256=reference.digest(Path(__file__))),
                        native=dict(path=str(binary), sha256=captured.native_sha256),
                        python=sys.version, upstream_root=str(source))
            return portable, host
    finally:
        # The shared context checks after a successful yield. This outer check
        # also verifies inputs when import, native capture or an assertion fails.
        if reference.verify_source(source, manifest) != before:
            raise reference.ReferenceError("source evidence changed during failed noise capture")
        if binary is not None and reference.digest(binary) != captured.native_sha256:
            raise reference.ReferenceError("native binary changed during noise capture")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    args = parser.parse_args()
    output, receipt = admit_destinations([args.out, args.receipt], args.upstream)
    document, host = capture(args.upstream)
    data = (json.dumps(document, indent=2, sort_keys=True) + "\n").encode()
    host["fixture"] = dict(sha256=hashlib.sha256(data).hexdigest(), bytes=len(data))
    with output.open("xb") as stream:
        stream.write(data)
    with receipt.open("x") as stream:
        json.dump(host, stream, indent=2, sort_keys=True)
        stream.write("\n")
    print(json.dumps(dict(draws=480, range_calls=3076, sha256=host["fixture"]["sha256"])))


if __name__ == "__main__":
    main()
