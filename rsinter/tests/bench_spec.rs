use rsinter::bench::spec::BenchmarkSpec;

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
