# Quantum Tanner CLI Workflow

This workflow starts from a committed quantum Tanner spec, exports the
constructed CSS checks as `sparse_rows`, and verifies the CSS distance.

Run the commands from the repository root. The example uses the committed
`toric_d4` fixture:

```text
qec-code/tests/fixtures/quantum_tanner/toric_d4.json
```

## Boundary

Rust consumes explicit finite-group specs. It validates and constructs from the
finite multiplication table, generator indices, and local GF(2) code matrices in
the spec. It does not search for groups, does not call GAP or Oscar, and does
not call qLDPC Python, Julia/Oscar, or other external construction code at
runtime.

The middle shape is intentional: use external tools or checked-in fixtures to
prepare explicit finite data, then hand that data to `qec-code` for deterministic
CSS matrix export and distance checks.

## Inspect The Fixture

The fixture records a `Z4 x Z4` no-cover left-right Cayley-complex example with
expected CSS metadata `n = 16`, `k = 2`, and expected distance `4`.

<!-- quantum_tanner_cli:inspect_toric_d4_fixture -->
```bash
sed -n '1,80p' qec-code/tests/fixtures/quantum_tanner/toric_d4.json
```

## Export `Hx` And `Hz`

These commands write ordinary `sparse_rows` JSON matrices.

<!-- quantum_tanner_cli:toric_d4_commands -->
```bash
mkdir -p target/qec-code-workflow
cargo run -q -p qec-code -- code css quantum-tanner --spec qec-code/tests/fixtures/quantum_tanner/toric_d4.json hx > target/qec-code-workflow/toric_d4_hx.json
cargo run -q -p qec-code -- code css quantum-tanner --spec qec-code/tests/fixtures/quantum_tanner/toric_d4.json hz > target/qec-code-workflow/toric_d4_hz.json
cargo run -q -p qec-code -- code css-distance exact --hx target/qec-code-workflow/toric_d4_hx.json --hz target/qec-code-workflow/toric_d4_hz.json --json
cargo run -q -p qec-code -- code css-distance exact --quantum-tanner-spec qec-code/tests/fixtures/quantum_tanner/toric_d4.json --json
```

The final command should return JSON with:

```json
{
  "status": "completed",
  "distance": 4
}
```

The `--hx`/`--hz` command verifies the exported files. The
`--quantum-tanner-spec` command verifies the same code directly from the spec.

## Negative Control

This invalid fixture removes an inverse generator from `A`. It should exit
non-zero before emitting a valid matrix or distance result.

<!-- quantum_tanner_cli:invalid_spec_command -->
```bash
cargo run -q -p qec-code -- code css quantum-tanner --spec qec-code/tests/fixtures/quantum_tanner/invalid_non_symmetric_a.json hx
```

## References And Licenses

The quantum Tanner construction vocabulary and fixture expectations were checked
against these references:

- local qLDPC reference implementation:
  `drafts/qLDPC/src/qldpc/codes/quantum.py`
- local qLDPC Cayley-complex reference:
  `drafts/qLDPC/src/qldpc/objects.py`
- local qLDPC toric Tanner test:
  `drafts/qLDPC/src/qldpc/codes/quantum_test.py`
- upstream qLDPC: <https://github.com/qLDPCOrg/qLDPC>
- QuantumExpanders.jl:
  <https://github.com/QuantumSavory/QuantumExpanders.jl>
- qTanner data/code repository for future import ideas:
  <https://github.com/RebKatRad/qTanner>

The local qLDPC clone used as a reference is Apache-2.0. Use the other
repositories according to their own licenses; treat them as
reference-only unless a compatible license is confirmed.
