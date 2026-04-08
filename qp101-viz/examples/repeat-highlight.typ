#import "../lib.typ": qp101-timeline-file, timeline-theme

#set page(width: auto, height: auto, margin: 10pt)

= repeat highlight

#qp101-timeline-file(
  "examples/repeat-highlight.qp101.json",
  theme: timeline-theme(step_width: 5.8em),
)
