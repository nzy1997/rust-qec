#import "../lib.typ": qp101-timeline-file, timeline-theme

#set page(width: auto, height: auto, margin: 10pt)

= Basic

#qp101-timeline-file("examples/basic.qp101.json")

= Repeat And Detector

#qp101-timeline-file(
  "examples/repeat-detector.qp101.json",
  theme: timeline-theme(step_width: 5.8em),
)
