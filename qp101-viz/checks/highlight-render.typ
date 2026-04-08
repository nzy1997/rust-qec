#import "@preview/quill:0.7.2": tequila
#import "../lib.typ": collect-render-model, render-main-op, timeline-theme

#let theme = timeline-theme()

#let gate-label-texts(gate) = gate.labels.map(label => repr(label.content))
#let gate-fill(gate) = repr(gate.fill)

#let single_doc = json("../examples/depolarize1-single-highlight.qp101.json")
#let single_model = collect-render-model(single_doc.at("operations", default: ()))
#let single_built = tequila.build(
  n: single_doc.at("num_qubits"),
  append-wire: true,
  ..render-main-op(single_model.moments.at(0).main.at(1), theme),
)
#let single_detector_built = tequila.build(
  n: single_doc.at("num_qubits"),
  append-wire: true,
  ..render-main-op(single_model.moments.at(0).main.at(3), theme),
)

#let repeat_doc = json("../examples/repeat-highlight.qp101.json")
#let repeat_model = collect-render-model(repeat_doc.at("operations", default: ()))
#let repeat_iter0_built = tequila.build(
  n: repeat_doc.at("num_qubits"),
  append-wire: true,
  ..render-main-op(repeat_model.moments.at(0).main.at(0), theme),
)
#let repeat_iter1_built = tequila.build(
  n: repeat_doc.at("num_qubits"),
  append-wire: true,
  ..render-main-op(repeat_model.moments.at(2).main.at(0), theme),
)

#let multi_doc = json("../examples/multi-source-highlight.qp101.json")
#let multi_model = collect-render-model(multi_doc.at("operations", default: ()))
#let multi_first_built = tequila.build(
  n: multi_doc.at("num_qubits"),
  append-wire: true,
  ..render-main-op(multi_model.moments.at(0).main.at(1), theme),
)
#let multi_second_built = tequila.build(
  n: multi_doc.at("num_qubits"),
  append-wire: true,
  ..render-main-op(multi_model.moments.at(0).main.at(2), theme),
)
#let multi_detector_built = tequila.build(
  n: multi_doc.at("num_qubits"),
  append-wire: true,
  ..render-main-op(multi_model.moments.at(0).main.at(4), theme),
)

#let rendered = (
  single: gate-label-texts(single_built.at(1)),
  single_detector: gate-label-texts(single_detector_built.at(1)),
  single_detector_fill: gate-fill(single_detector_built.at(1)),
  repeat_iter0_slot0: gate-label-texts(repeat_iter0_built.at(1)),
  repeat_iter0_slot1: gate-label-texts(repeat_iter0_built.at(2)),
  repeat_iter1_slot0: gate-label-texts(repeat_iter1_built.at(1)),
  repeat_iter1_slot1: gate-label-texts(repeat_iter1_built.at(2)),
  multi_first: gate-label-texts(multi_first_built.at(1)),
  multi_second: gate-label-texts(multi_second_built.at(1)),
  multi_detector: gate-label-texts(multi_detector_built.at(1)),
  multi_detector_fill: gate-fill(multi_detector_built.at(1)),
)

#assert.eq(rendered.single.len(), 2)
#assert.eq(rendered.single_detector.len(), 1)
#assert.eq(rendered.single_detector_fill, "rgb(\"#dbeafe\")")
#assert.eq(rendered.repeat_iter0_slot0.len(), 1)
#assert.eq(rendered.repeat_iter0_slot1.len(), 3)
#assert.eq(rendered.repeat_iter0_slot1.at(2), "styled(child: [repeat[0]], ..)")
#assert.eq(rendered.repeat_iter1_slot0.len(), 1)
#assert.eq(rendered.repeat_iter1_slot1.len(), 3)
#assert.eq(rendered.repeat_iter1_slot1.at(2), "styled(child: [repeat[1]], ..)")
#assert.eq(rendered.multi_first.len(), 2)
#assert.eq(rendered.multi_second.len(), 2)
#assert.eq(rendered.multi_detector.len(), 1)
#assert.eq(rendered.multi_detector_fill, "rgb(\"#dbeafe\")")

#metadata(rendered) <highlight-rendered>
