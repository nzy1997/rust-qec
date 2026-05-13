#import "@preview/quill:0.7.2": tequila
#import "../lib.typ": collect-render-model, entry-annotations, marker-info-for-slot, measurement-render-spec, noise-render-spec, operation-marker-info, qp101-timeline-file, render-main-op, timeline-theme

#set page(width: auto, height: auto, margin: 10pt)

#let doc = json("sample-trace.qp101.json")
#let ops = doc.at("operations", default: ())
#let model = collect-render-model(ops)
#let theme = timeline-theme(step_width: 8.4em)

#let main_entries = {
  let out = ()
  for moment in model.moments {
    for entry in moment.main {
      out.push(entry)
    }
  }
  out
}

#let first-match(xs, pred) = {
  for x in xs {
    if pred(x) {
      return x
    }
  }
  none
}

#let build-entry(entry) = tequila.build(
  n: doc.at("num_qubits"),
  append-wire: true,
  ..render-main-op(entry, theme),
)

#let gate-entry(gate) = first-match(
  main_entries,
  entry => entry.op.at("gate", default: none) == gate,
)

#let first-labeled-item(built, label_count, fill: none) = first-match(
  built,
  item => item.at("labels", default: ()).len() == label_count and (fill == none or item.fill == fill),
)

#let loss_entry = gate-entry("LOSS")
#let measurement_entry = first-match(
  main_entries,
  entry => entry.op.at("gate", default: none) == "M" and marker-info-for-slot(entry-annotations(entry), 0).label == "1[L]",
)
#let detector_entry = first-match(
  main_entries,
  entry => entry.kind == "detector" and operation-marker-info(entry-annotations(entry)).label == "D0",
)
#let loss_visible_entry = first-match(
  main_entries,
  entry => entry.op.at("gate", default: none) == "MRL" and marker-info-for-slot(entry-annotations(entry), 0).label == "L=0 | M=1",
)

#let measurement_built = build-entry(measurement_entry)
#let measurement_box = first-labeled-item(measurement_built, 2, fill: white)
#let detector_built = build-entry(detector_entry)
#let detector_box = first-labeled-item(detector_built, 1)
#let loss_visible_built = build-entry(loss_visible_entry)
#let loss_visible_box = first-labeled-item(loss_visible_built, 2, fill: white)

#metadata((
  loss_policy: noise-render-spec(loss_entry.op).policy,
  loss_marker: marker-info-for-slot(entry-annotations(loss_entry), 0).label,
  measurement_marker: marker-info-for-slot(entry-annotations(measurement_entry), 0).label,
  measurement_label_count: measurement_box.labels.len(),
  detector_fill: repr(detector_box.fill),
  detector_label_count: detector_box.labels.len(),
  loss_visible_gate: loss_visible_entry.measurement_targets.at(0).gate,
  loss_visible_marker: marker-info-for-slot(entry-annotations(loss_visible_entry), 0).label,
  loss_visible_label_count: loss_visible_box.labels.len(),
  measurement_single_output: measurement-render-spec("M").output_count,
  loss_visible_output_count: measurement-render-spec("MRL").output_count,
)) <sample-trace-rendered>

#assert.eq(loss_entry != none, true)
#assert.eq(measurement_entry != none, true)
#assert.eq(detector_entry != none, true)
#assert.eq(loss_visible_entry != none, true)
#assert.eq(measurement_box != none, true)
#assert.eq(detector_box != none, true)
#assert.eq(loss_visible_box != none, true)
#assert.eq(measurement-render-spec("M").output_count, 1)
#assert.eq(measurement-render-spec("MRL").output_count, 2)
#assert.eq(noise-render-spec(loss_entry.op).policy, "single")
#assert.eq(marker-info-for-slot(entry-annotations(loss_entry), 0).label, "L")
#assert.eq(measurement_entry.measurement_targets.len(), 1)
#assert.eq(marker-info-for-slot(entry-annotations(measurement_entry), 0).label, "1[L]")
#assert.eq(measurement_box.labels.len(), 2)
#assert.eq(loss_visible_entry.measurement_targets.len(), 1)
#assert.eq(loss_visible_entry.measurement_targets.at(0).gate, "MRL")
#assert.eq(marker-info-for-slot(entry-annotations(loss_visible_entry), 0).label, "L=0 | M=1")
#assert.eq(loss_visible_box.labels.len(), 2)
#assert.eq(repr(detector_box.fill), "rgb(\"#dbeafe\")")
#assert.eq(detector_box.labels.len(), 1)

#qp101-timeline-file(
  "checks/sample-trace.qp101.json",
  theme: theme,
)
