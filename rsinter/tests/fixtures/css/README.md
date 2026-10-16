# CSS Fixtures

APM P=96 fixtures are generated from the qec-code built-in CSS export:

```sh
cargo run -p qec-code -- code css apm_kasai:p=96 hx > rsinter/tests/fixtures/css/apm_p96_hx.json
cargo run -p qec-code -- code css apm_kasai:p=96 hz > rsinter/tests/fixtures/css/apm_p96_hz.json
```

The native BP/BP-OSD baseline for these fixtures is checked by
`rsinter/tests/apm_p96_rbposd_smoke.rs`. Future relay-BP and MIP fallback
reproduction is tracked in
[`docs/apm_decoder_hierarchy.md`](../../../../docs/apm_decoder_hierarchy.md).

Quantum Tanner `toric_d4` fixtures are generated from
`qec-code/tests/fixtures/quantum_tanner/toric_d4.json`, which is the
qLDPC-derived known-answer fixture used by the `qec-code` quantum Tanner
constructor tests:

```sh
cargo run -p qec-code -- code css quantum-tanner --spec qec-code/tests/fixtures/quantum_tanner/toric_d4.json hx > rsinter/tests/fixtures/css/quantum_tanner_toric_d4_hx.json
cargo run -p qec-code -- code css quantum-tanner --spec qec-code/tests/fixtures/quantum_tanner/toric_d4.json hz > rsinter/tests/fixtures/css/quantum_tanner_toric_d4_hz.json
```

Reference chain:

- `drafts/qLDPC/src/qldpc/codes/quantum_test.py` for the toric Tanner known-answer case.
- `drafts/qLDPC/src/qldpc/codes/quantum.py` for `QTCode`.
- Upstream `https://github.com/qLDPCOrg/qLDPC`.
