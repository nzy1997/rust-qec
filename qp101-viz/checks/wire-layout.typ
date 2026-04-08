#import "../lib.typ": collect-render-model, wire-label-items, timeline-theme

#let doc = json("wire-layout.qp101.json")

#metadata(collect-render-model(doc.at("operations", default: ()))) <wire-layout-model>
#metadata(wire-label-items(doc.at("num_qubits"), 0, doc.at("num_qubits") + 1, timeline-theme())) <wire-layout-labels>

#let expectation = (
  labels: ("q0", "q1"),
  main_wire_count: 2,
  first_moment_top_should_be_empty: true,
  failure_hint: "Timeline should only expose q0/q1 wires and should hide qubit_coords/shift_coords from visible moments",
)
#metadata(expectation) <wire-layout-expectation>
