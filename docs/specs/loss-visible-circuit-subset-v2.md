# Loss-Visible Circuit Subset v2

Status: **experimental input contract** (versioned). This extends the
[v1 circuit subset](loss-visible-circuit-subset-v1.md) for external atom
schedule compilers that retain native Clifford pulses and waiting-time
dephasing. It changes input capability, not the decoder's algorithm or
measured performance promise.

## Selection and compatibility

Both `rstim decode` and `rstim dataset import` accept
`--circuit-subset v1|v2`. The default remains **v1**; there is no automatic
upgrade based on circuit contents. Every instruction rejected by v1 remains
rejected when v1 is selected. Unknown version names are `invalid_arguments`
(exit 2). Both versions and the default appear in `capabilities --format json`.

Circuit subset versions are independent of dataset schemas and the
conditioned-decoding contract. Existing `rstim_decoder_dataset` schema v1
and `qude_decoder_dataset` schema v3 bundles retain their hashes, IDs, and
measurement packing. Import validates with the selected subset; decode must
select v2 again. No new manifest field is required or inferred.

Decode statistics retain schema `rustqec.decode-stats.v1` and add the
informational string field `circuit_subset` (`v1` or `v2`).

## Additional instructions

All v1 instructions and requirements remain available. The only additions are:

| Instruction | Analysis and loss semantics |
| --- | --- |
| `Z q...` | Keep the signed Clifford operation at its original position. It does not add a basis-change site. |
| `SQRT_Y q...` | Keep the signed Clifford operation; record a basis-change site immediately after it for each targeted physical wire. Its unsigned Pauli action swaps X and Z, like H; the signed noiseless reference still uses the actual pulse. |
| `CZ a b ...` | Keep the direct CZ operation; require complete, pairwise-disjoint qubit pairs within the instruction. It does not add synthetic H pulses or basis-change sites. Pauli probes propagate through CZ itself. |
| `Z_ERROR(p) q...` | Keep pure dephasing with its original parameter, targets, tag, and location in the ordinary DEM analysis. Remove it only in the separate noiseless reference/probe circuit. |

Targets of each new instruction must be plain, non-inverted qubits. Repeated
targets within a new instruction are rejected, including overlapping CZ pairs.
Normal parser validation of arguments and probability domains still applies;
DEM-level error probabilities must remain finite and below 0.5.

The extension does not add `SQRT_Y_DAG`, `ZCZ`, `Y_ERROR`, general Pauli
channels, non-Z loss-visible readout, or `REPEAT`.

## Physical locations and loss windows

The compiler preserves native pulse order and every explicit Pauli noise
location and parameter. For example, noise or a `LOSS` opportunity between
`Z` and `SQRT_Y` remains between those operations. The compiler does not
combine noisy native primitives into an ideal H/CX or substitute
depolarization for dephasing. The external producer encodes timing-dependent
noise as explicit channels; this subset does not infer pulse durations.

`LOSS` remains structural, using the existing delayed-erasure envelope
semantics: every opportunity since reset is recorded at its position in the
noiseless circuit. Native pulses retain their own positions in that circuit.
The categorical onset/candidate model and its weighting are unchanged.
Basis sites after `SQRT_Y` join existing H sites; direct CZ contributes
propagation without artificial basis sites.

`R`/`RZ` and `MRL`/`MRZL` close the physical wire's loss window and clear
its onset and basis history. A subsequent loss-visible readout needs a new
`LOSS` opportunity. Terminal `ML`/`MZL` still prohibit later use of that
wire, including any new native instruction or reset.

Every loss-visible readout retains its interleaved `flag,value` records.
Analysis uses one `MPAD` per flag, so annotations keep their indices.
Detectors and observables may reference values only. Changing a flagged value
placeholder must not change either decoder's prediction.

## Decoder boundaries

The v1 file-level, graph, resource, and output requirements continue to apply.
In particular, new gates do not enable non-graphlike ordinary Pauli effects,
observable-only ordinary effects, ambiguous logical labels on parallel
matching edges, or matching loss primitives that cannot map to an existing
edge. A circuit may pass instruction normalization and still fail these
later checks. The tiny two-wire input in issue #1182 is an instruction
capability example, not evidence that an unencoded lost logical wire is
recoverable by matching.

No-loss decoding agrees with ordinary Pauli decoding. With loss, the
[conditioned-decoding contract](delayed-erasure-conditioned-decoding-v1.md)
and the distinction between matching and categorical MLE remain unchanged.

## Example and verification

The committed
[native fixture](../../rstim/tests/fixtures/loss_visible_native_v2.stim)
and [reference](../../rstim/tests/fixtures/loss_visible_native_v2_reference.stim)
provide a deterministic three-wire conformance example. To import external
measurements and decode:

```sh
rstim dataset import --circuit native.stim --shots shots.b8 --shots-format b8 \
  --circuit-subset v2 --out dataset
rstim decode --decoder envelope-matching --circuit-subset v2 \
  --dataset dataset --out predictions.b8 --stats-out stats.json
```

Run the deterministic conformance evidence with:

```sh
cargo test --locked -p rstim --features ilp --lib native_v2
cargo test --locked -p rstim --features ilp --test unified_loss_subset_v2
```

The CLI DEM cross-check uses Stim (`stim`, or `RSTIM_TEST_STIM`).
The internal signed-Clifford/instrument oracle uses independent real
amplitudes with coherent three-qubit inputs and measurement/reset branches.
Other tests compare ordinary DEM probabilities, loss-onset/basis effects,
all eight no-loss syndromes, and all 64 three-wire flag/value rows.
Negative controls reject native inputs under default/explicit v1, malformed
or reused targets, stale loss windows, terminal wire reuse, and unsupported
graph shapes.
