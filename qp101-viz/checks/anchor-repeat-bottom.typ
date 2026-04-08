#import "../lib.typ": collect-render-model, render-bottom-entry

#let doc = json("../examples/anchor-repeat.qp101.json")
#let model = collect-render-model(doc.at("operations", default: ()))
#let labels = {
  let out = ()
  for moment in model.moments {
    for entry in moment.bottom {
      out.push(render-bottom-entry(entry, model.measurements))
    }
  }
  out
}

#metadata(labels) <bottoms>
