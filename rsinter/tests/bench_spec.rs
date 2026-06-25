use rsinter::bench::spec::{
    AxisSpec, BenchmarkMode, BenchmarkSpec, LogicalRateUnit, PanelSpec, PlotSpec, SeriesSpec,
};
use std::path::Path;

#[test]
fn benchmark_spec_parses_minimal_independent_surface_decoder_doc() {
    let text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rmatching"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [3]
p = [0.002]
rounds = [3]
max_shots = 2000
max_errors = 20
batch_size = 256

[plot]
title = "Surface Decoder"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner", "params.distance"]
label_template = "{runner} d={params.distance}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#;

    let spec: BenchmarkSpec = toml::from_str(text).unwrap();
    assert_eq!(spec.name, "surface_decoder");
    assert_eq!(spec.mode.as_str(), "independent");
    assert_eq!(spec.runners.len(), 1);
    assert_eq!(spec.plot.panels.len(), 1);
}

#[test]
fn benchmark_spec_loads_from_toml_fixture() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bench/minimal_surface_decoder.toml");
    let text = std::fs::read_to_string(path).unwrap();
    let spec: BenchmarkSpec = toml::from_str(&text).unwrap();
    assert_eq!(spec.runners[0].impl_key, "rmatching");
    assert_eq!(spec.plot.x.field, "params.p");
}

#[test]
fn benchmark_spec_defaults_logical_rate_unit_to_per_shot() {
    let text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rmatching"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [3]
p = [0.002]
rounds = [3]
max_shots = 2000
max_errors = 20
batch_size = 256

[plot]
title = "Surface Decoder"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner"]
label_template = "{runner}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#;

    let spec: BenchmarkSpec = toml::from_str(text).unwrap();
    assert_eq!(spec.plot.logical_rate_unit, LogicalRateUnit::PerShot);
}

#[test]
fn benchmark_spec_parses_non_default_logical_rate_unit() {
    let text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rmatching"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [3]
p = [0.002]
rounds = [3]
max_shots = 2000
max_errors = 20
batch_size = 256

[plot]
title = "Surface Decoder"
logical_rate_unit = "per_round_per_observable"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner"]
label_template = "{runner}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#;

    let spec: BenchmarkSpec = toml::from_str(text).unwrap();
    assert_eq!(
        spec.plot.logical_rate_unit,
        LogicalRateUnit::PerRoundPerObservable
    );
}

#[test]
fn benchmark_spec_rejects_invalid_logical_rate_unit() {
    let text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rmatching"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [3]
p = [0.002]
rounds = [3]
max_shots = 2000
max_errors = 20
batch_size = 256

[plot]
title = "Surface Decoder"
logical_rate_unit = "per_cycle"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner"]
label_template = "{runner}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#;

    let err = toml::from_str::<BenchmarkSpec>(text).unwrap_err();
    assert!(err.to_string().contains("per_cycle"));
}

#[test]
fn benchmark_spec_rejects_empty_plot_panels() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bench/invalid_plot.toml");
    let text = std::fs::read_to_string(path).unwrap();
    let spec: BenchmarkSpec = toml::from_str(&text).unwrap();
    let err = spec.validate().unwrap_err();
    assert!(err.contains("at least one plot panel"));
}

#[test]
fn benchmark_spec_rejects_missing_runners() {
    let spec = BenchmarkSpec {
        name: "surface_decoder".into(),
        version: 1,
        mode: BenchmarkMode::Independent,
        runners: Vec::new(),
        plot: PlotSpec {
            title: "Surface Decoder".into(),
            logical_rate_unit: LogicalRateUnit::PerShot,
            x: AxisSpec {
                field: "params.p".into(),
                scale: "log".into(),
                label: "Physical Error Rate".into(),
            },
            series: SeriesSpec {
                group_by: vec!["runner".into()],
                label_template: "{runner}".into(),
            },
            panels: vec![PanelSpec {
                metric: "metrics.logical_error_rate".into(),
                scale: "log".into(),
                label: "Logical Error Rate".into(),
            }],
        },
    };
    let err = spec.validate().unwrap_err();
    assert!(err.contains("must declare at least one runner"));
}

#[test]
fn benchmark_spec_rejects_empty_runner_identity() {
    let text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = ""
language = "rust"
impl_key = ""

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 2000
max_errors = 20
batch_size = 256

[plot]
title = "Surface Decoder"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner"]
label_template = "{runner}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#;
    let mut spec: BenchmarkSpec = toml::from_str(text).unwrap();
    let err = spec.validate().unwrap_err();
    assert!(err.contains("runner name must not be empty"));

    spec.runners[0].name = "rmatching".into();
    let err = spec.validate().unwrap_err();
    assert!(err.contains("must declare impl_key"));
}
