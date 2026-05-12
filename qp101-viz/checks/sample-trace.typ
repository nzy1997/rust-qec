#import "@preview/quill:0.7.2": tequila
#import "../lib.typ": collect-render-model, entry-annotations, marker-info-for-slot, noise-render-spec, qp101-timeline-file, render-main-op, timeline-theme

#set page(width: auto, height: auto, margin: 10pt)

#let doc = json("sample-trace.qp101.json")
#let ops = doc.at("operations", default: ())
#let model = collect-render-model(ops)
#let theme = timeline-theme(step_width: 8.4em)

#let loss_entry = model.moments.at(0).main.at(0)
#let measurement_entry = model.moments.at(2).main.at(0)
#let detector_entry = model.moments.at(4).main.at(0)
#let loss_visible_entry = model.moments.at(6).main.at(0)

#let loss_built = tequila.build(
  n: doc.at("num_qubits"),
  append-wire: true,
  ..render-main-op(loss_entry, theme),
)
#let measurement_built = tequila.build(
  n: doc.at("num_qubits"),
  append-wire: true,
  ..render-main-op(measurement_entry, theme),
)
#let detector_built = tequila.build(
  n: doc.at("num_qubits"),
  append-wire: true,
  ..render-main-op(detector_entry, theme),
)
#let loss_visible_built = tequila.build(
  n: doc.at("num_qubits"),
  append-wire: true,
  ..render-main-op(loss_visible_entry, theme),
)

#metadata((
  loss_policy: noise-render-spec(ops.at(0)).policy,
  loss_marker: marker-info-for-slot(entry-annotations(loss_entry), 0).label,
  measurement_marker: marker-info-for-slot(entry-annotations(measurement_entry), 0).label,
  measurement_label_count: measurement_built.at(1).labels.len(),
  detector_fill: repr(detector_built.at(1).fill),
  detector_label_count: detector_built.at(1).labels.len(),
  loss_visible_gate: loss_visible_entry.measurement_targets.at(0).gate,
  loss_visible_marker: marker-info-for-slot(entry-annotations(loss_visible_entry), 0).label,
  loss_visible_label_count: loss_visible_built.at(1).labels.len(),
)) <sample-trace-rendered>

#assert.eq(noise-render-spec(ops.at(0)).policy, "single")
#assert.eq(marker-info-for-slot(entry-annotations(loss_entry), 0).label, "L")
#assert.eq(measurement_entry.measurement_targets.len(), 1)
#assert.eq(marker-info-for-slot(entry-annotations(measurement_entry), 0).label, "1[L]")
#assert.eq(measurement_built.at(1).labels.len(), 2)
#assert.eq(loss_visible_entry.measurement_targets.len(), 1)
#assert.eq(loss_visible_entry.measurement_targets.at(0).gate, "MRL")
#assert.eq(marker-info-for-slot(entry-annotations(loss_visible_entry), 0).label, "L=0 | M=1")
#assert.eq(loss_visible_built.at(1).labels.len(), 2)
#assert.eq(repr(detector_built.at(1).fill), "rgb(\"#dbeafe\")")
#assert.eq(detector_built.at(1).labels.len(), 1)

#qp101-timeline-file(
  "checks/sample-trace.qp101.json",
  theme: theme,
)
