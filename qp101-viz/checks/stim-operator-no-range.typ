#import "../lib.typ": collect-render-model

#let doc = json("stim-operator-no-range.qp101.json")
#let model = collect-render-model(doc.at("operations", default: ()))

#metadata(model) <stim-operator-no-range-model>

#let expectation = (
  detector_source: "D0 = m1*m2*m3",
  failure_hint: "Stim-style detector source text should list each anchor explicitly instead of compressing runs into m1-m3 style ranges",
)
#metadata(expectation) <stim-operator-no-range-expectation>
