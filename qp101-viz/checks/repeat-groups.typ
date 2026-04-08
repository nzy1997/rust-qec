#import "../lib.typ": collect-render-model

#let doc = json("../examples/repeat-detector.qp101.json")
#let model = collect-render-model(doc.at("operations", default: ()))

#metadata(model.at("repeat_groups", default: ())) <repeat-groups>
