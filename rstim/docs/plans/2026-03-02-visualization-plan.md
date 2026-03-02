# rstim Visualization Plan

## Two Visualization Targets

1. **Quantum circuit diagrams** — timeline and timeslice views
2. **Sinter result plots** — logical error rate vs physical error rate

---

## Option A: Typst-based rendering

Generate `.typ` source files, compile with `typst` CLI to PDF/SVG/PNG.

**Circuit diagrams:** Emit Typst markup with `line()`, `rect()`, `circle()`, `content()` primitives on a canvas. Timeline = horizontal qubit wires + gate boxes at time positions. Timeslice = 2D grid using qubit coordinates per tick.

**Sinter plots:** Use Typst's `cetz` package (community plotting library) for scatter/line plots with log-scale y-axis. Group curves by distance, x = physical noise, y = logical error rate.

**Pros:**
- Single tool for both circuit diagrams and data plots
- Typst is fast (~10ms compile), installable via `cargo install typst-cli`
- Native PDF output — great for papers
- SVG output via `typst compile --format svg`
- Typst markup is human-readable and editable
- Clean math/label typesetting for free

**Cons:**
- External dependency (`typst` binary must be installed)
- `cetz` is a community package, not built into Typst core
- Less interactive than HTML/JS solutions
- Learning curve for Typst canvas API

---

## Option B: Direct SVG string building (Stim's approach)

Generate SVG by writing XML strings directly in Rust — no external dependencies.

**Circuit diagrams:** Port Stim's approach. Build SVG with `<line>`, `<rect>`, `<circle>`, `<text>` elements. Constants for gate pitch, padding, radius. Timeline = horizontal layout, timeslice = 2D coordinate-based layout.

**Sinter plots:** Write SVG with `<polyline>` for curves, `<text>` for axis labels, manual tick marks and grid lines. Compute log-scale transforms in Rust.

**Pros:**
- Zero external dependencies
- Full control over output
- Can embed in HTML trivially
- What Stim does — proven approach

**Cons:**
- Lots of boilerplate (SVG string formatting for every element)
- Manual axis layout, tick computation, log-scale math for plots
- No PDF output without additional tooling
- Hard to maintain and extend
- Text rendering in SVG is font-dependent

---

## Option C: Typst for circuits + plotters crate for data

Use Typst for circuit diagrams (where structured layout matters), use the `plotters` Rust crate for sinter result graphs (where it's a standard x-y plot).

**Circuit diagrams:** Same as Option A — Typst markup.

**Sinter plots:** Use `plotters` crate with SVG backend. Mature library with log-scale axes, legends, line series built in.

**Pros:**
- Best tool for each job — Typst excels at structured diagrams, plotters excels at data plots
- `plotters` is pure Rust, no external binary needed for plots
- Typst gives clean circuit diagrams with good label support

**Cons:**
- Two different rendering systems to maintain
- `plotters` API is verbose
- Still need `typst` binary for circuit diagrams

---

## Option D: All-Typst with typst-as-library

Use `typst` as a Rust library crate (it's written in Rust) instead of shelling out to the CLI. Generate Typst source programmatically and compile in-process.

**Pros:**
- No external binary — `typst` compiles as a Rust dependency
- Single rendering pipeline for everything
- Can output PDF, SVG, PNG directly from Rust

**Cons:**
- `typst` crate is large (adds significant compile time and binary size)
- API for programmatic use is not fully stable
- Heavier than string-building SVG

---

## Recommendation

**Option A (Typst CLI)** is the best fit:

1. **Unified tool** — one rendering system for both circuits and plots
2. **Fast** — Typst compiles in milliseconds
3. **Paper-ready** — native PDF with proper math typesetting
4. **Readable output** — `.typ` files are human-editable, unlike raw SVG
5. **Lightweight integration** — rstim generates `.typ` text, shells out to `typst compile`

The `typst` dependency is optional — users who don't need visualization don't need it installed. Gate it behind a feature flag or make it a separate crate.

---

## Architecture Sketch (Option A)

```
rstim::viz::circuit_timeline(circuit) -> String     // returns .typ source
rstim::viz::circuit_timeslice(circuit, ticks) -> String
rsinter::viz::error_rate_plot(stats) -> String      // returns .typ source

// Compile step (user calls or helper function)
typst compile output.typ output.pdf
typst compile output.typ output.svg --format svg
```

### Data flow for circuit timeline

```
Vec<StimInstr>
  → resolve operations (flatten repeats, track measurements)
  → assign time slots (moment index per gate)
  → layout: x = moment * pitch, y = qubit * pitch
  → emit Typst canvas:
      - horizontal lines for qubits
      - rectangles/circles for gates
      - vertical lines for two-qubit gates
      - text labels for gate names
      - dashed lines for tick boundaries
```

### Data flow for circuit timeslice

```
Vec<StimInstr>
  → extract qubit coordinates (QUBIT_COORDS annotations)
  → for each tick range: collect gates active in that slice
  → layout: use qubit (x,y) coordinates directly
  → emit Typst canvas:
      - circles at qubit positions
      - lines between interacting qubits
      - color coding for gate types (H=blue, CX=red, M=green)
      - grid of slices if multiple ticks
```

### Data flow for sinter error rate plot

```
Vec<TaskStats>
  → group by metadata key (e.g., distance)
  → for each group: (physical_error_rate, logical_error_rate) points
  → emit Typst with cetz:
      - log-scale y-axis
      - linear or log-scale x-axis
      - one line per distance
      - legend with distance labels
      - axis labels: "Physical error rate", "Logical error rate"
```
