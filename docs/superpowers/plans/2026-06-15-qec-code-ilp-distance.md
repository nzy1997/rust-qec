# QEC Code ILP Distance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add feature-gated ILP-backed exact distance solving to `qec-code` for general stabilizer codes, while extracting solver plumbing into a new shared crate that `rilpqec` also uses.

**Architecture:** Introduce a new small workspace crate `qec-ilp-core` for backend-agnostic binary ILP model building, linear constraints, backend config, and `HiGHS`/optional `Gurobi` solver bindings. Keep lowering logic domain-specific: `rilpqec` lowers DEM decoding problems into the shared model and reuses one compiled backend by mutating row RHS values, while `qec-code` lowers stabilizer-code distance into the shared model using canonical logical bases and a single global minimum-weight formulation.

**Tech Stack:** Rust 2024 workspace; `highs` and optional `gurobi`; existing `qec-code` GF(2)/symplectic helpers; `cargo test`; feature-gated crate dependencies.

---

## File Structure

- Modify `Cargo.toml`: add the new workspace member `qec-ilp-core`.
- Create `qec-ilp-core/Cargo.toml`: declare shared ILP crate dependencies and features.
- Create `qec-ilp-core/src/lib.rs`: export shared config, error, model, and backend APIs.
- Create `qec-ilp-core/src/config.rs`: move shared backend config and kind types here.
- Create `qec-ilp-core/src/error.rs`: define shared solver/config/model errors.
- Create `qec-ilp-core/src/model.rs`: define a solver-agnostic binary ILP model with general linear constraints and solved-assignment result helpers.
- Create `qec-ilp-core/src/backend/mod.rs`: define the shared backend trait and backend builder.
- Create `qec-ilp-core/src/backend/highs.rs`: move and adapt the shared `HiGHS` implementation.
- Create `qec-ilp-core/src/backend/gurobi.rs`: move and adapt the shared optional `Gurobi` implementation.
- Create `qec-ilp-core/tests/highs_backend.rs`: solver integration tests for the shared backend API, including mutable RHS support.
- Create `qec-ilp-core/tests/backend_auto.rs`: auto/gurobi fallback behavior tests in the shared crate.
- Modify `rilpqec/Cargo.toml`: replace direct `highs`/`gurobi` config dependencies with `qec-ilp-core`.
- Modify `rilpqec/src/lib.rs`: re-export shared backend config types where needed and stop exporting local backend modules directly if no longer necessary.
- Modify `rilpqec/src/config.rs`: either delete this file after migration or reduce it to type aliases/re-exports.
- Modify `rilpqec/src/error.rs`: keep DEM-specific errors, add conversion from shared backend errors, and remove duplicated backend-unavailable/backend-specific variants if they are moved.
- Modify `rilpqec/src/problem.rs`: keep DEM-specific lowered problem type and add conversion into the shared model.
- Modify `rilpqec/src/backend/mod.rs`: delete after migration or replace with a thin compatibility wrapper around `qec_ilp_core::backend`.
- Modify `rilpqec/src/backend/highs.rs`: delete after migration.
- Modify `rilpqec/src/backend/gurobi.rs`: delete after migration.
- Modify `rilpqec/src/decoder.rs`: call the shared backend builder on the converted shared model.
- Modify `rilpqec/tests/highs_backend.rs`: switch to the shared backend surface while preserving current decode behavior assertions.
- Modify `rilpqec/tests/backend_auto.rs`: keep current fallback expectations after the refactor.
- Modify `qec-code/Cargo.toml`: add optional dependency on `qec-ilp-core` and new features such as `distance-ilp-highs`.
- Modify `qec-code/src/error.rs`: add distance-solver and unsupported-configuration errors.
- Modify `qec-code/src/lib.rs`: expose any new internal modules needed by distance lowering.
- Modify `qec-code/src/distance.rs`: refactor into dispatcher plus exhaustive path plus ILP path.
- Create `qec-code/src/distance_ilp.rs`: lower stabilizer-code distance to the shared ILP model and reconstruct the witness.
- Create `qec-code/tests/distance_ilp_lowering.rs`: lowering-only tests that do not require a live solver.
- Modify `qec-code/tests/logical_distance.rs`: add general-`k`, feature-gated ILP, and unsupported-size assertions.
- Modify `qec-code/tests/cli.rs`: preserve CLI output with the new solve path and new error wording where relevant.

---

### Task 1: Create `qec-ilp-core` And Migrate Shared Backend Types

**Files:**
- Modify: `Cargo.toml`
- Create: `qec-ilp-core/Cargo.toml`
- Create: `qec-ilp-core/src/lib.rs`
- Create: `qec-ilp-core/src/config.rs`
- Create: `qec-ilp-core/src/error.rs`
- Create: `qec-ilp-core/src/model.rs`
- Create: `qec-ilp-core/src/backend/mod.rs`
- Create: `qec-ilp-core/src/backend/highs.rs`
- Create: `qec-ilp-core/src/backend/gurobi.rs`
- Create: `qec-ilp-core/tests/highs_backend.rs`
- Create: `qec-ilp-core/tests/backend_auto.rs`

- [ ] **Step 1: Write the failing shared-backend tests**

Create `qec-ilp-core/tests/highs_backend.rs`:

```rust
use qec_ilp_core::backend::build_binary_backend;
use qec_ilp_core::{
    BackendConfig, BackendKind, BinaryIlpConfig, BinaryIlpModel, ConstraintSense,
    LinearConstraint, ModelVar,
};

fn single_column_flip_model() -> BinaryIlpModel {
    BinaryIlpModel {
        binary_vars: vec![ModelVar {
            name: "e0".into(),
            objective: 1.0,
            lower: 0.0,
            upper: 1.0,
        }],
        integer_vars: vec![ModelVar {
            name: "t0".into(),
            objective: 0.0,
            lower: 0.0,
            upper: f64::INFINITY,
        }],
        constraints: vec![LinearConstraint {
            name: "row0".into(),
            sense: ConstraintSense::Eq,
            binary_terms: vec![(0, 1.0)],
            integer_terms: vec![(0, -2.0)],
            rhs: 1.0,
        }],
        solution_binary_prefix_len: 1,
    }
}

#[test]
fn highs_solves_a_single_binary_parity_problem() {
    let mut backend = build_binary_backend(
        &single_column_flip_model(),
        &BinaryIlpConfig {
            backend: BackendConfig {
                kind: BackendKind::Highs,
                time_limit_seconds: None,
                mip_gap: None,
                threads: Some(1),
                verbose: false,
            },
        },
    )
    .unwrap();

    let solution = backend.solve().unwrap();

    assert_eq!(solution.binary_values, vec![true]);
}

#[test]
fn highs_respects_optional_solver_settings() {
    let mut backend = build_binary_backend(
        &single_column_flip_model(),
        &BinaryIlpConfig {
            backend: BackendConfig {
                kind: BackendKind::Highs,
                time_limit_seconds: Some(1.0),
                mip_gap: Some(0.05),
                threads: Some(1),
                verbose: false,
            },
        },
    )
    .unwrap();

    let solution = backend.solve().unwrap();

    assert_eq!(solution.binary_values, vec![true]);
}

#[test]
fn highs_backend_supports_mutating_one_rhs_between_solves() {
    let mut backend = build_binary_backend(
        &single_column_flip_model(),
        &BinaryIlpConfig {
            backend: BackendConfig {
                kind: BackendKind::Highs,
                time_limit_seconds: None,
                mip_gap: None,
                threads: Some(1),
                verbose: false,
            },
        },
    )
    .unwrap();

    assert_eq!(backend.solve().unwrap().binary_values, vec![true]);
    backend.set_rhs(0, 0.0).unwrap();
    assert_eq!(backend.solve().unwrap().binary_values, vec![false]);
}
```

Create `qec-ilp-core/tests/backend_auto.rs`:

```rust
use qec_ilp_core::backend::build_binary_backend;
use qec_ilp_core::{
    BackendConfig, BackendKind, BinaryIlpConfig, BinaryIlpModel, ConstraintSense,
    LinearConstraint, ModelVar,
};
#[cfg(not(feature = "gurobi"))]
use qec_ilp_core::BinaryIlpError;

fn simple_model() -> BinaryIlpModel {
    BinaryIlpModel {
        binary_vars: vec![ModelVar {
            name: "x".into(),
            objective: 1.0,
            lower: 0.0,
            upper: 1.0,
        }],
        integer_vars: vec![ModelVar {
            name: "t".into(),
            objective: 0.0,
            lower: 0.0,
            upper: f64::INFINITY,
        }],
        constraints: vec![LinearConstraint {
            name: "parity".into(),
            sense: ConstraintSense::Eq,
            binary_terms: vec![(0, 1.0)],
            integer_terms: vec![(0, -2.0)],
            rhs: 1.0,
        }],
        solution_binary_prefix_len: 1,
    }
}

#[test]
fn auto_backend_falls_back_to_highs() {
    let mut backend = build_binary_backend(&simple_model(), &BinaryIlpConfig::default()).unwrap();

    let solution = backend.solve().unwrap();

    assert_eq!(solution.binary_values, vec![true]);
}

#[cfg(not(feature = "gurobi"))]
#[test]
fn explicit_gurobi_selection_reports_unavailable_without_feature() {
    let err = build_binary_backend(
        &simple_model(),
        &BinaryIlpConfig {
            backend: BackendConfig {
                kind: BackendKind::Gurobi,
                time_limit_seconds: None,
                mip_gap: None,
                threads: None,
                verbose: false,
            },
        },
    )
    .unwrap_err();

    assert_eq!(
        err,
        BinaryIlpError::BackendUnavailable {
            requested: BackendKind::Gurobi,
        }
    );
}
```

- [ ] **Step 2: Run the new shared-backend tests**

Run:

```bash
cargo test -p qec-ilp-core highs_backend backend_auto
```

Expected: FAIL because the `qec-ilp-core` crate and its exported shared solver types do not exist yet.

- [ ] **Step 3: Add the new workspace crate and shared config/error/model surfaces**

Modify the workspace member list in `Cargo.toml`:

```toml
members = [
    "qec-code",
    "rstim",
    "rsinter",
    "rbposd",
    "rmatching",
    "rilpqec",
    "qec-ilp-core",
    "benchmarks/surface_decoder_compare/rust_bridge",
]
```

Create `qec-ilp-core/Cargo.toml`:

```toml
[package]
name = "qec-ilp-core"
version = "0.1.0"
edition = "2024"

[features]
default = []
gurobi = ["dep:gurobi"]

[dependencies]
thiserror = "2"
highs = "2.1.0"
highs-sys = "1.14.2"
gurobi = { version = "0.3.4", optional = true }
```

Create `qec-ilp-core/src/lib.rs`:

```rust
pub mod backend;
pub mod config;
pub mod error;
pub mod model;

pub use config::{BackendConfig, BackendKind, BinaryIlpConfig};
pub use error::BinaryIlpError;
pub use model::{
    BinaryIlpModel, ConstraintSense, LinearConstraint, ModelSolution, ModelVar,
};
```

Create `qec-ilp-core/src/config.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Auto,
    Highs,
    Gurobi,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BackendConfig {
    pub kind: BackendKind,
    pub time_limit_seconds: Option<f64>,
    pub mip_gap: Option<f64>,
    pub threads: Option<u32>,
    pub verbose: bool,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            kind: BackendKind::Auto,
            time_limit_seconds: None,
            mip_gap: None,
            threads: None,
            verbose: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BinaryIlpConfig {
    pub backend: BackendConfig,
}
```

Create `qec-ilp-core/src/error.rs`:

```rust
use thiserror::Error;

use crate::config::BackendKind;

#[derive(Debug, Error, PartialEq)]
pub enum BinaryIlpError {
    #[error("model row references an unknown binary variable index {0}")]
    UnknownBinaryVar(usize),
    #[error("model row references an unknown integer variable index {0}")]
    UnknownIntegerVar(usize),
    #[error("no ILP backend is available for kind {requested:?}")]
    BackendUnavailable { requested: BackendKind },
    #[error("HiGHS backend error: {0}")]
    Highs(String),
    #[cfg(feature = "gurobi")]
    #[error("Gurobi backend error: {0}")]
    Gurobi(String),
}
```

Create `qec-ilp-core/src/model.rs`:

```rust
use crate::error::BinaryIlpError;

#[derive(Debug, Clone, PartialEq)]
pub struct ModelVar {
    pub name: String,
    pub objective: f64,
    pub lower: f64,
    pub upper: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintSense {
    Eq,
    Le,
    Ge,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinearConstraint {
    pub name: String,
    pub sense: ConstraintSense,
    pub binary_terms: Vec<(usize, f64)>,
    pub integer_terms: Vec<(usize, f64)>,
    pub rhs: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BinaryIlpModel {
    pub binary_vars: Vec<ModelVar>,
    pub integer_vars: Vec<ModelVar>,
    pub constraints: Vec<LinearConstraint>,
    pub solution_binary_prefix_len: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelSolution {
    pub binary_values: Vec<bool>,
}

impl BinaryIlpModel {
    pub fn validate(&self) -> Result<(), BinaryIlpError> {
        for row in &self.constraints {
            for &(index, _) in &row.binary_terms {
                if index >= self.binary_vars.len() {
                    return Err(BinaryIlpError::UnknownBinaryVar(index));
                }
            }
            for &(index, _) in &row.integer_terms {
                if index >= self.integer_vars.len() {
                    return Err(BinaryIlpError::UnknownIntegerVar(index));
                }
            }
        }
        if self.solution_binary_prefix_len > self.binary_vars.len() {
            return Err(BinaryIlpError::UnknownBinaryVar(self.solution_binary_prefix_len));
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Add the shared backend trait and move the `HiGHS`/`Gurobi` implementations**

Create `qec-ilp-core/src/backend/mod.rs`:

```rust
#[cfg(feature = "gurobi")]
mod gurobi;
mod highs;

use crate::config::{BackendKind, BinaryIlpConfig};
use crate::error::BinaryIlpError;
use crate::model::{BinaryIlpModel, ModelSolution};

pub trait BinaryBackend {
    fn solve(&mut self) -> Result<ModelSolution, BinaryIlpError>;
    fn set_rhs(&mut self, row: usize, rhs: f64) -> Result<(), BinaryIlpError>;
}

pub fn build_binary_backend(
    model: &BinaryIlpModel,
    config: &BinaryIlpConfig,
) -> Result<Box<dyn BinaryBackend>, BinaryIlpError> {
    model.validate()?;
    match config.backend.kind {
        BackendKind::Highs => Ok(Box::new(highs::HighsBinaryBackend::new(model, config)?)),
        BackendKind::Auto => build_auto_backend(model, config),
        BackendKind::Gurobi => build_gurobi_backend(model, config),
    }
}

fn build_auto_backend(
    model: &BinaryIlpModel,
    config: &BinaryIlpConfig,
) -> Result<Box<dyn BinaryBackend>, BinaryIlpError> {
    #[cfg(feature = "gurobi")]
    if let Ok(backend) = gurobi::GurobiBinaryBackend::new(model, config) {
        return Ok(Box::new(backend));
    }

    Ok(Box::new(highs::HighsBinaryBackend::new(model, config)?))
}

fn build_gurobi_backend(
    model: &BinaryIlpModel,
    config: &BinaryIlpConfig,
) -> Result<Box<dyn BinaryBackend>, BinaryIlpError> {
    #[cfg(feature = "gurobi")]
    {
        return Ok(Box::new(gurobi::GurobiBinaryBackend::new(model, config)?));
    }

    #[cfg(not(feature = "gurobi"))]
    {
        let _ = (model, config);
        Err(BinaryIlpError::BackendUnavailable {
            requested: BackendKind::Gurobi,
        })
    }
}
```

Create `qec-ilp-core/src/backend/highs.rs` by adapting the current `rilpqec` implementation so it:

- consumes `BinaryIlpModel`
- allocates rows from `model.constraints`
- maps `ConstraintSense::Eq`, `ConstraintSense::Le`, and `ConstraintSense::Ge` to solver row bounds
- adds binary columns from `model.binary_vars`
- adds integer columns from `model.integer_vars`
- supports changing a row RHS between solves via `set_rhs`
- returns the first `solution_binary_prefix_len` columns as `ModelSolution`

Use this exact struct and constructor signature:

```rust
#[derive(Debug)]
pub struct HighsBinaryBackend {
    model: Option<Model>,
    row_senses: Vec<qec_ilp_core::ConstraintSense>,
    solution_binary_prefix_len: usize,
}

impl HighsBinaryBackend {
    pub fn new(
        problem: &BinaryIlpModel,
        config: &BinaryIlpConfig,
    ) -> Result<Self, BinaryIlpError> {
        // adapt the current rilpqec highs model build here, using row bounds
        // derived from each constraint sense and rhs
    }
}
```

Inside `solve`, return:

```rust
Ok(ModelSolution {
    binary_values: columns[..self.solution_binary_prefix_len]
        .iter()
        .map(|&value| value > 0.5)
        .collect(),
})
```

Create `qec-ilp-core/src/backend/gurobi.rs` by adapting the current `rilpqec` implementation to the same `BinaryIlpModel`/`ModelSolution` surface.

Implement `set_rhs` in both backends with these semantics:

```rust
fn set_rhs(&mut self, row: usize, rhs: f64) -> Result<(), BinaryIlpError>;
```

- for `Eq`, update the row to `rhs == rhs`
- for `Le`, update the row upper bound to `rhs`
- for `Ge`, update the row lower bound to `rhs`

- [ ] **Step 5: Run the shared-backend tests again**

Run:

```bash
cargo test -p qec-ilp-core highs_backend backend_auto
```

Expected: PASS.

- [ ] **Step 6: Commit the shared backend crate**

```bash
git add Cargo.toml qec-ilp-core
git commit -m "feat: add shared qec ILP core crate"
```

---

### Task 2: Migrate `rilpqec` To `qec-ilp-core` Without Changing Decode Behavior

**Files:**
- Modify: `rilpqec/Cargo.toml`
- Modify: `rilpqec/src/lib.rs`
- Modify: `rilpqec/src/config.rs`
- Modify: `rilpqec/src/error.rs`
- Modify: `rilpqec/src/problem.rs`
- Modify: `rilpqec/src/decoder.rs`
- Delete: `rilpqec/src/backend/mod.rs`
- Delete: `rilpqec/src/backend/highs.rs`
- Delete: `rilpqec/src/backend/gurobi.rs`
- Modify: `rilpqec/tests/highs_backend.rs`
- Modify: `rilpqec/tests/backend_auto.rs`

- [ ] **Step 1: Write the failing conversion and decode-regression tests**

Add this test to `rilpqec/tests/highs_backend.rs`:

```rust
use qec_ilp_core::BinaryIlpModel;

#[test]
fn lowered_dem_problem_converts_to_shared_binary_model() {
    let dem = DetectorErrorModel::parse("error(0.1) D0 L0\nerror(0.2) D1\n").unwrap();
    let problem = lower_dem_to_problem(&dem).unwrap();

    let model: BinaryIlpModel = problem.to_binary_ilp_model().unwrap();

    assert_eq!(model.binary_vars.len(), 2);
    assert_eq!(model.integer_vars.len(), 2);
    assert_eq!(model.constraints.len(), 2);
    assert_eq!(model.solution_binary_prefix_len, 2);
}
```

Add this test to `rilpqec/tests/backend_auto.rs`:

```rust
#[test]
fn default_decoder_config_reexports_shared_auto_backend_kind() {
    let config = IlpDecoderConfig::default();

    assert_eq!(config.backend.kind, BackendKind::Auto);
}
```

- [ ] **Step 2: Run the `rilpqec` backend tests to verify they fail for the new API**

Run:

```bash
cargo test -p rilpqec highs_backend backend_auto
```

Expected: FAIL because `LoweredDemProblem::to_binary_ilp_model` and the shared backend re-exports do not exist.

- [ ] **Step 3: Move `rilpqec` onto the shared config/error/model path**

Modify `rilpqec/Cargo.toml`:

```toml
[features]
default = []
gurobi = ["qec-ilp-core/gurobi"]

[dependencies]
rstim = { path = "../rstim" }
thiserror = "2"
qec-ilp-core = { path = "../qec-ilp-core" }
```

Update `rilpqec/src/lib.rs`:

```rust
pub mod config;
pub mod decoder;
pub mod error;
pub mod lowering;
pub mod problem;

pub use config::{BackendConfig, BackendKind, IlpDecoderConfig};
pub use decoder::IlpDemDecoder;
pub use error::IlpDecodeError;
pub use lowering::lower_dem_to_problem;
pub use problem::{ColumnTerm, LoweredDemProblem};
```

Update `rilpqec/src/config.rs` to become shared-type re-exports:

```rust
pub use qec_ilp_core::{BackendConfig, BackendKind};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct IlpDecoderConfig {
    pub backend: BackendConfig,
}
```

Update `rilpqec/src/error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum IlpDecodeError {
    #[error("DEM probability must lie in [0, 1], got {0}")]
    InvalidProbability(f64),
    #[error("detector width mismatch: expected {expected}, got {actual}")]
    DetectorWidthMismatch { expected: usize, actual: usize },
    #[error("packed detection buffer length mismatch: expected {expected}, got {actual}")]
    PackedDetectionsLengthMismatch { expected: usize, actual: usize },
    #[error("correction width mismatch: expected {expected}, got {actual}")]
    CorrectionWidthMismatch { expected: usize, actual: usize },
    #[error("observable width mismatch: expected {expected}, got {actual}")]
    ObservableWidthMismatch { expected: usize, actual: usize },
    #[error(transparent)]
    Backend(#[from] qec_ilp_core::BinaryIlpError),
}
```

Update all tests in `rilpqec/tests/backend_auto.rs` that currently compare to local backend errors so they compare to:

```rust
IlpDecodeError::Backend(qec_ilp_core::BinaryIlpError::BackendUnavailable {
    requested: BackendKind::Gurobi,
})
```

- [ ] **Step 4: Convert DEM problems into the shared model and call the shared backend**

Add this method to `rilpqec/src/problem.rs`:

```rust
impl LoweredDemProblem {
    pub fn to_binary_ilp_model(&self) -> Result<qec_ilp_core::BinaryIlpModel, IlpDecodeError> {
        let binary_vars = self
            .columns
            .iter()
            .enumerate()
            .map(|(index, column)| qec_ilp_core::ModelVar {
                name: format!("e_{index}"),
                objective: column.weight,
                lower: 0.0,
                upper: 1.0,
            })
            .collect::<Vec<_>>();

        let integer_vars = (0..self.num_detectors)
            .map(|row| qec_ilp_core::ModelVar {
                name: format!("a_{row}"),
                objective: 0.0,
                lower: 0.0,
                upper: f64::INFINITY,
            })
            .collect::<Vec<_>>();

        let constraints = (0..self.num_detectors)
            .map(|row| qec_ilp_core::LinearConstraint {
                name: format!("det_{row}"),
                sense: qec_ilp_core::ConstraintSense::Eq,
                binary_terms: self
                    .columns
                    .iter()
                    .enumerate()
                    .filter_map(|(index, column)| {
                        column.detectors.contains(&row).then_some((index, 1.0))
                    })
                    .collect(),
                integer_terms: vec![(row, -2.0)],
                rhs: 0.0,
            })
            .collect::<Vec<_>>();

        Ok(qec_ilp_core::BinaryIlpModel {
            binary_vars,
            integer_vars,
            constraints,
            solution_binary_prefix_len: self.columns.len(),
        })
    }
}
```

Update `rilpqec/src/decoder.rs` to replace:

```rust
use crate::backend::build_batch_backend;
```

with:

```rust
use qec_ilp_core::backend::build_binary_backend;
use qec_ilp_core::BinaryIlpConfig;
```

Then replace the backend construction and solve path inside `decode_batch_bit_packed` with:

```rust
let base_model = self.problem.to_binary_ilp_model()?;
let mut backend = build_binary_backend(
    &base_model,
    &BinaryIlpConfig {
        backend: self.config.backend.clone(),
    },
)?;
```

and:

```rust
let correction = backend.solve()?.binary_values;
```

Before each solve, set row RHS values in the compiled backend:

```rust
for (row, (&bit, &forced)) in syndrome.iter().zip(&self.problem.forced_syndrome).enumerate() {
    let rhs = if bit ^ forced { 1.0 } else { 0.0 };
    backend.set_rhs(row, rhs)?;
}
```

This preserves the current compiled-model reuse behavior instead of rebuilding one model per shot.

- [ ] **Step 5: Run the `rilpqec` tests after migration**

Run:

```bash
cargo test -p rilpqec
```

Expected: PASS.

- [ ] **Step 6: Commit the `rilpqec` migration**

```bash
git add rilpqec
git commit -m "refactor: move rilpqec onto shared ILP core"
```

---

### Task 3: Add `qec-code` ILP Lowering And Lowering-Only Tests

**Files:**
- Modify: `qec-code/Cargo.toml`
- Modify: `qec-code/src/lib.rs`
- Modify: `qec-code/src/error.rs`
- Create: `qec-code/src/distance_ilp.rs`
- Create: `qec-code/tests/distance_ilp_lowering.rs`

- [ ] **Step 1: Write the failing lowering tests**

Create `qec-code/tests/distance_ilp_lowering.rs`:

```rust
use qec_code::codes::steane::Steane;
use qec_code::distance_ilp::lower_distance_problem;
use qec_code::{Pauli, StabilizerCode};

fn pauli(n: usize, x_support: &[usize], z_support: &[usize]) -> Pauli {
    let mut x = vec![0; n];
    let mut z = vec![0; n];
    for &qubit in x_support {
        x[qubit] = 1;
    }
    for &qubit in z_support {
        z[qubit] = 1;
    }
    Pauli::from_xz_bits(x, z).unwrap()
}

#[test]
fn steane_distance_problem_has_expected_variable_shape() {
    let steane = Steane::new().unwrap();
    let lowered = lower_distance_problem(steane.code()).unwrap();

    assert_eq!(lowered.model.binary_vars.len(), 6 + 2 + 14 + 7);
    assert_eq!(lowered.model.integer_vars.len(), 14);
    assert_eq!(lowered.model.constraints.len(), 14 + 1 + (7 * 3));
    assert_eq!(lowered.model.solution_binary_prefix_len, 6 + 2 + 14 + 7);
}

#[test]
fn multi_logical_code_gets_one_nonzero_logical_constraint() {
    let code = StabilizerCode::from_stabilizers(
        4,
        vec![pauli(4, &[], &[0]), pauli(4, &[], &[1])],
    )
    .unwrap();

    let lowered = lower_distance_problem(&code).unwrap();
    let logical_constraint = lowered
        .nonzero_logical_constraint_row
        .as_ref()
        .expect("nonzero logical row");

    assert_eq!(logical_constraint.binary_terms.len(), 4);
    assert_eq!(logical_constraint.sense, qec_ilp_core::ConstraintSense::Ge);
    assert_eq!(logical_constraint.rhs, 1.0);
}

#[test]
fn zero_logical_qubit_code_is_rejected_before_lowering() {
    let code = StabilizerCode::from_stabilizers(
        2,
        vec![
            Pauli::from_xz_bits(vec![1, 0], vec![0, 0]).unwrap(),
            Pauli::from_xz_bits(vec![0, 0], vec![0, 1]).unwrap(),
        ],
    )
    .unwrap();

    let err = lower_distance_problem(&code).unwrap_err();

    assert_eq!(err.to_string(), "distance witness not found");
}
```

- [ ] **Step 2: Run the lowering tests to verify they fail**

Run:

```bash
cargo test -p qec-code distance_ilp_lowering
```

Expected: FAIL because the `distance_ilp` module and `lower_distance_problem` do not exist.

- [ ] **Step 3: Add feature wiring and distance-solver errors**

Modify `qec-code/Cargo.toml`:

```toml
[features]
default = []
distance-ilp-highs = ["dep:qec-ilp-core"]

[dependencies]
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
qec-ilp-core = { path = "../qec-ilp-core", optional = true }
```

Modify `qec-code/src/error.rs` to add:

```rust
    #[error("distance computation is unsupported for {n} qubits in the current configuration: {reason}")]
    DistanceComputationUnsupported { n: usize, reason: String },
    #[error("ILP backend is unavailable: {0}")]
    IlpBackendUnavailable(String),
    #[error("ILP solve failed: {0}")]
    IlpSolveFailed(String),
    #[error("ILP model is infeasible for a code with logical qubits")]
    IlpInfeasible,
```

and add:

```rust
#[cfg(feature = "distance-ilp-highs")]
impl From<qec_ilp_core::BinaryIlpError> for QecError {
    fn from(value: qec_ilp_core::BinaryIlpError) -> Self {
        match value {
            qec_ilp_core::BinaryIlpError::BackendUnavailable { requested } => {
                Self::IlpBackendUnavailable(format!("{requested:?}"))
            }
            other => Self::IlpSolveFailed(other.to_string()),
        }
    }
}
```

Modify `qec-code/src/lib.rs`:

```rust
pub mod distance;
#[cfg(feature = "distance-ilp-highs")]
pub mod distance_ilp;
```

- [ ] **Step 4: Add the stabilizer-distance lowering module**

Create `qec-code/src/distance_ilp.rs`:

```rust
use crate::error::{QecError, Result};
use crate::gf2::BinaryRow;
use crate::{Pauli, StabilizerCode};

pub struct LoweredDistanceProblem {
    pub model: qec_ilp_core::BinaryIlpModel,
    pub stabilizer_var_count: usize,
    pub logical_var_count: usize,
    pub symplectic_var_offset: usize,
    pub qubit_activity_offset: usize,
    pub nonzero_logical_constraint_row: Option<qec_ilp_core::LinearConstraint>,
}

pub fn lower_distance_problem(code: &StabilizerCode) -> Result<LoweredDistanceProblem> {
    if code.num_logical_qubits() == 0 {
        return Err(QecError::DistanceWitnessNotFound);
    }

    let stabilizer_rows = code.stabilizer_rows();
    let basis = code.canonical_logical_basis()?;
    let logical_rows = basis
        .logical_x
        .iter()
        .chain(&basis.logical_z)
        .map(Pauli::to_symplectic_row)
        .collect::<Vec<_>>();
    let width = code.n() * 2;
    let stabilizer_var_count = stabilizer_rows.len();
    let logical_var_count = logical_rows.len();
    let symplectic_var_offset = stabilizer_var_count + logical_var_count;
    let qubit_activity_offset = symplectic_var_offset + width;

    let mut binary_vars = Vec::new();
    for i in 0..stabilizer_var_count {
        binary_vars.push(qec_ilp_core::ModelVar {
            name: format!("lambda_{i}"),
            objective: 0.0,
            lower: 0.0,
            upper: 1.0,
        });
    }
    for i in 0..logical_var_count {
        binary_vars.push(qec_ilp_core::ModelVar {
            name: format!("logical_{i}"),
            objective: 0.0,
            lower: 0.0,
            upper: 1.0,
        });
    }
    for c in 0..width {
        binary_vars.push(qec_ilp_core::ModelVar {
            name: format!("p_{c}"),
            objective: 0.0,
            lower: 0.0,
            upper: 1.0,
        });
    }
    for q in 0..code.n() {
        binary_vars.push(qec_ilp_core::ModelVar {
            name: format!("y_{q}"),
            objective: 1.0,
            lower: 0.0,
            upper: 1.0,
        });
    }

    let integer_vars = (0..width)
        .map(|c| qec_ilp_core::ModelVar {
            name: format!("t_{c}"),
            objective: 0.0,
            lower: 0.0,
            upper: f64::INFINITY,
        })
        .collect::<Vec<_>>();

    let mut constraints = (0..width)
        .map(|c| coordinate_parity_row(
            c,
            &stabilizer_rows,
            &logical_rows,
            symplectic_var_offset,
        ))
        .collect::<Vec<_>>();

    let logical_terms = (stabilizer_var_count..(stabilizer_var_count + logical_var_count))
        .map(|index| (index, 1.0))
        .collect::<Vec<_>>();
    let nonzero_logical_constraint_row = qec_ilp_core::LinearConstraint {
        name: "logical_nonzero".into(),
        sense: qec_ilp_core::ConstraintSense::Ge,
        binary_terms: logical_terms.clone(),
        integer_terms: vec![],
        rhs: 1.0,
    };
    constraints.push(nonzero_logical_constraint_row.clone());

    for qubit in 0..code.n() {
        constraints.extend(weight_rows_for_qubit(
            qubit,
            symplectic_var_offset,
            qubit_activity_offset,
            code.n(),
        ));
    }

    Ok(LoweredDistanceProblem {
        model: qec_ilp_core::BinaryIlpModel {
            binary_vars,
            integer_vars,
            constraints,
            solution_binary_prefix_len: qubit_activity_offset + code.n(),
        },
        stabilizer_var_count,
        logical_var_count,
        symplectic_var_offset,
        qubit_activity_offset,
        nonzero_logical_constraint_row: Some(nonzero_logical_constraint_row),
    })
}

fn coordinate_parity_row(
    coord: usize,
    stabilizers: &[BinaryRow],
    logicals: &[BinaryRow],
    symplectic_var_offset: usize,
) -> qec_ilp_core::LinearConstraint {
    let mut binary_terms = Vec::new();
    for (index, row) in stabilizers.iter().enumerate() {
        if row[coord] == 1 {
            binary_terms.push((index, 1.0));
        }
    }
    for (index, row) in logicals.iter().enumerate() {
        if row[coord] == 1 {
            binary_terms.push((stabilizers.len() + index, 1.0));
        }
    }
    binary_terms.push((symplectic_var_offset + coord, -1.0));

    qec_ilp_core::LinearConstraint {
        name: format!("coord_{coord}"),
        sense: qec_ilp_core::ConstraintSense::Eq,
        binary_terms,
        integer_terms: vec![(coord, -2.0)],
        rhs: 0.0,
    }
}

fn weight_rows_for_qubit(
    qubit: usize,
    symplectic_var_offset: usize,
    qubit_activity_offset: usize,
    n: usize,
) -> Vec<qec_ilp_core::LinearConstraint> {
    let x_index = symplectic_var_offset + qubit;
    let z_index = symplectic_var_offset + n + qubit;
    let y_index = qubit_activity_offset + qubit;

    vec![
        qec_ilp_core::LinearConstraint {
            name: format!("weight_x_{qubit}"),
            sense: qec_ilp_core::ConstraintSense::Le,
            binary_terms: vec![(x_index, 1.0), (y_index, -1.0)],
            integer_terms: vec![],
            rhs: 0.0,
        },
        qec_ilp_core::LinearConstraint {
            name: format!("weight_z_{qubit}"),
            sense: qec_ilp_core::ConstraintSense::Le,
            binary_terms: vec![(z_index, 1.0), (y_index, -1.0)],
            integer_terms: vec![],
            rhs: 0.0,
        },
        qec_ilp_core::LinearConstraint {
            name: format!("weight_or_{qubit}"),
            sense: qec_ilp_core::ConstraintSense::Le,
            binary_terms: vec![(y_index, 1.0), (x_index, -1.0), (z_index, -1.0)],
            integer_terms: vec![],
            rhs: 0.0,
        },
    ]
}
```

- [ ] **Step 5: Run the lowering tests again**

Run:

```bash
cargo test -p qec-code distance_ilp_lowering
```

Expected: PASS.

- [ ] **Step 6: Commit the lowering-only implementation**

```bash
git add qec-code/Cargo.toml qec-code/src/lib.rs qec-code/src/error.rs qec-code/src/distance_ilp.rs qec-code/tests/distance_ilp_lowering.rs
git commit -m "feat: add qec-code ILP distance lowering"
```

---

### Task 4: Integrate ILP Solving Into `qec-code::compute_distance` And Preserve Existing Behavior

**Files:**
- Modify: `qec-code/src/distance.rs`
- Modify: `qec-code/tests/logical_distance.rs`
- Modify: `qec-code/tests/cli.rs`

- [ ] **Step 1: Write the failing distance-dispatch and ILP-regression tests**

Add these tests to `qec-code/tests/logical_distance.rs`:

```rust
#[cfg(feature = "distance-ilp-highs")]
#[test]
fn steane_distance_matches_ilp_path() {
    let steane = Steane::new().unwrap();

    let distance = compute_distance(steane.code()).unwrap();

    assert_eq!(distance.distance, 3);
    assert_eq!(distance.witness.weight(), 3);
}

#[cfg(feature = "distance-ilp-highs")]
#[test]
fn multi_logical_code_returns_a_nontrivial_minimum_witness() {
    let code = StabilizerCode::from_stabilizers(4, vec![pauli(4, &[], &[0]), pauli(4, &[], &[1])])
        .unwrap();

    let distance = compute_distance(&code).unwrap();

    assert_eq!(distance.distance, 1);
    assert_eq!(distance.witness.weight(), 1);
    assert!(!distance.witness.x_bits().iter().chain(distance.witness.z_bits()).all(|&bit| bit == 0));
}

#[cfg(not(feature = "distance-ilp-highs"))]
#[test]
fn large_code_without_ilp_reports_configuration_specific_unsupported_error() {
    let stabilizers = (0..31)
        .map(|qubit| {
            let mut z = vec![0; 32];
            z[qubit] = 1;
            Pauli::from_xz_bits(vec![0; 32], z).unwrap()
        })
        .collect();
    let code = StabilizerCode::from_stabilizers(32, stabilizers).unwrap();

    let err = compute_distance(&code).unwrap_err();

    assert_eq!(
        err,
        QecError::DistanceComputationUnsupported {
            n: 32,
            reason: "enable a distance ILP feature or use a smaller code".into(),
        }
    );
}
```

Add this CLI regression test to `qec-code/tests/cli.rs`:

```rust
#[cfg(not(feature = "distance-ilp-highs"))]
#[test]
fn large_distance_errors_render_configuration_message() {
    let stderr = qec_code::QecError::DistanceComputationUnsupported {
        n: 32,
        reason: "enable a distance ILP feature or use a smaller code".into(),
    }
    .to_string();

    assert!(stderr.contains("distance computation is unsupported"));
}
```

- [ ] **Step 2: Run the distance and CLI tests to verify they fail**

Run:

```bash
cargo test -p qec-code logical_distance cli
```

Expected: FAIL because `compute_distance` still only uses exhaustive enumeration and does not return the new configuration-specific unsupported error.

- [ ] **Step 3: Refactor `compute_distance` into feature-gated dispatch and ILP solve**

Replace the body of `qec-code/src/distance.rs` with this structure:

```rust
use crate::Pauli;
use crate::binary::try_in_row_span;
use crate::code::StabilizerCode;
use crate::error::{QecError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalClass {
    XLike,
    ZLike,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistanceResult {
    pub distance: usize,
    pub witness: Pauli,
    pub logical_class: LogicalClass,
}

pub fn compute_distance(code: &StabilizerCode) -> Result<DistanceResult> {
    if code.num_logical_qubits() == 0 {
        return Err(QecError::DistanceWitnessNotFound);
    }

    #[cfg(feature = "distance-ilp-highs")]
    {
        return compute_distance_via_ilp(code);
    }

    #[cfg(not(feature = "distance-ilp-highs"))]
    {
        return compute_distance_via_exhaustive_search(code);
    }
}

#[cfg(feature = "distance-ilp-highs")]
fn compute_distance_via_ilp(code: &StabilizerCode) -> Result<DistanceResult> {
    let lowered = crate::distance_ilp::lower_distance_problem(code)?;
    let mut backend = qec_ilp_core::backend::build_binary_backend(
        &lowered.model,
        &qec_ilp_core::BinaryIlpConfig::default(),
    )?;
    let solution = backend.solve()?;
    let row = solution.binary_values[lowered.symplectic_var_offset
        ..lowered.symplectic_var_offset + code.n() * 2]
        .iter()
        .map(|&bit| u8::from(bit))
        .collect::<Vec<_>>();
    let witness = Pauli::from_symplectic_row(row)?;
    post_validate_distance_witness(code, &witness)?;

    Ok(DistanceResult {
        distance: witness.weight(),
        logical_class: classify_logical(&witness),
        witness,
    })
}

#[cfg(not(feature = "distance-ilp-highs"))]
fn compute_distance_via_exhaustive_search(code: &StabilizerCode) -> Result<DistanceResult> {
    let mut best_witness: Option<Pauli> = None;

    for candidate in all_normalizer_candidates(code)? {
        let replace = match &best_witness {
            Some(current) => candidate.weight() < current.weight(),
            None => true,
        };

        if replace {
            best_witness = Some(candidate);
        }
    }

    let witness = best_witness.ok_or(QecError::DistanceWitnessNotFound)?;

    Ok(DistanceResult {
        distance: witness.weight(),
        logical_class: classify_logical(&witness),
        witness,
    })
}
```

Update the unsupported-size branch in `all_normalizer_candidates` to return:

```rust
QecError::DistanceComputationUnsupported {
    n,
    reason: "enable a distance ILP feature or use a smaller code".into(),
}
```

Add this helper at the bottom of the same file:

```rust
fn post_validate_distance_witness(code: &StabilizerCode, witness: &Pauli) -> Result<()> {
    if !code
        .stabilizers()
        .iter()
        .all(|stabilizer| witness.commutes_with(stabilizer))
    {
        return Err(QecError::IlpSolveFailed(
            "returned witness does not commute with stabilizers".into(),
        ));
    }

    if try_in_row_span(&code.stabilizer_rows(), &witness.to_symplectic_row())? {
        return Err(QecError::IlpSolveFailed(
            "returned witness lies in stabilizer span".into(),
        ));
    }

    if witness.weight() == 0 {
        return Err(QecError::IlpInfeasible);
    }

    Ok(())
}
```

- [ ] **Step 4: Run the `qec-code` test suite in both configurations**

Run:

```bash
cargo test -p qec-code
cargo test -p qec-code --features distance-ilp-highs
```

Expected: PASS in both modes.

- [ ] **Step 5: Run focused workspace regression coverage**

Run:

```bash
cargo test -p qec-ilp-core
cargo test -p rilpqec
cargo test -p qec-code --features distance-ilp-highs logical_distance
```

Expected: PASS.

- [ ] **Step 6: Commit the `qec-code` solve integration**

```bash
git add qec-code/src/distance.rs qec-code/tests/logical_distance.rs qec-code/tests/cli.rs
git commit -m "feat: add ILP-backed qec-code distance solving"
```
