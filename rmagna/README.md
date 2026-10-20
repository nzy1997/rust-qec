# rmagna

Independent Rust foundations for global-network near-Clifford simulation.
The current package provides exact scalar algebra only. Circuit compilation,
contraction and sampling are subsequent deliverables.

`ExactScalar` represents
`(a + b*omega + c*omega^2 + d*omega^3) * 2^power`, where
`omega = exp(i*pi/4)`. Coefficients are arbitrary-width signed integers, and
binary powers are signed 64-bit integers. Common powers of two are removed,
so equivalent values compare and hash equally. Arithmetic uses no floats.

```rust
use rmagna::scalar::{ExactScalar, ScalarArithmetic};

let arithmetic = ScalarArithmetic::default();
let t = arithmetic.omega(1, 0)?;
let h = arithmetic.omega(0, -1)?; // 1 / sqrt(2)
assert_eq!(arithmetic.norm_squared(&t)?, ExactScalar::one());
assert_eq!(arithmetic.multiply(&h, &h)?, arithmetic.omega(0, -2)?);
# Ok::<(), rmagna::scalar::ScalarError>(())
```

`ScalarArithmetic::new(bits)` configures the maximum bit length of canonical
coefficients and aligned addition operands (default 1,048,576 bits). This
protects against a tiny operand with a huge exponent gap allocating an enormous
shift. It is an arithmetic resource limit, not a process-memory cap: products
and real-sign comparisons use larger integer temporaries. Failed operations
return a `ScalarError`; they never truncate coefficients or approximate the
result. A cancellation may fail the alignment budget even if its final result
would fit. Exponent limits apply to the normalized result.

`real_sign` compares an exactly real value with zero by integer arithmetic,
including values too small or cancellation-sensitive for floating-point
comparison. It rejects complex values. `conjugate` preserves global phases;
`norm_squared` is exact. Rational noise weights will be represented separately
because a probability such as 1/3 is outside this scalar ring.

Run `cargo test -p rmagna`. Tests cover mixed complex coefficients, phase and
scale identities, destructive interference, tiny probability differences,
large Pell convergents, canonical equality, resource failures and exponent
boundaries. This is independently authored Rust code; the pinned Magna
implementation used for later comparisons is recorded in
`benchmarks/magna_reproduction/reference.json` at the workspace root.

The committed scalar witness file contains numerical outputs from that pinned
reference, including its source-manifest and native-extension hashes. Regenerate
it with the reference environment, leaving the existing file untouched:

```sh
/absolute/path/to/reference-env/bin/python -I \
  benchmarks/magna_reproduction/scalar_vectors.py \
  --upstream /absolute/path/to/magna --out /absolute/path/to/new-witnesses.json
```

The generator refuses to overwrite an existing destination and verifies the
pinned source before and after evaluating the fixed 300-bit operand set.
