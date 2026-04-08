#import "../lib.typ": collect-render-model

#let doc = json("stim-operator-host.qp101.json")
#let model = collect-render-model(doc.at("operations", default: ()))

#metadata(model) <stim-operator-host-model>

#let expectation = (
  main_entries: (
    (
      kind: "detector",
      host_wire: "q0",
      label: "DETECTOR",
      source: "D0 = m2*m1",
    ),
    (
      kind: "observable_include",
      host_wire: "q1",
      label: "OBS_INCLUDE(0)",
      source: "L0 *= m7",
    ),
  ),
  bottom_expectations: (
    should_be_empty: true,
    failure_hint: "Detector/observable text should disappear from moment.bottom once promoted",
  ),
  failure_hint: "Should promote detector/observable into moment.main with Stim-style host/text",
)
#metadata(expectation) <stim-operator-host-expectation>
