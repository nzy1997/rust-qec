#import "../lib.typ": collect-render-model, measurement-gate-labels, timeline-theme

#let doc = json("../examples/anchor-basic.qstd101.json")
#let model = collect-render-model(doc.at("operations", default: ()))
#let target = model.moments.at(0).main.at(0).measurement_targets.at(0)

#metadata(measurement-gate-labels(target, timeline-theme())) <labels>
