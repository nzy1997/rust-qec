#import "@preview/quill:0.7.2": tequila
#import "../lib.typ": collect-render-model, render-main-op, timeline-theme

#let doc = json("mr-box.qp101.json")
#let model = collect-render-model(doc.at("operations", default: ()))
#let theme = timeline-theme()

#metadata(tequila.build(
  n: doc.at("num_qubits"),
  append-wire: true,
  ..render-main-op(model.moments.at(0).main.at(0), theme),
)) <mr-box-built>

#let expectation = (
  gate_content: "MR",
  draw_function: "draw-boxed-gate",
  anchor_text: "m1",
  width_should_be_reserved: true,
  failure_hint: "MR should render as a boxed MR gate with its own reserved width instead of a meter glyph with a floating R label",
)
#metadata(expectation) <mr-box-expectation>
