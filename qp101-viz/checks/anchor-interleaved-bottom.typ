#import "../lib.typ": collect-render-model, render-bottom-entry

#let doc = json("anchor-interleaved.qp101.json")
#let model = collect-render-model(doc.at("operations", default: ()))

#metadata(model.moments.at(0).bottom.map(entry => render-bottom-entry(entry, model.measurements))) <bottoms>
