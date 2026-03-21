#import "../lib.typ": qstd101-timeline-file, timeline-theme

#set page(width: auto, height: auto, margin: 10pt)

= Basic

#qstd101-timeline-file("examples/basic.qstd101.json")

= Repeat And Detector

#qstd101-timeline-file(
  "examples/repeat-detector.qstd101.json",
  theme: timeline-theme(step_width: 5.8em),
)
