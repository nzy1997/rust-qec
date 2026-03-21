#import "../lib.typ": collect-render-model, render-bottom-entry

#let doc = json("../examples/anchor-basic.qstd101.json")
#let model = collect-render-model(doc.at("operations", default: ()))

#metadata(render-bottom-entry(model.moments.at(0).bottom.at(0), model.measurements)) <bottom>
