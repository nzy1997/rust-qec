#import "@preview/quill:0.7.2": tequila
#import "../lib.typ": collect-render-model, render-main-op, timeline-theme

#let doc = json("stim-operator-host.qp101.json")
#let model = collect-render-model(doc.at("operations", default: ()))
#let theme = timeline-theme()

#let detector = model.moments.at(0).main.at(6)
#let observable = model.moments.at(0).main.at(8)

#metadata(tequila.build(n: doc.at("num_qubits"), append-wire: true, ..render-main-op(detector, theme))) <operator-spacing-detector>
#metadata(tequila.build(n: doc.at("num_qubits"), append-wire: true, ..render-main-op(observable, theme))) <operator-spacing-observable>

#let expectation = (
  detector_width_should_be_reserved: true,
  observable_width_should_be_reserved: true,
  detector_source: "D0 = m2*m1",
  observable_source: "L0 *= m7",
  source_label_dy: "-0.15em",
  failure_hint: "Stim-style operators should reserve enough width for their top source text so labels do not overlap adjacent columns",
)
#metadata(expectation) <operator-spacing-expectation>
