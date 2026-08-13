#import "../lib.typ": collect-render-model

#let doc = json("measurement-contract.qp101.json")
#let model = collect-render-model(doc.at("operations", default: ()))

#metadata((
  measurement_count: model.measurements.len(),
  measurement_anchors: model.measurements.map(item => item.anchor),
  detector_sources: model.moments
    .map(moment => moment.main)
    .flatten()
    .filter(entry => entry.at("kind", default: "") == "detector")
    .map(entry => entry.source),
)) <measurement-contract>
