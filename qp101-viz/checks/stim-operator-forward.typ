#import "../lib.typ": collect-render-model

#let doc = json("stim-operator-forward.qp101.json")
#let model = collect-render-model(doc.at("operations", default: ()))

#metadata(model) <stim-operator-forward-model>

#let expectation = (
  main_entries: (
    (
      kind: "detector",
      host_wire: "q0",
      label: "DETECTOR",
      source: "D0 = rec[0]",
      unresolved: true,
    ),
  ),
  bottom_expectations: (
    should_be_empty: true,
    failure_hint: "Forward detector text should be removed from moment.bottom once promoted",
  ),
  failure_hint: "Forward rec[0] detector should be promoted but keep unresolved source text",
)
#metadata(expectation) <stim-operator-forward-expectation>
