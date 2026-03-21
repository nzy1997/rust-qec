#import "@preview/quill:0.7.2": *

#let timeline-theme(
  step_width: 5.2em,
  lane_width: 6.5em,
  font_size: 8pt,
  note_font_size: 7pt,
  wire: .75pt + rgb("#2f3540"),
  color: rgb("#20262e"),
  scale: 100%,
  row_spacing: 14pt,
  column_spacing: auto,
  gate_padding: .35em,
  gate_fill: white,
  noise_fill: rgb("#ffe4d6"),
  note_color: rgb("#4b5563"),
  circuit_padding: (top: 1.2em, bottom: 1.2em, left: 0.4em, right: 0.6em),
) = {
  let resolved_column_spacing = if column_spacing == auto {
    step_width / 3
  } else {
    column_spacing
  }
  (
    step_width: step_width,
    lane_width: lane_width,
    font_size: font_size,
    note_font_size: note_font_size,
    wire: wire,
    color: color,
    scale: scale,
    row_spacing: row_spacing,
    column_spacing: resolved_column_spacing,
    gate_padding: gate_padding,
    gate_fill: gate_fill,
    noise_fill: noise_fill,
    note_color: note_color,
    circuit_padding: circuit_padding,
  )
}

#let round3(x) = calc.round(x, digits: 3)

#let fmt-num(x) = str(round3(x))

#let fmt-vec(xs) = {
  if xs.len() == 0 {
    return "()"
  }
  "(" + xs.map(v => fmt-num(v)).join(", ") + ")"
}

#let contains-int(xs, needle) = {
  for x in xs {
    if x == needle {
      return true
    }
  }
  false
}

#let unique-ints(xs) = {
  let out = ()
  for x in xs {
    if not contains-int(out, x) {
      out.push(x)
    }
  }
  out
}

#let shifted-qubits(xs) = xs

#let qubits-from-refs(refs) = {
  let out = ()
  for ref in refs {
    let kind = ref.at("kind", default: "")
    if kind == "qubit" {
      out.push(ref.at("index"))
    } else if kind == "pauli" {
      out.push(ref.at("qubit"))
    }
  }
  unique-ints(out)
}

#let render-ref(ref) = {
  let kind = ref.at("kind", default: "")
  if kind == "qubit" {
    let prefix = if ref.at("inverted", default: false) { "!" } else { "" }
    return prefix + "q" + str(ref.at("index"))
  }
  if kind == "rec" {
    return "rec[" + str(ref.at("offset")) + "]"
  }
  if kind == "pauli" {
    let prefix = if ref.at("inverted", default: false) { "!" } else { "" }
    return prefix + ref.at("basis") + str(ref.at("qubit"))
  }
  if kind == "combiner" {
    return "*"
  }
  if kind == "sweep" {
    return "sweep[" + str(ref.at("index")) + "]"
  }
  "?"
}

#let render-refs(refs) = {
  if refs.len() == 0 {
    return "-"
  }
  refs.map(render-ref).join(" ")
}

#let measurement-by-index(measurements, measurement_index) = {
  for measurement in measurements {
    if measurement.measurement_index == measurement_index {
      return measurement
    }
  }
  none
}

#let resolve-source(ref, measurements, measurement_count) = {
  let kind = ref.at("kind", default: "")
  if kind == "rec" {
    let offset = ref.at("offset", default: 0)
    let resolved_index = measurement_count + offset + 1
    if offset >= 0 or resolved_index <= 0 or resolved_index > measurement_count {
      return (
        kind: "text",
        text: "unresolved " + render-ref(ref),
      )
    }
    let measurement = measurement-by-index(measurements, resolved_index)
    if measurement == none {
      return (
        kind: "text",
        text: "unresolved " + render-ref(ref),
      )
    }
    return (
      kind: "anchor",
      index: measurement.measurement_index,
      text: measurement.anchor,
    )
  }
  (
    kind: "text",
    text: render-ref(ref),
  )
}

#let resolve-stim-source(ref, measurements, measurement_count) = {
  let kind = ref.at("kind", default: "")
  if kind == "rec" {
    let offset = ref.at("offset", default: 0)
    let resolved_index = measurement_count + offset + 1
    if offset >= 0 or resolved_index <= 0 or resolved_index > measurement_count {
      return (
        kind: "text",
        text: render-ref(ref),
        unresolved: true,
      )
    }
    let measurement = measurement-by-index(measurements, resolved_index)
    if measurement == none {
      return (
        kind: "text",
        text: render-ref(ref),
        unresolved: true,
      )
    }
    return (
      kind: "anchor",
      index: measurement.measurement_index,
      text: measurement.anchor,
      qubit: measurement.qubit,
      unresolved: false,
    )
  }
  (
    kind: "text",
    text: render-ref(ref),
    unresolved: false,
  )
}

#let compress-anchor-run(run) = {
  if run.len() >= 2 {
    return (run.first().text + "-" + run.last().text,)
  }
  run.map(item => item.text)
}

#let render-resolved-refs(refs, measurements, measurement_count) = {
  if refs.len() == 0 {
    return "-"
  }
  let resolved = refs.map(ref => resolve-source(ref, measurements, measurement_count))
  let out = ()
  let anchor_run = ()

  for item in resolved {
    if item.kind == "anchor" {
      if anchor_run.len() == 0 {
        anchor_run.push(item)
      } else if item.index == anchor_run.last().index + 1 {
        anchor_run.push(item)
      } else {
        for text in compress-anchor-run(anchor_run) {
          out.push(text)
        }
        anchor_run = (item,)
      }
    } else {
      if anchor_run.len() > 0 {
        for text in compress-anchor-run(anchor_run) {
          out.push(text)
        }
        anchor_run = ()
      }
      out.push(item.text)
    }
  }

  if anchor_run.len() > 0 {
    for text in compress-anchor-run(anchor_run) {
      out.push(text)
    }
  }

  out.join(" ")
}

#let stim-source-pieces(refs, measurements, measurement_count) = {
  if refs.len() == 0 {
    return ("-",)
  }
  refs.map(ref => resolve-stim-source(ref, measurements, measurement_count).text)
}

#let stim-source-text(refs, measurements, measurement_count) = {
  stim-source-pieces(refs, measurements, measurement_count).join("*")
}

#let stim-source-unresolved(refs, measurements, measurement_count) = {
  for ref in refs {
    if resolve-stim-source(ref, measurements, measurement_count).at("unresolved", default: false) {
      return true
    }
  }
  false
}

#let stim-source-host-qubit(refs, measurements, measurement_count) = {
  let candidates = ()
  for ref in refs {
    let resolved = resolve-stim-source(ref, measurements, measurement_count)
    if resolved.kind == "anchor" {
      candidates.push(resolved.qubit)
    }
  }
  if candidates.len() > 0 {
    return candidates.sorted().first()
  }

  let fallbacks = qubits-from-refs(refs).sorted()
  if fallbacks.len() > 0 {
    return fallbacks.first()
  }

  0
}

#let gate-label(op) = {
  let display = op.at("display", default: none)
  if display != none {
    let label = display.at("label", default: none)
    if label != none {
      return label
    }
  }
  let gate = op.at("gate")
  let params = op.at("params", default: ())
  if params.len() == 0 {
    return gate
  }
  gate + "(" + params.map(v => fmt-num(v)).join(", ") + ")"
}

#let gate-qubits(op) = {
  let out = ()
  for q in op.at("targets", default: ()) {
    out.push(q)
  }
  for q in op.at("controls", default: ()) {
    out.push(q)
  }
  let raw = op.at("raw_targets", default: none)
  if raw != none {
    for q in qubits-from-refs(raw) {
      out.push(q)
    }
  }
  unique-ints(out)
}

#let span-pass-through(qubits) = {
  let sorted = unique-ints(qubits).sorted()
  if sorted.len() < 2 {
    return ()
  }
  let start = sorted.first()
  let last = sorted.last()
  let holes = ()
  for q in range(start + 1, last) {
    if not contains-int(sorted, q) {
      holes.push(q - start)
    }
  }
  holes
}

#let empty-moment() = (top: (), main: (), bottom: ())

#let moment-empty(moment) = moment.top.len() == 0 and moment.main.len() == 0 and moment.bottom.len() == 0

#let moment-add-top(moment, text) = (
  top: moment.top + (text,),
  main: moment.main,
  bottom: moment.bottom,
)

#let moment-add-main(moment, op) = (
  top: moment.top,
  main: moment.main + (op,),
  bottom: moment.bottom,
)

#let moment-add-bottom(moment, text) = (
  top: moment.top,
  main: moment.main,
  bottom: moment.bottom + (text,),
)

#let main-entry(op, measurement_targets: none) = (
  kind: "gate",
  op: op,
  measurement_targets: measurement_targets,
)

#let repeat-group-entry(count, start_moment_index, end_moment_index, iteration_starts) = (
  label: "repeat x" + str(count),
  count: count,
  start_moment_index: start_moment_index,
  end_moment_index: end_moment_index,
  iteration_starts: iteration_starts,
)

#let stim-operator-entry(op, measurements, measurement_count, detector_index: none) = {
  let kind = op.at("type", default: "")
  let sources = op.at("sources", default: ())
  let source_text = stim-source-text(sources, measurements, measurement_count)

  (
    kind: kind,
    host_qubit: stim-source-host-qubit(sources, measurements, measurement_count),
    label: if kind == "detector" {
      "DETECTOR"
    } else {
      "OBS_INCLUDE(" + str(op.at("index")) + ")"
    },
    source: if kind == "detector" {
      "D" + str(detector_index) + " = " + source_text
    } else {
      "L" + str(op.at("index")) + " *= " + source_text
    },
    unresolved: stim-source-unresolved(sources, measurements, measurement_count),
    index: if kind == "detector" { detector_index } else { op.at("index") },
    coords: op.at("coords", default: ()),
    measurement_count: measurement_count,
    sources: sources,
  )
}

#let measurement-targets(op) = {
  if op.at("type", default: "") != "gate" {
    return none
  }
  let gate = op.at("gate", default: "")
  if gate != "M" and gate != "MX" and gate != "MR" {
    return none
  }
  let out = ()
  for (target_index, qubit) in op.at("targets", default: ()).enumerate() {
    out.push((
      target_index: target_index,
      qubit: qubit,
      gate: gate,
    ))
  }
  out
}

#let bottom-entry(op, measurement_count) = {
  let kind = op.at("type", default: "")
  if kind == "detector" {
    return (
      kind: kind,
      sources: op.at("sources", default: ()),
      coords: op.at("coords", default: ()),
      measurement_count: measurement_count,
    )
  }
  if kind == "observable_include" {
    return (
      kind: kind,
      index: op.at("index"),
      sources: op.at("sources", default: ()),
      measurement_count: measurement_count,
    )
  }
  none
}

#let render-bottom-entry(entry, measurements) = {
  if entry.kind == "detector" {
    let suffix = if entry.coords.len() == 0 { "" } else { " @ " + fmt-vec(entry.coords) }
    return "det " + render-resolved-refs(entry.sources, measurements, entry.measurement_count) + suffix
  }
  if entry.kind == "observable_include" {
    return "obs[" + str(entry.index) + "] " + render-resolved-refs(entry.sources, measurements, entry.measurement_count)
  }
  "?"
}

#let collect-render-model-from(ops, moment_base: 0, starting_measurement_index: 0, starting_detector_index: 0) = {
  let moments = ()
  let measurements = ()
  let repeat_groups = ()
  let current = empty-moment()
  let measurement_index = starting_measurement_index
  let detector_index = starting_detector_index

  for op in ops {
    let kind = op.at("type")
    if kind == "tick" {
      if not moment-empty(current) {
        moments.push(current)
        current = empty-moment()
      }
      moments.push((top: ("|",), main: (), bottom: ()))
    } else if kind == "repeat" {
      if not moment-empty(current) {
        moments.push(current)
        current = empty-moment()
      }
      let count = op.at("count")
      let group_start = moment_base + moments.len()
      let iteration_starts = ()
      for i in range(count) {
        iteration_starts.push(moment_base + moments.len())
        let nested = collect-render-model-from(
          op.at("body"),
          moment_base: moment_base + moments.len(),
          starting_measurement_index: measurement_index,
          starting_detector_index: detector_index,
        )
        for nested_moment in nested.moments {
          moments.push(nested_moment)
        }
        for measurement in nested.measurements {
          measurements.push(measurement)
        }
        for repeat_group in nested.repeat_groups {
          repeat_groups.push(repeat_group)
        }
        measurement_index = nested.next_measurement_index
        detector_index = nested.next_detector_index
      }
      let group_end = moment_base + moments.len() - 1
      if group_start <= group_end {
        repeat_groups.push(repeat-group-entry(count, group_start, group_end, iteration_starts))
      }
    } else if kind == "qubit_coords" {
    } else if kind == "shift_coords" {
    } else if kind == "detector" {
      current = moment-add-main(current, stim-operator-entry(op, measurements, measurement_index, detector_index: detector_index))
      detector_index += 1
    } else if kind == "observable_include" {
      current = moment-add-main(current, stim-operator-entry(op, measurements, measurement_index))
    } else if kind == "annotation" {
      current = moment-add-top(current, op.at("kind") + ": " + op.at("text"))
    } else {
      let measurement_targets = measurement-targets(op)
      if measurement_targets == none {
        current = moment-add-main(current, main-entry(op))
      } else {
        let detailed_targets = ()
        let current_moment_index = moment_base + moments.len()
        for target in measurement_targets {
          measurement_index += 1
          let detail = (
            measurement_index: measurement_index,
            anchor: "m" + str(measurement_index),
            moment_index: current_moment_index,
            render_moment_index: current_moment_index,
            target_index: target.target_index,
            qubit: target.qubit,
            gate: target.gate,
          )
          detailed_targets.push(detail)
          measurements.push(detail)
        }
        current = moment-add-main(current, main-entry(op, measurement_targets: detailed_targets))
      }
    }
  }

  if not moment-empty(current) {
    moments.push(current)
  }

  (
    moments: moments,
    measurements: measurements,
    repeat_groups: repeat_groups,
    next_measurement_index: measurement_index,
    next_detector_index: detector_index,
  )
}

#let collect-render-model(ops) = {
  let model = collect-render-model-from(ops)
  (
    moments: model.moments,
    measurements: model.measurements,
    repeat_groups: model.repeat_groups,
  )
}

#let note-op(qubit, note_text, theme) = tequila.gate(
  qubit,
  text(size: theme.note_font_size, fill: theme.note_color)[#note_text],
  fill: none,
  stroke: none,
)

#let estimated-text-width(text_value, font_size, padding: 0em) = {
  0.58em * calc.max(str(text_value).len(), 1) + padding
}

#let max-length(xs) = {
  let out = xs.first()
  for x in xs.slice(1) {
    if x > out {
      out = x
    }
  }
  out
}

#let reserved-gate-width(widths, minimum: 1.8em) = {
  max-length((minimum,) + widths)
}

#let measurement-anchor-badge(anchor, theme) = box(
  inset: (x: 0.18em, y: 0.03em),
  radius: 0.22em,
  fill: white,
  stroke: .45pt + theme.note_color,
  text(size: theme.note_font_size - 1pt, fill: theme.note_color)[#anchor],
)

#let measurement-anchor-label(target, theme) = (
  content: measurement-anchor-badge(target.anchor, theme),
  pos: top,
  dy: 0.55em,
)

#let measurement-gate-labels(target, theme) = {
  let labels = ()
  if target.gate == "MX" {
    labels.push((content: $X$, pos: top, dy: 0.5em))
  } else if target.gate == "MR" {
    labels.push((content: "R", pos: top, dy: 0.5em))
  }
  labels.push((
    content: measurement-anchor-badge(target.anchor, theme),
    pos: top + right,
    dx: -0.3em,
    dy: 0.1em,
  ))
  labels
}

#let measurement-box-label(target) = {
  if target.gate == "MX" {
    return "MX"
  }
  if target.gate == "MR" {
    return "MR"
  }
  "M"
}

#let measurement-box-width(target, theme) = reserved-gate-width((
  estimated-text-width(measurement-box-label(target), theme.note_font_size, padding: 0.9em),
  estimated-text-width(target.anchor, theme.note_font_size - 1pt, padding: 0.9em),
), minimum: 2.2em)

#let measurement-box-gate(target, theme) = tequila.gate(
  target.qubit,
  text(size: theme.note_font_size, fill: theme.note_color)[#measurement-box-label(target)],
  fill: white,
  stroke: .6pt + theme.note_color,
  width: measurement-box-width(target, theme),
  label: (measurement-anchor-label(target, theme),),
)

#let light-gate(qubit, label, theme) = tequila.gate(
  qubit,
  label,
  fill: none,
  stroke: .6pt + theme.note_color,
)

#let generic-gate(qubits, label, theme, fill: none) = {
  let sorted = unique-ints(qubits).sorted()
  if sorted.len() == 0 {
    return none
  }
  if sorted.len() == 1 {
    return tequila.gate(sorted.first(), label, fill: fill)
  }
  let first = sorted.first()
  let last = sorted.last()
  tequila.mqgate(
    first,
    n: last - first + 1,
    label,
    fill: fill,
    pass-through: span-pass-through(sorted),
  )
}

#let wire-label-items(num_qubits, top_wire, bottom_wire, theme) = {
  let items = ()
  for q in range(num_qubits) {
    items.push(lstick(
      text(size: theme.font_size, fill: theme.color)[#("q" + str(q))],
      x: 0,
      y: q,
    ))
  }
  items
}

#let stim-operator-gate(entry, theme) = tequila.gate(
  entry.host_qubit,
  text(size: theme.note_font_size, fill: theme.color)[#entry.label],
  fill: luma(235),
  stroke: .6pt + theme.color,
  radius: 0pt,
  width: reserved-gate-width((
    estimated-text-width(entry.label, theme.note_font_size, padding: 0.9em),
    estimated-text-width(entry.source, theme.note_font_size - 1pt, padding: 0.9em),
  ), minimum: 3em),
  label: (
    (
      content: text(size: theme.note_font_size - 1pt, fill: theme.note_color)[#entry.source],
      pos: top,
      dy: 0.9em,
    ),
  ),
)

#let render-main-op(entry, theme) = {
  if entry.kind == "detector" or entry.kind == "observable_include" {
    return (stim-operator-gate(entry, theme),)
  }

  let op = entry.op
  let kind = op.at("type")
  let label = gate-label(op)

  if kind == "noise" {
    let qubits = shifted-qubits(qubits-from-refs(op.at("raw_targets", default: ())))
    let gate = generic-gate(qubits, label, theme, fill: theme.noise_fill)
    if gate == none { return () }
    return (gate,)
  }

  let targets = shifted-qubits(op.at("targets", default: ()))
  let controls = shifted-qubits(op.at("controls", default: ()))
  let visible = shifted-qubits(gate-qubits(op))
  let gate = op.at("gate")
  let measurement_targets = entry.measurement_targets

  if measurement_targets != none {
    let ops = ()
    for target in measurement_targets {
      ops.push(measurement-box-gate(target, theme))
    }
    return ops
  }

  if gate == "R" {
    return targets.map(t => light-gate(t, "R", theme))
  }
  if gate == "RX" {
    return targets.map(t => light-gate(t, $R_x$, theme))
  }
  if gate == "H" {
    return targets.map(t => tequila.h(t))
  }
  if gate == "X" and controls.len() == 0 {
    return targets.map(t => tequila.x(t))
  }
  if gate == "Y" {
    return targets.map(t => tequila.y(t))
  }
  if gate == "Z" and controls.len() == 0 {
    return targets.map(t => tequila.z(t))
  }
  if gate == "S" {
    return targets.map(t => tequila.s(t))
  }
  if gate == "T" {
    return targets.map(t => tequila.t(t))
  }

  if controls.len() == 1 and targets.len() == 1 {
    if gate == "X" {
      return (tequila.cx(controls.first(), targets.first()),)
    }
    if gate == "Z" {
      return (tequila.cz(controls.first(), targets.first()),)
    }
    return (tequila.ca(controls.first(), targets.first(), label, fill: theme.gate_fill),)
  }

  if controls.len() > 1 and targets.len() == 1 {
    return (
      tequila.multi-controlled-gate(
        controls,
        targets.first(),
        gate.with(label, fill: theme.gate_fill),
      ),
    )
  }

  if gate == "S_DAG" and targets.len() == 1 {
    return (tequila.sdg(targets.first()),)
  }
  if gate == "T" and targets.len() == 1 {
    return (tequila.t(targets.first()),)
  }
  if gate == "T_DAG" and targets.len() == 1 {
    return (tequila.tdg(targets.first()),)
  }
  if gate == "CX" and targets.len() == 2 {
    return (tequila.cx(targets.at(0), targets.at(1)),)
  }
  if gate == "CX" and targets.len() > 2 and calc.rem(targets.len(), 2) == 0 {
    let ops = ()
    for pair in range(calc.floor(targets.len() / 2)) {
      let i = pair * 2
      ops.push(tequila.cx(targets.at(i), targets.at(i + 1)))
    }
    return ops
  }
  if gate == "CZ" and targets.len() == 2 {
    return (tequila.cz(targets.at(0), targets.at(1)),)
  }
  if gate == "CZ" and targets.len() > 2 and calc.rem(targets.len(), 2) == 0 {
    let ops = ()
    for pair in range(calc.floor(targets.len() / 2)) {
      let i = pair * 2
      ops.push(tequila.cz(targets.at(i), targets.at(i + 1)))
    }
    return ops
  }
  if gate == "SWAP" and targets.len() == 2 {
    return (tequila.swap(targets.at(0), targets.at(1)),)
  }

  let rendered = generic-gate(
    if visible.len() == 0 { targets } else { visible },
    label,
    theme,
    fill: theme.gate_fill,
  )
  if rendered == none {
    return ()
  }
  (rendered,)
}

#let build-moment-ops(moment, measurements, top_wire, bottom_wire, theme) = {
  let ops = ()
  if moment.top == ("|",) and moment.main.len() == 0 and moment.bottom.len() == 0 {
    if top_wire == bottom_wire {
      ops.push(note-op(top_wire, "|", theme))
    } else {
      ops.push(tequila.barrier(start: top_wire, end: bottom_wire))
    }
    return ops
  }
  if moment.top.len() > 0 {
    ops.push(note-op(top_wire, moment.top.join("  |  "), theme))
  }
  for op in moment.main {
    for rendered in render-main-op(op, theme) {
      ops.push(rendered)
    }
  }
  if moment.bottom.len() > 0 {
    ops.push(note-op(bottom_wire, moment.bottom.map(entry => render-bottom-entry(entry, measurements)).join("  |  "), theme))
  }
  ops
}

#let last-column(items) = calc.max(..items.map(item => item.x))

#let repeat-group-label(group, theme) = (
  content: box(
    inset: (x: 0.22em, y: 0.04em),
    radius: 0.18em,
    fill: white,
    text(size: theme.note_font_size, fill: theme.note_color)[#group.label],
  ),
  pos: top + left,
  dx: 0.45em,
  dy: 0.72em,
)

#let repeat-slice-label(text_value, theme) = (
  content: text(size: theme.note_font_size - 1pt, fill: theme.note_color)[#text_value],
  pos: top,
  dy: 0.62em,
)

#let build-repeat-decorations(model, moment_spans, top_wire, total_wires, theme) = {
  let items = ()

  for group in model.repeat_groups {
    let start_span = moment_spans.at(group.start_moment_index, default: none)
    let end_span = moment_spans.at(group.end_moment_index, default: none)
    if start_span == none or end_span == none {
      continue
    }

    items.push(gategroup(
      total_wires,
      end_span.end - start_span.start + 1,
      x: start_span.start,
      y: top_wire,
      z: "below",
      padding: (
        left: 0.2em,
        right: 0.2em,
        top: 0.9em,
        bottom: 0.4em,
      ),
      stroke: (paint: luma(180), thickness: .6pt, dash: "dashed"),
      fill: luma(248),
      radius: 4pt,
      label: (repeat-group-label(group, theme),),
    ))

    for (iteration_offset, moment_index) in group.iteration_starts.slice(1).enumerate() {
      let span = moment_spans.at(moment_index, default: none)
      if span == none {
        continue
      }
      items.push(slice(
        n: total_wires,
        x: span.start,
        y: top_wire,
        z: "below",
        stroke: (paint: luma(195), thickness: .55pt, dash: "dashed"),
        label: (repeat-slice-label("iter " + str(iteration_offset + 2), theme),),
      ))
    }
  }

  items
}

#let qstd101-timeline(doc, theme: timeline-theme()) = {
  let model = collect-render-model(doc.at("operations", default: ()))
  let moments = model.moments
  let total_wires = doc.at("num_qubits")
  let top_wire = 0
  let bottom_wire = total_wires - 1

  let items = ()
  let moment_spans = ()
  let next_x = 1

  for label in wire-label-items(doc.at("num_qubits"), top_wire, bottom_wire, theme) {
    items.push(label)
  }

  for moment in moments {
    let start_x = next_x
    let ops = build-moment-ops(moment, model.measurements, top_wire, bottom_wire, theme)
    if ops.len() == 0 {
      moment_spans.push(none)
      continue
    }
    let built = tequila.build(
      n: total_wires,
      x: start_x,
      y: 0,
      append-wire: true,
      ..ops,
    )
    for item in built {
      items.push(item)
    }
    let end_x = last-column(built)
    moment_spans.push((start: start_x, end: end_x))
    next_x = end_x + 1
  }

  for item in build-repeat-decorations(model, moment_spans, top_wire, total_wires, theme) {
    items.push(item)
  }

  block(
    inset: 8pt,
    stroke: .5pt + luma(225),
    radius: 6pt,
    fill: white,
    [
      #set text(size: 10pt, fill: theme.color)
      #strong(doc.at("standard", default: "QSTD101-ZY")) v#doc.at("version", default: "1.0")
      #v(0.6em)
      #quantum-circuit(
        wire: theme.wire,
        row-spacing: theme.row_spacing,
        column-spacing: theme.column_spacing,
        gate-padding: theme.gate_padding,
        font-size: theme.font_size,
        color: theme.color,
        scale: theme.scale,
        circuit-padding: theme.circuit_padding,
        wires: total_wires,
        ..items,
      )
    ],
  )
}

#let qstd101-timeline-file(path, theme: timeline-theme()) = {
  qstd101-timeline(json(path), theme: theme)
}
