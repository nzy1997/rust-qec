"use client";

import Image from "next/image";
import { useMemo, useState } from "react";
import {
  EVIDENCE_COUNTS,
  HIGH_ENTROPY_CONTROL,
  LINKS,
  MEASURED_CASE,
  READINESS_DISCLOSURE,
  formatBytes,
  formatDuration,
  formatInteger,
  formatPercent,
  projectShots,
} from "../lib/rsmpEvidence.mjs";

const commandTabs = [
  {
    id: "pack",
    label: "Pack",
    title: "Archive measurement results",
    purpose: "Create a circuit-bound RSMP archive from b8 measurement samples.",
    command: `cargo run --locked -p rstim --bin rstim -- pack_samples \\
  --circuit rstim/tests/fixtures/rsmp/v1/compat.stim \\
  --shots 4 \\
  --in rstim/tests/fixtures/rsmp/v1/compat-measurements.b8 \\
  --in_format b8 \\
  --out /tmp/compat.rsmp`,
    output: "wrote /tmp/compat.rsmp",
  },
  {
    id: "verify",
    label: "Verify",
    title: "Validate without writing outputs",
    purpose:
      "Check archive integrity, circuit identity, block order, digests, and trailer consistency.",
    command: `cargo run --locked -p rstim --bin rstim -- unpack_samples \\
  --circuit rstim/tests/fixtures/rsmp/v1/compat.stim \\
  --in /tmp/compat.rsmp \\
  --verify_only`,
    output: "PASS rsmp archive shots=4 blocks=2 measurements=2 detectors=9 observables=1",
  },
  {
    id: "unpack",
    label: "Unpack",
    title: "Recover decoder inputs",
    purpose:
      "Recover every measurement result first, then derive detectors and observables from the supplied circuit.",
    command: `cargo run --locked -p rstim --bin rstim -- unpack_samples \\
  --circuit rstim/tests/fixtures/rsmp/v1/compat.stim \\
  --in /tmp/compat.rsmp \\
  --measurements_out /tmp/measurements.b8 \\
  --measurements_out_format b8 \\
  --detectors_out /tmp/detectors.01 \\
  --detectors_out_format 01 \\
  --obs_out /tmp/observables.01 \\
  --obs_out_format 01`,
    output: "detectors: 000000000, 000000000, 111111111, 111111111",
  },
];

const projectionShortcuts = [
  { label: "1M", value: 1_000_000 },
  { label: "100M", value: 100_000_000 },
  { label: "1B", value: 1_000_000_000 },
];

const compressionRows = [
  {
    label: "Raw b8",
    bytes: MEASURED_CASE.rawB8Bytes,
    relative: 100,
    tone: "raw",
  },
  {
    label: "Direct Zstandard",
    bytes: MEASURED_CASE.directZstdBytes,
    relative: (MEASURED_CASE.directZstdBytes / MEASURED_CASE.rawB8Bytes) * 100,
    tone: "zstd",
  },
  {
    label: "RSMP v1",
    bytes: MEASURED_CASE.rsmpBytes,
    relative: (MEASURED_CASE.rsmpBytes / MEASURED_CASE.rawB8Bytes) * 100,
    tone: "rsmp",
  },
];

const highEntropyRows = [
  {
    label: "Raw b8",
    bytes: HIGH_ENTROPY_CONTROL.rawB8Bytes,
    relative: 100,
    tone: "raw",
  },
  {
    label: "Direct Zstandard",
    bytes: HIGH_ENTROPY_CONTROL.directZstdBytes,
    relative:
      (HIGH_ENTROPY_CONTROL.directZstdBytes / HIGH_ENTROPY_CONTROL.rawB8Bytes) * 100,
    tone: "zstd",
  },
  {
    label: "RSMP v1",
    bytes: HIGH_ENTROPY_CONTROL.rsmpBytes,
    relative:
      (HIGH_ENTROPY_CONTROL.rsmpBytes / HIGH_ENTROPY_CONTROL.rawB8Bytes) * 100,
    tone: "rsmp",
  },
];

function TerminalWorkflow() {
  const [activeTab, setActiveTab] = useState(commandTabs[0].id);
  const [copied, setCopied] = useState<string | null>(null);

  async function copyCommand(command: string, id: string) {
    try {
      await navigator.clipboard.writeText(command);
      setCopied(id);
      window.setTimeout(() => setCopied((current) => (current === id ? null : current)), 1400);
    } catch {
      setCopied(null);
    }
  }

  return (
    <section className="band band-dark" id="workflow">
      <div className="section-inner workflow-grid">
        <div>
          <p className="eyebrow">Real CLI path</p>
          <h2>Sample, pack, verify, unpack.</h2>
          <p className="section-lede">
            The compatibility fixture is intentionally tiny, so it demonstrates mechanics rather
            than compression. It proves the shape of the workflow: measurement samples go in,
            exact measurements, detectors, and observables come back out.
          </p>
          <div className="micro-note">
            The archive requires the original circuit, not a DEM. Sweep-bit circuits are
            unsupported in v1.
          </div>
        </div>

        <div className="terminal-shell" aria-label="RSMP command examples">
          <div className="tab-list" role="tablist" aria-label="Command examples">
            {commandTabs.map((tab) => (
              <button
                aria-controls={`panel-${tab.id}`}
                aria-selected={activeTab === tab.id}
                className="tab-button"
                id={`tab-${tab.id}`}
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                role="tab"
                type="button"
              >
                {tab.label}
              </button>
            ))}
          </div>
          {commandTabs.map((tab) => (
            <div
              aria-labelledby={`tab-${tab.id}`}
              hidden={activeTab !== tab.id}
              id={`panel-${tab.id}`}
              key={tab.id}
              role="tabpanel"
            >
              <div className="terminal-head">
                <div>
                  <p>{tab.title}</p>
                  <span>{tab.purpose}</span>
                </div>
                <button
                  aria-label={`Copy ${tab.label} command`}
                  className="copy-button"
                  onClick={() => copyCommand(tab.command, tab.id)}
                  type="button"
                >
                  {copied === tab.id ? "Copied" : "Copy"}
                </button>
              </div>
              <pre tabIndex={0}>
                <code>{tab.command}</code>
              </pre>
              <p className="terminal-output">{tab.output}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

function CompressionBars({
  rows,
  percentDigits = 2,
}: {
  rows: Array<{ label: string; bytes: number; relative: number; tone: string }>;
  percentDigits?: number;
}) {
  return (
    <div className="bar-stack">
      {rows.map((row) => (
        <div className="bar-row" key={row.label}>
          <div className="bar-row-head">
            <span>{row.label}</span>
            <span>
              {formatInteger(row.bytes)} B, {row.relative.toFixed(percentDigits)}%
            </span>
          </div>
          <div
            aria-label={`${row.label} uses ${row.relative.toFixed(percentDigits)} percent of raw b8`}
            aria-valuemax={100}
            aria-valuemin={0}
            aria-valuenow={Math.min(row.relative, 100)}
            className="bar-track"
            role="meter"
          >
            <span
              className={`bar-fill bar-${row.tone}`}
              style={{ width: `${Math.min(row.relative, 100)}%` }}
            />
          </div>
        </div>
      ))}
    </div>
  );
}

function ScaleEstimator() {
  const [shotText, setShotText] = useState("1000000");
  const parsedShots = Number(shotText);
  const validationError =
    !Number.isFinite(parsedShots) || parsedShots <= 0
      ? "Enter a positive finite shot count."
      : null;
  const projection = useMemo(() => {
    if (validationError) {
      return null;
    }
    return projectShots(parsedShots);
  }, [parsedShots, validationError]);
  const sliderValue = validationError
    ? 1_000_000
    : Math.min(Math.max(Math.round(parsedShots), MEASURED_CASE.shots), 1_000_000_000);

  return (
    <section className="band band-light" id="scale">
      <div className="section-inner estimator-grid">
        <div>
          <p className="eyebrow eyebrow-projected">Projected</p>
          <h2>Estimate large-scale storage from the pinned case.</h2>
          <p className="section-lede">
            This calculator scales the 1024-shot observation linearly. It is useful for
            planning order-of-magnitude storage and serial runtime, not for claiming a
            measured large-run guarantee.
          </p>
          <div className="shot-control">
            <label htmlFor="shots">Shot count</label>
            <input
              id="shots"
              inputMode="numeric"
              min="1"
              onChange={(event) => setShotText(event.target.value)}
              type="number"
              value={shotText}
            />
            <input
              aria-label="Projected shot count"
              max="1000000000"
              min={MEASURED_CASE.shots}
              onChange={(event) => setShotText(event.target.value)}
              step={MEASURED_CASE.shots}
              type="range"
              value={sliderValue}
            />
            <div className="shortcut-row" aria-label="Projection shortcuts">
              {projectionShortcuts.map((shortcut) => (
                <button
                  key={shortcut.label}
                  onClick={() => setShotText(String(shortcut.value))}
                  type="button"
                >
                  {shortcut.label}
                </button>
              ))}
            </div>
            {validationError ? <p className="input-error">{validationError}</p> : null}
          </div>
        </div>

        <div className="projection-panel" aria-live="polite">
          {projection ? (
            <>
              <div className="projection-main">
                <span>RSMP estimate</span>
                <strong>{formatBytes(projection.rsmpBytes)}</strong>
                <small>for {formatInteger(projection.shots)} shots</small>
              </div>
              <dl className="projection-list">
                <div>
                  <dt>Raw b8</dt>
                  <dd>{formatBytes(projection.rawBytes)}</dd>
                </div>
                <div>
                  <dt>Direct Zstandard</dt>
                  <dd>{formatBytes(projection.zstdBytes)}</dd>
                </div>
                <div>
                  <dt>Pack time</dt>
                  <dd>{formatDuration(projection.packSeconds)}</dd>
                </div>
                <div>
                  <dt>Unpack time</dt>
                  <dd>{formatDuration(projection.unpackSeconds)}</dd>
                </div>
                <div>
                  <dt>Saved vs raw</dt>
                  <dd>{formatPercent(projection.rawSavingsBytes, projection.rawBytes, 2)}</dd>
                </div>
                <div>
                  <dt>Saved vs Zstd</dt>
                  <dd>{formatPercent(projection.zstdSavingsBytes, projection.zstdBytes, 2)}</dd>
                </div>
              </dl>
              <p className="projection-caveat">
                Linear projection from one 1024-shot observation on the pinned surface-code
                case. Not a large-scale measurement or performance guarantee.
              </p>
            </>
          ) : (
            <p className="input-error large">{validationError}</p>
          )}
        </div>
      </div>
    </section>
  );
}

export default function Home() {
  const rawSaving = formatPercent(
    MEASURED_CASE.rawB8Bytes - MEASURED_CASE.rsmpBytes,
    MEASURED_CASE.rawB8Bytes,
    2,
  );
  const zstdSaving = formatPercent(
    MEASURED_CASE.directZstdBytes - MEASURED_CASE.rsmpBytes,
    MEASURED_CASE.directZstdBytes,
    2,
  );
  const oneMillion = projectShots(1_000_000);

  return (
    <main className="site-shell">
      <section className="hero" aria-labelledby="hero-title">
        <div className="hero-backdrop" aria-hidden="true" />
        <div className="hero-inner">
          <div className="hero-copy">
            <p className="eyebrow">Lossless circuit-bound sample archives</p>
            <h1 id="hero-title">RSMP v1</h1>
            <p className="hero-lede">
              Save every measurement needed for decoding, compress the sparse detector
              structure, and recover detector and observable results with the original circuit.
            </p>
          </div>
          <div className="hero-metrics" aria-label="Pinned compression result">
            <div className="metric metric-primary">
              <span>Measured RSMP size</span>
              <strong>11.98%</strong>
              <small>of raw b8</small>
            </div>
            <div className="metric">
              <span>Measured against Zstandard</span>
              <strong>57.14%</strong>
              <small>of direct Zstd</small>
            </div>
            <div className="case-card">
              <span className="status status-measured">Measured</span>
              <p>{MEASURED_CASE.circuit}</p>
              <small>
                {formatInteger(MEASURED_CASE.shots)} shots, M={formatInteger(MEASURED_CASE.measurements)}, D=
                {formatInteger(MEASURED_CASE.detectors)}, L={MEASURED_CASE.observables}
              </small>
            </div>
          </div>
        </div>
      </section>

      <section className="band band-light" id="intro">
        <div className="section-inner split">
          <div>
            <p className="eyebrow eyebrow-measured">What it stores</p>
            <h2>Measurements first. Decoder data on demand.</h2>
            <p className="section-lede">
              RSMP v1 archives every measurement bit needed to reconstruct a sample result.
              During unpack, the tool rebuilds the circuit-derived transform, recovers
              measurements losslessly, and derives all detector and observable outputs.
            </p>
          </div>
          <div className="principles">
            <div>
              <span>01</span>
              <p>Every measurement result is recovered losslessly.</p>
            </div>
            <div>
              <span>02</span>
              <p>Detectors and logical observables are derived using the supplied circuit.</p>
            </div>
            <div>
              <span>03</span>
              <p>Integrity checks detect corrupted headers, blocks, payloads, and trailers.</p>
            </div>
          </div>
        </div>
      </section>

      <section className="band band-ink" id="transform">
        <div className="section-inner">
          <div className="section-heading">
            <p className="eyebrow">Why circuit knowledge helps</p>
            <h2>A reversible transform separates sparse syndrome bits from free coordinates.</h2>
          </div>
          <div className="pipeline" aria-label="RSMP transform pipeline">
            {[
              "measurements",
              "XOR noiseless reference",
              "detectors + free coordinates",
              "dense or sparse syndrome codec",
              "independent Zstandard frames",
            ].map((stage, index) => (
              <div className="pipeline-stage" key={stage}>
                <span>{String(index + 1).padStart(2, "0")}</span>
                <p>{stage}</p>
              </div>
            ))}
          </div>
          <p className="pipeline-note">
            The archive stores the circuit identity and transform dimensions, not a standalone
            DEM. The reader consumes blocks in order, so v1 offers sequential access, not random
            shot access.
          </p>
        </div>
      </section>

      <TerminalWorkflow />

      <section className="band band-light" id="compression">
        <div className="section-inner compression-grid">
          <div>
            <p className="eyebrow eyebrow-measured">Measured</p>
            <h2>The pinned d=11/r=100 case compresses to 11.98% of raw b8.</h2>
            <p className="section-lede">
              RSMP uses {formatInteger(MEASURED_CASE.rsmpBytes)} bytes for the pinned
              surface-code sample set, compared with {formatInteger(MEASURED_CASE.rawB8Bytes)}
              bytes of raw b8 and {formatInteger(MEASURED_CASE.directZstdBytes)} bytes after
              applying Zstandard directly to raw samples.
            </p>
            <div className="savings-row">
              <div>
                <strong>{rawSaving}</strong>
                <span>fewer bytes than raw b8</span>
              </div>
              <div>
                <strong>{zstdSaving}</strong>
                <span>fewer bytes than direct Zstandard</span>
              </div>
            </div>
          </div>
          <div className="chart-panel">
            <CompressionBars rows={compressionRows} />
          </div>
        </div>
      </section>

      <section className="band band-muted">
        <div className="section-inner compression-grid">
          <div>
            <p className="eyebrow">High-entropy control</p>
            <h2>Bounded overhead, not magic compression.</h2>
            <p className="section-lede">
              The high-entropy control is shown separately so the page does not imply every
              circuit compresses like the surface-code case. RSMP is {formatPercent(
                HIGH_ENTROPY_CONTROL.rsmpBytes,
                HIGH_ENTROPY_CONTROL.rawB8Bytes,
                4,
              )}{" "}
              of raw b8 on that control.
            </p>
          </div>
          <div className="chart-panel">
            <CompressionBars rows={highEntropyRows} percentDigits={4} />
          </div>
        </div>
      </section>

      <ScaleEstimator />

      <section className="band band-dark" id="confidence">
        <div className="section-inner evidence-grid">
          <div>
            <p className="eyebrow eyebrow-measured">Evidence and limits</p>
            <h2>Production-style corruption coverage, plus one important readiness caveat.</h2>
            <p className="section-lede">
              The committed evidence includes fixed semantic coverage, corruption mutations,
              and an immutable two-block reader specimen with sparse and dense codecs.
            </p>
          </div>
          <div className="evidence-counters">
            <div>
              <strong>{EVIDENCE_COUNTS.semanticRoles}</strong>
              <span>required semantic roles</span>
            </div>
            <div>
              <strong>{EVIDENCE_COUNTS.namedCorruptionRecipes}</strong>
              <span>named corruption recipes</span>
            </div>
            <div>
              <strong>{EVIDENCE_COUNTS.generatedTruncations}</strong>
              <span>generated truncation checks</span>
            </div>
            <div>
              <strong>{EVIDENCE_COUNTS.generatedBitFlips}</strong>
              <span>generated bit flips</span>
            </div>
          </div>
          <div className="readiness-box">
            <span className="status status-caveat">Caveat</span>
            <p>{READINESS_DISCLOSURE.cleanCheckout}</p>
            <p>{READINESS_DISCLOSURE.currentTree}</p>
            <p>{READINESS_DISCLOSURE.interpretation}</p>
          </div>
          <div className="link-grid" aria-label="RSMP references">
            <a href={LINKS.repository}>Repository</a>
            <a href={LINKS.format}>Normative format</a>
            <a href={LINKS.cliGuide}>CLI guide</a>
            <a href={LINKS.evidence}>Evidence bundle</a>
            <a href={LINKS.testReport}>Feature-test report</a>
          </div>
        </div>
      </section>

      <section className="band band-share">
        <div className="section-inner share-strip">
          <div>
            <p className="eyebrow">At 1M shots</p>
            <h2>{formatBytes(oneMillion.rsmpBytes)} projected RSMP storage.</h2>
            <p>
              Same pinned case, same linear model: raw b8 would be {formatBytes(
                oneMillion.rawBytes,
              )}, direct Zstandard would be {formatBytes(oneMillion.zstdBytes)}.
            </p>
          </div>
          <Image
            alt="RSMP v1 social preview card"
            height={630}
            priority
            src="/og.png"
            width={1200}
          />
        </div>
      </section>
    </main>
  );
}
