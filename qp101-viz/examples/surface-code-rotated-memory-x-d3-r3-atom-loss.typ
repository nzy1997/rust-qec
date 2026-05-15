#import "../lib.typ": qp101-timeline-file, timeline-theme

#set page(width: auto, height: auto, margin: 10pt)

#let compact-theme = timeline-theme(
  step_width: 4.6em,
  font_size: 6.4pt,
  note_font_size: 5.8pt,
  row_spacing: 12pt,
  gate_padding: 0.28em,
)

= rotated surface code memory X, d=3, r=3, sparse mixed noise

== Source circuit

#qp101-timeline-file(
  "examples/surface-code-rotated-memory-x-d3-r3-atom-loss.qp101.json",
  theme: compact-theme,
)

== Seeded sample shot

#qp101-timeline-file(
  "examples/surface-code-rotated-memory-x-d3-r3-atom-loss-sample.qp101.json",
  theme: compact-theme,
)
