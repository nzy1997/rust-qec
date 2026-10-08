#!/usr/bin/env bash
set -euo pipefail

rustup toolchain install 1.93.1 --profile minimal
rustup default 1.93.1
mkdir -p drafts
python benchmarks/near_clifford/test_rust_pair_scout.py > drafts/x86-scout-provenance-normal.log 2>&1
python -O benchmarks/near_clifford/test_rust_pair_scout.py > drafts/x86-scout-provenance-optimized.log 2>&1
grep -m1 '^flags' /proc/cpuinfo > drafts/x86-scout-features.txt
grep -qw avx2 drafts/x86-scout-features.txt
grep -qw fma drafts/x86-scout-features.txt
cargo test --release --locked -p rstim --lib fused_avx2_pairs_match_independent_full_vector_gather_bits -- --nocapture > drafts/x86-scout-direct-bits.log 2>&1
cargo test --release --locked -p rstim --lib both_policies_highest_pairs_match_snapshot_gather_at_all_mask_boundaries -- --nocapture > drafts/x86-scout-highest-gather.log 2>&1
cargo test --release --locked -p rstim --lib scalar_high_multi_x_matches_gather_coefficient_and_cdf_bits_for_both_policies -- --nocapture > drafts/x86-scout-gather-cdf.log 2>&1
cargo test --release --locked -p rstim --lib scalar_rotation_preserves_frozen_coefficient_bits_for_all_phase_and_pair_cases -- --nocapture > drafts/x86-scout-frozen-bits.log 2>&1
cargo test --release --locked -p rstim --lib both_rotation_policies_match_independent_coefficient_and_cdf_bits -- --nocapture > drafts/x86-scout-both-policy-bits.log 2>&1
cargo test --release --locked -p rstim --lib near_clifford::compiled::compact_replay::tests -- --nocapture > drafts/x86-scout-compact-zero-spans.log 2>&1
cargo test --release --locked -p rstim --lib compact_noise_restores_original_typed_rows_and_carry_with_both_independent_producers -- --nocapture > drafts/x86-scout-noise-hit-masks.log 2>&1
cargo test --release --locked -p rstim --test near_clifford_postselected_counts -- --nocapture > drafts/x86-scout-public-counts.log 2>&1
grep -q 'test result: ok. 1 passed' drafts/x86-scout-direct-bits.log
grep -q 'test result: ok. 1 passed' drafts/x86-scout-highest-gather.log
grep -q 'test result: ok. 1 passed' drafts/x86-scout-gather-cdf.log
grep -q 'test result: ok. 1 passed' drafts/x86-scout-frozen-bits.log
grep -q 'test result: ok. 1 passed' drafts/x86-scout-both-policy-bits.log
grep -q 'test result: ok. 10 passed' drafts/x86-scout-public-counts.log
grep -q 'test result: ok. 3 passed' drafts/x86-scout-compact-zero-spans.log
grep -q 'test result: ok. 1 passed' drafts/x86-scout-noise-hit-masks.log
