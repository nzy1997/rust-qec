#import "../lib.typ": collect-render-model

#let doc = json("anchor-interleaved.qp101.json")
#let model = collect-render-model(doc.at("operations", default: ()))

#metadata(model.moments.at(0).main) <main>
#metadata(model.moments.at(0).bottom) <bottom>
#metadata(model.measurements) <measurements>
