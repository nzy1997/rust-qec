# Stim Test Port Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Port ~348 Stim behavior tests to rstim to ensure correctness parity.

**Architecture:** Direct port from C++ TEST() macros to Rust #[test] functions. Each Stim test file maps to one rstim test file. Tests that already exist in rstim are skipped. The subagent reads each Stim .test.cc file, translates test logic to rstim's API, and verifies all tests pass.

**Tech Stack:** Rust, rstim crate, Stim C++ source as reference

---

### Task 1: Port tableau_simulator.test.cc (62 tests)

**Files:**
- Read: `/Users/nzy/rcode/rstim/Stim/src/stim/simulators/tableau_simulator.test.cc`
- Read: `/Users/nzy/rcode/rstim/rstim/tests/executor_clifford.rs` (check for overlaps)
- Read: `/Users/nzy/rcode/rstim/rstim/tests/frame_sim.rs` (check for overlaps)
- Create: `/Users/nzy/rcode/rstim/rstim/tests/stim_tableau_simulator.rs`

**What to port:** All 62 tests from tableau_simulator.test.cc. Key categories:
- Basic gate behavior: identity, bit_flip, epr, paulis
- Measurement: measure_kickback_z/x/y, measure_x/y/z_entangled, noisy_measurement_x/y/z
- Reset: reset_pure, reset_random, reset_x/y/z_entangled, simulate_reset
- Measure-reset: measure_reset_x/y/z_entangled, noisy_measure_reset_x/y/z, mr_repeated_target
- MPP: measure_pauli_product_1, measure_pauli_product_4body, measure_pauli_product_epr, measure_pauli_product_inversions, measure_pauli_product_noisy
- Pair measure: mxx, myy, mzz, mxx_myy_mzz_vs_mpp_unsigned
- Other: correlated_error, classical_can_control_quantum, mpad, heralded_erase, sweep controls

**Translation pattern:**
- Stim `TableauSimulator<W>` → rstim `parse_lines()` + `sample_batch()` or direct executor calls
- Stim `sim.do_H(0)` → rstim circuit with `H 0` instruction
- Stim `ASSERT_EQ(sim.measure_z(0), false)` → assert measurement result from `sample_batch`
- Stim `Circuit(R"CIRCUIT(...)CIRCUIT")` → `parse_lines("...")`
- Tests using `TEST_EACH_WORD_SIZE_W` → single Rust test (rstim doesn't have word-size variants)

**Skip these tests** (features not in rstim):
- to_vector_sim, to_state_vector, to_state_vector_endian, to_state_vector_canonical (no vector sim)
- peek_bloch, peek_observable_expectation, peek_x (no peek API)
- postselect_x/y/z, postselect_observable (no postselection)
- apply_tableau, measure_pauli_string (no direct tableau API)
- set_num_qubits, set_num_qubits_reduce_random, set_num_qubits_reduce_preserves_scrambled_stabilizers, amortized_resizing (no resize API)
- sample_circuit_mutates_rng_state, sample_stream_mutates_rng_state (RNG internals)
- quantum_cannot_control_classical (no classical control distinction)
- big_determinism (implementation detail)
- s_state_distillation_low_depth, s_state_distillation_low_space (complex protocols, already tested implicitly)

**Expected:** ~40 tests after filtering skips and deduplication.

**Step 1:** Read the Stim test file and all existing rstim executor/frame_sim tests to identify exact overlaps.

**Step 2:** Write all non-duplicate tests in `stim_tableau_simulator.rs`. Each test should be self-contained with its own circuit string.

**Step 3:** Run `cargo test -p rstim stim_tableau` and fix any failures.

**Step 4:** Commit:
```bash
git add rstim/tests/stim_tableau_simulator.rs
git commit -m "test: port Stim tableau_simulator tests to rstim"
```

---

### Task 2: Port error_analyzer.test.cc (48 tests)

**Files:**
- Read: `/Users/nzy/rcode/rstim/Stim/src/stim/simulators/error_analyzer.test.cc`
- Read: `/Users/nzy/rcode/rstim/rstim/tests/error_analyzer.rs` (check for overlaps)
- Read: `/Users/nzy/rcode/rstim/rstim/tests/error_analyzer_coverage.rs` (check for overlaps)
- Create: `/Users/nzy/rcode/rstim/rstim/tests/stim_error_analyzer.rs`

**What to port:** All 48 tests. Key categories:
- Basic DEM extraction: circuit_to_detector_error_model, reversed_operation_order
- Noise analysis: noisy_measurement_mx/my/mz, noisy_measurement_mrx/mry/mrz
- Error decomposition: decompose_error_failures, brute_force_decomp_*, is_graph_like, honeycomb_code_decomposes
- Loop folding: loop_folding, loop_folding_nested_loop, loop_folding_rep_code_circuit
- Edge cases: duplicate_records_in_detectors, too_many_symptoms, measurement_before_beginning
- Gauge: detect_gauge_observables, detect_gauge_detectors, gauge_detectors
- Composite: composite_error_analysis, pauli_channel_composite_errors
- Special ops: mpad, mxx, myy, mzz, measure_pauli_product_4body

**Translation pattern:**
- Stim `ErrorAnalyzer::circuit_to_detector_error_model(circuit, ...)` → rstim `ErrorAnalyzer::circuit_to_dem(&instrs)`
- Stim `ASSERT_EQ(dem.str(), "error(0.25) D0\n")` → `assert_eq!(dem.to_string(), "error(0.25) D0\n")`
- For `decompose_errors=true` tests → use `ErrorAnalyzer::circuit_to_dem_decomposed()`

**Skip these tests** (features not in rstim):
- heralded_erase, heralded_erase_conditional_division, heralded_pauli_channel_1 (no heralded erasure)
- else_correlated_error_block (ELSE_CORRELATED_ERROR not in rstim)
- tagged_noise (tags not in rstim)
- OBS_INCLUDE_PAULIS (not in rstim)
- runs_on_general_circuit (uses features not in rstim)
- ignore_failures (decompose_errors flag variant)

**Expected:** ~35 tests after filtering.

**Step 1-4:** Same pattern as Task 1.

**Commit:**
```bash
git add rstim/tests/stim_error_analyzer.rs
git commit -m "test: port Stim error_analyzer tests to rstim"
```

---

### Task 3: Port circuit.test.cc (50 tests)

**Files:**
- Read: `/Users/nzy/rcode/rstim/Stim/src/stim/circuit/circuit.test.cc`
- Read: `/Users/nzy/rcode/rstim/rstim/tests/parser.rs` (check for overlaps)
- Read: `/Users/nzy/rcode/rstim/rstim/tests/extended_features.rs` (check for overlaps)
- Create: `/Users/nzy/rcode/rstim/rstim/tests/stim_circuit.rs`

**What to port:** Circuit parsing, repr, validation, manipulation. Key categories:
- Parsing: from_text, parse_mpp, parse_spp, parse_spp_dag, parse_sweep_bits, parse_tag
- Validation: repeat_validation, tick_validation, detector_validation, x_error_validation, pauli_err_1/2_validation, validate_nan_probability, validate_mpad
- Counting: count_qubits, count_measurements, count_detectors_num_observables, count_sweep_bits, max_lookback, count_ticks
- String repr: str, round trip parse → to_string → parse
- Coordinates: qubit_coords, negative_float_coordinates, get_final_qubit_coords, coords_of_detector, final_coord_shift
- Repeat blocks: preserves_repetition_blocks, big_rep_count, zero_repetitions_not_allowed
- Equality: equality, approx_equals
- Misc: flattened, parse_windows_newlines, aliased_noiseless_circuit

**Translation pattern:**
- Stim `Circuit(text)` → `parse_lines(text)`
- Stim `circuit.str()` → `circuit_to_string(&instrs)`
- Stim `circuit.count_qubits()` → count from parsed instructions
- Stim validation errors → `parse_lines(bad_input).is_err()`

**Skip these tests** (API differences):
- py_get_slice (Python API)
- append_repeat_block, append_circuit, append_op_fuse, concat_fuse, concat_self_fuse, insert_circuit, insert_instruction (mutation API not in rstim)
- self_addition, addition_shares_blocks, multiplication_repeats (operator overloads)
- for_each_operation, for_each_operation_reverse (iterator API)
- assignment_copies_operations (Rust ownership handles this)
- classical_controls (not applicable)
- inverse (circuit inverse not implemented)
- generate_test_circuit_with_all_operations (Stim-specific)
- without_tags (tags not in rstim)

**Expected:** ~25 tests after filtering.

**Commit:**
```bash
git add rstim/tests/stim_circuit.rs
git commit -m "test: port Stim circuit tests to rstim"
```

---

### Task 4: Port frame_simulator.test.cc (42 tests)

**Files:**
- Read: `/Users/nzy/rcode/rstim/Stim/src/stim/simulators/frame_simulator.test.cc`
- Read: `/Users/nzy/rcode/rstim/rstim/tests/frame_sim.rs` (check for overlaps)
- Read: `/Users/nzy/rcode/rstim/rstim/tests/frame_sim_coverage.rs` (check for overlaps)
- Create: `/Users/nzy/rcode/rstim/rstim/tests/stim_frame_simulator.rs`

**What to port:** Batch sampling, noise, detection events. Key categories:
- Noise: noisy_measurement_x/y/z, noisy_measurement_reset_x/y/z, correlated_error
- Measurements: measure_pauli_product_4body, non_deterministic_pauli_product_detectors, mxxyyzz_basis/inversion, mpad
- Reset: measure_y_without_reset_doesnt_reset, resets_vs_measurements
- Batch: block_results_single_shot, block_results_triple_shot
- Classical: classical_can_control_quantum, classical_controls
- Other: runs_on_general_circuit, observable_include_paulis_rx/ry/rz

**Translation pattern:**
- Stim `FrameSimulator::sample_flipped_measurements(circuit, ...)` → rstim `sample_batch(&instrs, shots, &mut rng)`
- Compare measurement/detection statistics over many shots

**Skip these tests:**
- get_set_frame, reconfigure_for (internal API)
- bulk_operations_consistent_with_tableau_data, consistency, test_util_is_output_possible (internal consistency)
- sample_batch_measurements_writing_results_to_disk, stream_*, run_length_measurement_formats (I/O format)
- big_circuit_measurements, big_circuit_random_measurements (performance)
- record_gets_trimmed, stream_huge_case (I/O internals)
- quantum_cannot_control_classical (not applicable)
- ignores_sweep_controls_when_given_no_sweep_data (sweep internals)
- heralded_erase_*, heralded_pauli_channel_* (not in rstim)

**Expected:** ~20 tests after filtering.

**Commit:**
```bash
git add rstim/tests/stim_frame_simulator.rs
git commit -m "test: port Stim frame_simulator tests to rstim"
```

---

### Task 5: Port stabilizer tests — tableau.test.cc + pauli_string.test.cc (55 tests)

**Files:**
- Read: `/Users/nzy/rcode/rstim/Stim/src/stim/stabilizers/tableau.test.cc`
- Read: `/Users/nzy/rcode/rstim/Stim/src/stim/stabilizers/pauli_string.test.cc`
- Create: `/Users/nzy/rcode/rstim/rstim/tests/stim_stabilizers.rs`

**What to port:** Tableau algebra and Pauli string operations. Key categories:

Tableau (25 tests):
- identity, gate1, str, equality
- gate_tableau_data_vs_unitary_data, inverse_data
- eval, eval_y, apply_within
- inplace_scatter_append/prepend
- check_invariants, is_conjugation_by_pauli
- to_pauli_string, from_pauli_string
- random, expand, expand_pad, transposed_access, inverse
- then, raised_to, direct_sum

Pauli string (30 tests):
- str, equality, multiplication, identity, commutes
- sparse_str, ensure_num_qubits
- pauli_xz_to_xyz, pauli_xyz_to_xz
- after_circuit, before_circuit, after_tableau, before_tableau
- left_mul_pauli, left_mul_pauli_mul_table, right_mul_pauli_mul_table
- before_after_circuit_ignores_annotations, before_after_circuit_understands_*

**Note:** Many of these test internal Tableau/PauliString APIs that rstim may not expose directly. The subagent should check which rstim APIs exist for tableau/pauli and only port tests that can be expressed through the available API. Tests that exercise circuit-level behavior (like after_circuit, before_circuit) can use `parse_lines` + the error analyzer's sensitivity tracking.

**Skip tests needing APIs not in rstim:**
- Tests using direct tableau mutation (scatter, prepend, expand, etc.)
- Tests using direct PauliString objects if not exposed
- py_get_item, py_get_slice (Python API)
- foreign_memory (C++ specific)
- gather, swap_with_overwrite_with, scatter (memory layout)
- move_copy_assignment (C++ specific)

**Expected:** ~15-25 tests depending on available API.

**Commit:**
```bash
git add rstim/tests/stim_stabilizers.rs
git commit -m "test: port Stim stabilizer tests to rstim"
```

---

### Task 6: Port DEM + m2d + codegen tests (50 tests)

**Files:**
- Read: `/Users/nzy/rcode/rstim/Stim/src/stim/dem/detector_error_model.test.cc`
- Read: `/Users/nzy/rcode/rstim/Stim/src/stim/simulators/measurements_to_detection_events.test.cc`
- Read: `/Users/nzy/rcode/rstim/Stim/src/stim/gen/circuit_gen_params.test.cc`
- Read: `/Users/nzy/rcode/rstim/rstim/tests/dem_format.rs` (check for overlaps)
- Read: `/Users/nzy/rcode/rstim/rstim/tests/m2d.rs` (check for overlaps)
- Create: `/Users/nzy/rcode/rstim/rstim/tests/stim_dem.rs`
- Create: `/Users/nzy/rcode/rstim/rstim/tests/stim_m2d.rs`
- Create: `/Users/nzy/rcode/rstim/rstim/tests/stim_codegen.rs`

**DEM tests (27):**
- Parse/display: round_trip_str, parse, parse_windows_newlines, parse_tag
- Counting: count_detectors, count_observables, total_detector_shift
- Instructions: append_error/detector/observable_instruction, append_shift_detectors_instruction, append_block
- Manipulation: mul, imul, add, iadd, flattened
- Coordinates: get_detector_coordinates_*, final_detector_and_coord_shift, surface_code_coords_dont_infinite_loop
- Other: init_equality, dem_target_general, dem_instruction_general, rounded

**M2D tests (15):**
- single_detector_no_sweep_data, sweep_data, empty_cases
- big_shots, big_data, many_shots, many_measurements_and_detectors
- append_observables, with_error_propagation
- Format conversion tests

**Codegen tests (6):**
- append_begin_round_tick, append_unitary_1/2, append_reset, append_measure, append_measure_reset

**Skip:**
- from_file (file I/O)
- py_get_slice (Python API)
- movement (C++ move semantics)
- File format conversion tests in m2d (I/O format details)

**Expected:** ~30 tests after filtering.

**Commit:**
```bash
git add rstim/tests/stim_dem.rs rstim/tests/stim_m2d.rs rstim/tests/stim_codegen.rs
git commit -m "test: port Stim DEM, m2d, and codegen tests to rstim"
```

---

### Task 7: Push and verify

**Step 1:** Run full test suite:
```bash
cd /Users/nzy/rcode/rstim && cargo test -p rstim
```

**Step 2:** Count total tests and compare:
```bash
grep -r "#\[test\]" rstim/tests/ | wc -l
```

**Step 3:** Push:
```bash
git push origin master
```
