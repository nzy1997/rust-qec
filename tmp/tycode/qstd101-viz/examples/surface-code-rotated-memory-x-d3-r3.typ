#import "../lib.typ": qstd101-timeline-file, timeline-theme

#set page(width: auto, height: auto, margin: 10pt)

= rotated surface code memory X, d=3, r=3

#qstd101-timeline-file(
  "../../../rstim/tests/fixtures/qstd101/surface_code_rotated_memory_x_d3_r3.json",
  theme: timeline-theme(
    step_width: 4.2em,
    font_size: 6.6pt,
    note_font_size: 6pt,
    row_spacing: 12pt,
    gate_padding: 0.28em,
  ),
)
