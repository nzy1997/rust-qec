# Issue 247: Plot Series Grouping By Configured Fields

## Context

Benchmark plotting currently builds plotted series from the rendered legend label. That makes labels part of data identity: two rows that intentionally render the same label are merged, and label-template edits can split or merge plotted data.

`BenchmarkSpec` already parses `plot.series.group_by` into `SeriesSpec.group_by`, but `rsinter/src/bench/plot.rs` does not use it when preparing panel groups or assigning series styles.

## Design

Use `plot.series.group_by` as the plotted-series identity when it is configured. Each ok result row gets an internal series key before rendering the legend label. The key is built from the configured row fields in order, with support for the same row scopes used elsewhere in benchmark plotting: `runner`, `language`, `params.*`, `metrics.*`, and `case_summary.*`.

The prepared panel groups store both the internal key and the display label. Draw code uses the key for style lookup and ordering, while the legend still receives the rendered label from `label_template`. If several rows share a group key but render different labels, the first label seen for that key is used for the plotted series.

When `plot.series.group_by` is empty, preserve the existing behavior by using the rendered label as the internal key. Existing specs all configure `group_by`, but this fallback keeps the old behavior available for partial or manually constructed specs.

## Error Handling

Configured group fields must exist. Missing or unsupported group fields produce a plotting error that includes the field name and row context. This keeps invalid grouping specs from silently collapsing unrelated rows into one series.

## Testing

Add an integration regression in `rsinter/tests/bench_plot.rs` named `plot_series_group_by_is_independent_from_label`.

The test covers:

- different `runner` values with the same rendered label remain separate plotted series when `group_by = ["runner"]`;
- rows with different rendered labels merge into one plotted series when they share the configured group key;
- output is inspected through SVG series markers so the failure mode is observable without exposing plotting internals.

Run the issue verification command and then the broader crate/workspace check required for this Agent Desk run.
