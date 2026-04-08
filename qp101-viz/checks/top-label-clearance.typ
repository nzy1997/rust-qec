#import "@preview/quill:0.7.2": tequila
#import "../lib.typ": collect-render-model, measurement-gate-labels, render-main-op, timeline-theme

#let theme = timeline-theme()

#let measurement_doc = json("../examples/anchor-basic.qp101.json")
#let measurement_model = collect-render-model(measurement_doc.at("operations", default: ()))
#let measurement_target = measurement_model.moments.at(0).main.at(0).measurement_targets.at(0)
#let measurement_built = tequila.build(
  n: measurement_doc.at("num_qubits"),
  append-wire: true,
  ..render-main-op(measurement_model.moments.at(0).main.at(0), theme),
)

#let stim_doc = json("stim-operator-host.qp101.json")
#let stim_model = collect-render-model(stim_doc.at("operations", default: ()))
#let detector_built = tequila.build(
  n: stim_doc.at("num_qubits"),
  append-wire: true,
  ..render-main-op(stim_model.moments.at(0).main.at(6), theme),
)

#metadata((
  measurement_anchor_dy: measurement_built.at(1).labels.at(0).dy,
  measurement_badge_dy: measurement-gate-labels(measurement_target, theme).at(0).dy,
  detector_source_dy: detector_built.at(1).labels.at(0).dy,
)) <top-label-dy>
