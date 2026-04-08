#import "@preview/quill:0.7.2": tequila
#import "../lib.typ": collect-render-model, noise-render-spec, render-main-op, timeline-theme

#let doc = json("noise-render.qp101.json")
#let ops = doc.at("operations", default: ())
#let model = collect-render-model(ops)
#let theme = timeline-theme()

#metadata((
  x_error: noise-render-spec(ops.at(0)),
  depolarize1: noise-render-spec(ops.at(2)),
  depolarize2: noise-render-spec(ops.at(4)),
)) <noise-render-spec>

#assert.eq(noise-render-spec(ops.at(0)).note, "0.001")
#assert.eq(noise-render-spec(ops.at(2)).note, "0.001")
#assert.eq(noise-render-spec(ops.at(4)).note, "0.001")

#let single = render-main-op(model.moments.at(0).main.at(0), theme)
#let batched = render-main-op(model.moments.at(2).main.at(0), theme)
#let paired = render-main-op(model.moments.at(4).main.at(0), theme)
#let single_built = tequila.build(n: doc.at("num_qubits"), append-wire: true, ..single)
#let batched_built = tequila.build(n: doc.at("num_qubits"), append-wire: true, ..batched)
#let paired_built = tequila.build(n: doc.at("num_qubits"), append-wire: true, ..paired)

#metadata((
  single: (
    count: single.len(),
    qubits: single.map(op => op.first().qubit),
    spans: single.map(op => op.first().n),
    supplement_counts: single.map(op => op.first().supplements.len()),
    note_counts: single_built.slice(1).map(op => op.labels.len()),
    note_reprs: single_built.slice(1).map(op => op.labels.map(label => repr(label.content))),
  ),
  batched: (
    count: batched.len(),
    qubits: batched.map(op => op.first().qubit),
    spans: batched.map(op => op.first().n),
    supplement_counts: batched.map(op => op.first().supplements.len()),
    note_counts: batched_built.slice(1).map(op => op.labels.len()),
    note_reprs: batched_built.slice(1).map(op => op.labels.map(label => repr(label.content))),
  ),
  paired: (
    count: paired.len(),
    lead_qubits: paired.map(op => op.first().qubit),
    spans: paired.map(op => op.first().n),
    supplement_counts: paired.map(op => op.first().supplements.len()),
    note_counts: paired_built.slice(1).map(op => op.labels.len()),
    note_reprs: paired_built.slice(1).map(op => op.labels.map(label => repr(label.content))),
  ),
)) <noise-render-structure>

#assert.eq(single_built.slice(1).map(op => op.labels.len()), (1,))
#assert.eq(batched_built.slice(1).map(op => op.labels.len()), (1, 1, 1))
#assert.eq(
  batched_built.slice(1).map(op => repr(op.labels.at(0).content)),
  ("styled(child: [0.001], ..)", "styled(child: [0.001], ..)", "styled(child: [0.001], ..)"),
)
#assert.eq(
  paired_built.slice(1).filter(op => op.labels.len() > 0).map(op => repr(op.labels.at(0).content)),
  ("styled(child: [0.001], ..)", "styled(child: [0.001], ..)"),
)
