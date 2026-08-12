import initWasm, { ShotSession } from "../../site/static/interactive/pkg/rstim_shot_web.js";
import { jsPDF } from "jspdf";
import { svg2pdf } from "svg2pdf.js";

const root = document.querySelector("#shot-viewer");

if (root) {
  start().catch(showFatalError);
}

async function start() {
  root.insertAdjacentHTML("beforeend", shellMarkup(root.dataset.mode));
  const ui = collectUi();
  const mode = root.dataset.mode;
  await initWasm({ module_or_path: new URL(root.dataset.wasmUrl, window.location.href) });

  const state = {
    mode,
    session: null,
    snapshot: null,
    sourceName: mode === "fixed" ? "fixed-circuit.stim" : null,
    selectedEventId: null,
    transform: { x: 28, y: 28, scale: 1 },
    drag: null,
    ui,
  };

  bindControls(state);
  ui.loading.hidden = true;

  if (mode === "fixed") {
    const response = await fetch(root.dataset.circuitUrl, { cache: "no-cache" });
    if (!response.ok) throw new Error(`Could not load the fixed circuit (${response.status}).`);
    await openCircuit(state, await response.text(), state.sourceName);
  } else {
    showEmpty(state);
  }
}

function shellMarkup(mode) {
  return `
    <div id="shot-loading" class="shot-loading" role="status">Preparing the circuit laboratory…</div>
    <div id="shot-error" class="shot-alert" role="alert" hidden></div>
    ${mode === "local" ? `<div id="shot-empty" class="shot-empty" hidden>
      <div class="shot-drop-target" id="shot-drop-target">
        <span class="shot-drop-icon" aria-hidden="true">.stim</span>
        <h2>Choose a Stim circuit</h2>
        <p>The file is parsed and executed in this browser. It is never uploaded.</p>
        <label class="shot-button shot-button-primary" for="shot-file">Open .stim file</label>
        <input id="shot-file" type="file" accept=".stim,.txt,text/plain" hidden>
      </div>
    </div>` : ""}
    <div id="shot-workspace" class="shot-workspace" hidden>
      <div class="shot-toolbar" role="toolbar" aria-label="Shot controls">
        <div class="shot-toolbar-group">
          <button class="shot-button shot-button-primary" id="shot-sample" type="button">Sample</button>
          <button class="shot-button" id="shot-clear" type="button">No-error shot</button>
          <button class="shot-icon-button" id="shot-undo" type="button" title="Undo error edit (Ctrl/Cmd+Z)" aria-label="Undo error edit">↶</button>
          <button class="shot-icon-button" id="shot-redo" type="button" title="Redo error edit (Ctrl/Cmd+Shift+Z)" aria-label="Redo error edit">↷</button>
        </div>
        <div class="shot-toolbar-group shot-toolbar-status" aria-live="polite">
          <span id="shot-base-badge" class="shot-badge">No-error</span>
          <span id="shot-summary">0 errors · 0 detectors</span>
        </div>
        <div class="shot-toolbar-group shot-toolbar-actions">
          <button class="shot-button" id="shot-fit" type="button">Fit</button>
          <button class="shot-button" id="shot-export-svg" type="button">Export SVG</button>
          <button class="shot-button" id="shot-export-pdf" type="button">Export PDF</button>
          ${mode === "local" ? '<button class="shot-button shot-button-quiet" id="shot-close" type="button">Close circuit</button>' : ""}
        </div>
      </div>
      <div class="shot-layout">
        <aside class="shot-panel shot-filter-panel" aria-label="Display filters">
          <h2>View</h2>
          <label><input id="shot-filter-errors" type="checkbox" checked> Noise sites</label>
          <label><input id="shot-filter-measurements" type="checkbox" checked> Measurements</label>
          <label><input id="shot-filter-detectors" type="checkbox" checked> Detectors</label>
          <label><input id="shot-filter-observables" type="checkbox" checked> Observables</label>
          <hr>
          <p class="shot-help"><strong>Navigate</strong><br>Drag to pan · wheel to zoom · <kbd>+</kbd>/<kbd>−</kbd> zoom · arrow keys pan.</p>
          <div id="shot-warnings" class="shot-warnings"></div>
        </aside>
        <div class="shot-stage-wrap">
          <div id="shot-stage" class="shot-stage" tabindex="0" aria-label="Circuit diagram. Click a noise site to edit its realized outcome.">
            <div id="shot-canvas" class="shot-canvas"></div>
          </div>
          <div id="shot-popover" class="shot-popover" role="dialog" aria-label="Choose realized noise outcome" hidden></div>
        </div>
        <aside id="shot-detail" class="shot-panel shot-detail" aria-live="polite">
          <p class="eyebrow">Selection</p>
          <h2>Choose a noise site</h2>
          <p>Click an orange noise box in the circuit to inspect or override that event.</p>
        </aside>
      </div>
    </div>`;
}

function collectUi() {
  const find = (id) => document.getElementById(id);
  return {
    loading: find("shot-loading"),
    error: find("shot-error"),
    empty: find("shot-empty"),
    workspace: find("shot-workspace"),
    file: find("shot-file"),
    drop: find("shot-drop-target"),
    sample: find("shot-sample"),
    clear: find("shot-clear"),
    undo: find("shot-undo"),
    redo: find("shot-redo"),
    fit: find("shot-fit"),
    exportSvg: find("shot-export-svg"),
    exportPdf: find("shot-export-pdf"),
    close: find("shot-close"),
    badge: find("shot-base-badge"),
    summary: find("shot-summary"),
    warnings: find("shot-warnings"),
    stage: find("shot-stage"),
    canvas: find("shot-canvas"),
    popover: find("shot-popover"),
    detail: find("shot-detail"),
    filters: {
      noise: find("shot-filter-errors"),
      measurements: find("shot-filter-measurements"),
      detectors: find("shot-filter-detectors"),
      observables: find("shot-filter-observables"),
    },
  };
}

function bindControls(state) {
  const { ui } = state;
  ui.sample.addEventListener("click", () => mutate(state, () => state.session.sample(...randomSeed())));
  ui.clear.addEventListener("click", () => mutate(state, () => state.session.clear(...randomSeed())));
  ui.undo.addEventListener("click", () => mutate(state, () => state.session.undo()));
  ui.redo.addEventListener("click", () => mutate(state, () => state.session.redo()));
  ui.fit.addEventListener("click", () => fitDiagram(state));
  ui.exportSvg.addEventListener("click", () => exportSvg(state));
  ui.exportPdf.addEventListener("click", () => exportPdf(state));
  ui.close?.addEventListener("click", () => closeCircuit(state));

  ui.file?.addEventListener("change", async () => {
    const [file] = ui.file.files;
    if (file) await loadFile(state, file);
  });
  for (const eventName of ["dragenter", "dragover"]) {
    ui.drop?.addEventListener(eventName, (event) => {
      event.preventDefault();
      ui.drop.classList.add("is-dragging");
    });
  }
  for (const eventName of ["dragleave", "drop"]) {
    ui.drop?.addEventListener(eventName, (event) => {
      event.preventDefault();
      ui.drop.classList.remove("is-dragging");
    });
  }
  ui.drop?.addEventListener("drop", async (event) => {
    const [file] = event.dataTransfer.files;
    if (file) await loadFile(state, file);
  });

  ui.canvas.addEventListener("click", (event) => selectFromDiagram(state, event));
  ui.canvas.addEventListener("keydown", (event) => {
    if ((event.key === "Enter" || event.key === " ") && event.target.dataset.noiseEventId) {
      event.preventDefault();
      selectNoise(state, event.target.dataset.noiseEventId, event.target.getBoundingClientRect());
    }
  });
  ui.popover.addEventListener("click", (event) => handleOutcomeChoice(state, event));

  for (const [name, checkbox] of Object.entries(ui.filters)) {
    checkbox.addEventListener("change", () => {
      ui.canvas.classList.toggle(`hide-${name}`, !checkbox.checked);
    });
  }

  bindPanZoom(state);
  document.addEventListener("keydown", (event) => handleShortcut(state, event));
  document.addEventListener("pointerdown", (event) => {
    if (!ui.popover.hidden && !ui.popover.contains(event.target) && !event.target.closest(".noise-site")) {
      hidePopover(state);
    }
  });
  window.addEventListener("resize", () => {
    if (state.snapshot) fitDiagram(state, false);
  });
}

async function loadFile(state, file) {
  if (file.size > 2_000_000) {
    showError(state, "This file exceeds the 2 MB local-file limit.");
    return;
  }
  try {
    await openCircuit(state, await file.text(), file.name);
  } catch (error) {
    showError(state, errorMessage(error));
  } finally {
    state.ui.file.value = "";
  }
}

async function openCircuit(state, source, sourceName) {
  setBusy(state, true);
  clearError(state);
  try {
    await nextPaint();
    const [low, high] = randomSeed();
    const candidate = new ShotSession(source, low, high);
    const snapshot = decodeSnapshot(candidate.snapshot());
    if (snapshot.format_version !== "rstim-shot-view-v1") {
      candidate.free?.();
      throw new Error("The page and simulation engine versions do not match. Refresh this page and try again.");
    }
    state.session?.free?.();
    state.session = candidate;
    state.snapshot = snapshot;
    state.sourceName = sourceName;
    state.selectedEventId = null;
    state.transform = { x: 28, y: 28, scale: 1 };
    if (state.ui.empty) state.ui.empty.hidden = true;
    state.ui.workspace.hidden = false;
    renderSnapshot(state, snapshot, { fit: true });
  } finally {
    setBusy(state, false);
  }
}

function closeCircuit(state) {
  state.session?.free?.();
  state.session = null;
  state.snapshot = null;
  state.sourceName = null;
  state.selectedEventId = null;
  state.ui.canvas.replaceChildren();
  state.ui.workspace.hidden = true;
  hidePopover(state);
  showEmpty(state);
}

function showEmpty(state) {
  clearError(state);
  state.ui.empty.hidden = false;
  state.ui.workspace.hidden = true;
}

async function mutate(state, operation) {
  if (!state.session) return;
  setBusy(state, true);
  clearError(state);
  await nextPaint();
  try {
    const snapshot = decodeSnapshot(operation());
    state.snapshot = snapshot;
    renderSnapshot(state, snapshot);
  } catch (error) {
    showError(state, errorMessage(error));
  } finally {
    setBusy(state, false);
  }
}

function renderSnapshot(state, snapshot, options = {}) {
  const { ui } = state;
  const previousTransform = { ...state.transform };
  ui.canvas.innerHTML = snapshot.svg;
  applyFilters(state);
  updateToolbar(state);
  updateWarnings(state);
  flashChanged(state);

  if (state.selectedEventId) {
    const selected = ui.canvas.querySelector(`[data-noise-event-id="${cssEscape(state.selectedEventId)}"]`);
    if (selected) selected.classList.add("is-selected");
    updateDetail(state, state.selectedEventId);
  }

  if (options.fit) {
    requestAnimationFrame(() => fitDiagram(state));
  } else {
    state.transform = previousTransform;
    applyTransform(state);
  }
}

function updateToolbar(state) {
  const { snapshot, ui } = state;
  const baseKind = snapshot.shot.base.kind;
  const activeErrors = snapshot.shot.result.noise_events.filter(
    (event) => event.effective_outcome.kind !== "identity" && event.applicable,
  ).length;
  const firedDetectors = snapshot.shot.result.detectors.filter((detector) => detector.flipped).length;
  ui.badge.textContent = baseKind === "sampled" ? "Sampled" : "No-error";
  ui.badge.classList.toggle("is-sampled", baseKind === "sampled");
  ui.summary.textContent = `${activeErrors} active errors · ${firedDetectors}/${snapshot.shot.result.detectors.length} detectors`;
  ui.undo.disabled = !snapshot.shot.can_undo;
  ui.redo.disabled = !snapshot.shot.can_redo;
}

function updateWarnings(state) {
  state.ui.warnings.replaceChildren(
    ...state.snapshot.warnings.map((warning) => {
      const paragraph = document.createElement("p");
      paragraph.textContent = warning;
      return paragraph;
    }),
  );
}

function flashChanged(state) {
  const changed = state.snapshot.shot.changed_by_last_action;
  const selectors = [
    ...changed.measurements.map((id) => `[data-measurement-ids~="${cssEscape(id)}"]`),
    ...changed.detectors.map((id) => `[data-detector-id="${cssEscape(id)}"]`),
    ...changed.observables.map((id) => `[data-observable-id="${cssEscape(id)}"]`),
  ];
  for (const selector of selectors) state.ui.canvas.querySelector(selector)?.classList.add("is-changed");
}

function applyFilters(state) {
  for (const [name, checkbox] of Object.entries(state.ui.filters)) {
    state.ui.canvas.classList.toggle(`hide-${name}`, !checkbox.checked);
  }
}

function selectFromDiagram(state, click) {
  const noise = click.target.closest("[data-noise-event-id]");
  if (noise) {
    selectNoise(state, noise.dataset.noiseEventId, noise.getBoundingClientRect());
    return;
  }
  const measurement = click.target.closest("[data-measurement-ids]");
  const detector = click.target.closest("[data-detector-id]");
  const observable = click.target.closest("[data-observable-id]");
  if (measurement) updateResultDetail(state, "measurement", measurement.dataset.measurementIds.split(" "));
  else if (detector) updateResultDetail(state, "detector", [detector.dataset.detectorId]);
  else if (observable) updateResultDetail(state, "observable", [observable.dataset.observableId]);
}

function selectNoise(state, eventId, targetRect) {
  state.selectedEventId = eventId;
  state.ui.canvas.querySelectorAll(".is-selected").forEach((node) => node.classList.remove("is-selected"));
  state.ui.canvas.querySelector(`[data-noise-event-id="${cssEscape(eventId)}"]`)?.classList.add("is-selected");
  updateDetail(state, eventId);
  showPopover(state, eventId, targetRect);
}

function updateDetail(state, eventId) {
  const event = state.snapshot.shot.result.noise_events.find((item) => item.id === eventId);
  const site = state.snapshot.noise_sites.find((item) => item.id === event?.site_id);
  if (!event || !site) return;
  const requested = event.override_outcome ? outcomeLabel(event.override_outcome) : "base result";
  state.ui.detail.innerHTML = `
    <p class="eyebrow">Noise event</p>
    <h2>${escapeHtml(event.instruction)}</h2>
    <dl>
      <dt>Qubits</dt><dd>${event.target_qubits.map((q) => `q${q}`).join(", ")}</dd>
      <dt>Probability</dt><dd>${site.probability ?? "channel"}</dd>
      <dt>Base</dt><dd>${outcomeLabel(event.base_outcome)}</dd>
      <dt>Requested</dt><dd>${escapeHtml(requested)}</dd>
      <dt>Effective</dt><dd>${outcomeLabel(event.effective_outcome)}</dd>
      <dt>Applicable</dt><dd>${event.applicable ? "yes" : "no"}</dd>
    </dl>
    <p>${event.editable ? "This existing noise outcome can be overridden for the current shot." : "This stochastic instruction is read-only in the first version."}</p>
  `;
}

function updateResultDetail(state, kind, ids) {
  hidePopover(state);
  const key = `${kind}s`;
  const results = state.snapshot.shot.result[key].filter((item) => ids.includes(item.id));
  state.ui.detail.innerHTML = `
    <p class="eyebrow">${escapeHtml(kind)}</p>
    <h2>${ids.map(escapeHtml).join(", ")}</h2>
    ${results.map((result) => `<pre>${escapeHtml(JSON.stringify(result, null, 2))}</pre>`).join("")}
  `;
}

function showPopover(state, eventId, targetRect) {
  const event = state.snapshot.shot.result.noise_events.find((item) => item.id === eventId);
  const site = state.snapshot.noise_sites.find((item) => item.id === event?.site_id);
  if (!event || !site || !event.editable) {
    hidePopover(state);
    return;
  }
  const current = outcomeLabel(event.override_outcome ?? event.base_outcome);
  state.ui.popover.innerHTML = `
    <p><strong>${escapeHtml(event.instruction)}</strong> on ${event.target_qubits.map((q) => `q${q}`).join(", ")}</p>
    <div class="shot-outcomes">
      ${site.allowed_outcomes.map((outcome) => {
        const label = outcomeLabel(outcome);
        return `<button type="button" data-outcome="${escapeHtml(label)}" aria-pressed="${label === current}">${escapeHtml(label)}</button>`;
      }).join("")}
    </div>
    <button type="button" class="shot-button shot-restore" data-restore="true" ${event.override_outcome ? "" : "disabled"}>Restore sampled result</button>
  `;
  const wrapRect = state.ui.stage.getBoundingClientRect();
  state.ui.popover.hidden = false;
  const left = clamp(targetRect.left - wrapRect.left, 8, wrapRect.width - state.ui.popover.offsetWidth - 8);
  const top = clamp(targetRect.bottom - wrapRect.top + 10, 8, wrapRect.height - state.ui.popover.offsetHeight - 8);
  state.ui.popover.style.left = `${left}px`;
  state.ui.popover.style.top = `${top}px`;
  state.ui.popover.querySelector("button:not([disabled])")?.focus({ preventScroll: true });
}

function handleOutcomeChoice(state, click) {
  const button = click.target.closest("button");
  if (!button || !state.selectedEventId) return;
  const id = state.selectedEventId;
  hidePopover(state);
  if (button.dataset.restore) mutate(state, () => state.session.restoreNoise(id));
  else if (button.dataset.outcome) mutate(state, () => state.session.setNoise(id, button.dataset.outcome));
}

function hidePopover(state) {
  state.ui.popover.hidden = true;
}

function bindPanZoom(state) {
  const { stage } = state.ui;
  stage.addEventListener("wheel", (event) => {
    event.preventDefault();
    const rect = stage.getBoundingClientRect();
    const pointerX = event.clientX - rect.left;
    const pointerY = event.clientY - rect.top;
    const oldScale = state.transform.scale;
    const scale = clamp(oldScale * Math.exp(-event.deltaY * 0.0015), 0.2, 4);
    state.transform.x = pointerX - ((pointerX - state.transform.x) / oldScale) * scale;
    state.transform.y = pointerY - ((pointerY - state.transform.y) / oldScale) * scale;
    state.transform.scale = scale;
    applyTransform(state);
  }, { passive: false });
  stage.addEventListener("pointerdown", (event) => {
    if (event.button !== 0 || event.target.closest("[data-noise-event-id], [data-measurement-ids], [data-detector-id], [data-observable-id]")) return;
    stage.setPointerCapture(event.pointerId);
    state.drag = { pointerId: event.pointerId, x: event.clientX, y: event.clientY, originX: state.transform.x, originY: state.transform.y };
    stage.classList.add("is-panning");
  });
  stage.addEventListener("pointermove", (event) => {
    if (!state.drag || state.drag.pointerId !== event.pointerId) return;
    state.transform.x = state.drag.originX + event.clientX - state.drag.x;
    state.transform.y = state.drag.originY + event.clientY - state.drag.y;
    applyTransform(state);
  });
  const stop = () => {
    state.drag = null;
    stage.classList.remove("is-panning");
  };
  stage.addEventListener("pointerup", stop);
  stage.addEventListener("pointercancel", stop);
}

function handleShortcut(state, event) {
  if (!state.session || event.target.matches("input, textarea, select")) return;
  const command = event.metaKey || event.ctrlKey;
  if (command && event.key.toLowerCase() === "z") {
    event.preventDefault();
    mutate(state, () => event.shiftKey ? state.session.redo() : state.session.undo());
    return;
  }
  if (event.key === "Escape") hidePopover(state);
  if (event.key === "+" || event.key === "=") zoomAroundCenter(state, 1.2);
  if (event.key === "-" || event.key === "_") zoomAroundCenter(state, 1 / 1.2);
  const delta = event.shiftKey ? 80 : 28;
  if (event.key === "ArrowLeft") state.transform.x += delta;
  else if (event.key === "ArrowRight") state.transform.x -= delta;
  else if (event.key === "ArrowUp") state.transform.y += delta;
  else if (event.key === "ArrowDown") state.transform.y -= delta;
  else return;
  event.preventDefault();
  applyTransform(state);
}

function zoomAroundCenter(state, factor) {
  const rect = state.ui.stage.getBoundingClientRect();
  const oldScale = state.transform.scale;
  const scale = clamp(oldScale * factor, 0.2, 4);
  state.transform.x = rect.width / 2 - ((rect.width / 2 - state.transform.x) / oldScale) * scale;
  state.transform.y = rect.height / 2 - ((rect.height / 2 - state.transform.y) / oldScale) * scale;
  state.transform.scale = scale;
  applyTransform(state);
}

function fitDiagram(state, resetSelection = true) {
  const svg = state.ui.canvas.querySelector("svg");
  if (!svg) return;
  const stageRect = state.ui.stage.getBoundingClientRect();
  const viewBox = svg.viewBox.baseVal;
  const diagramWidth = viewBox.width || Number.parseFloat(svg.getAttribute("width"));
  const diagramHeight = viewBox.height || Number.parseFloat(svg.getAttribute("height"));
  const scale = clamp(Math.min((stageRect.width - 48) / diagramWidth, (stageRect.height - 48) / diagramHeight), 0.2, 2);
  state.transform = {
    x: (stageRect.width - diagramWidth * scale) / 2,
    y: (stageRect.height - diagramHeight * scale) / 2,
    scale,
  };
  if (resetSelection) hidePopover(state);
  applyTransform(state);
}

function applyTransform(state) {
  const { x, y, scale } = state.transform;
  state.ui.canvas.style.transform = `translate(${x}px, ${y}px) scale(${scale})`;
}

function exportSvg(state) {
  const svgText = svgWithProvenance(state);
  download(new Blob([svgText], { type: "image/svg+xml;charset=utf-8" }), exportName(state, "svg"));
}

async function exportPdf(state) {
  setBusy(state, true);
  try {
    const parser = new DOMParser();
    const documentNode = parser.parseFromString(svgWithProvenance(state), "image/svg+xml");
    const svg = documentNode.documentElement;
    const viewBox = svg.viewBox.baseVal;
    const sourceWidth = viewBox.width || Number.parseFloat(svg.getAttribute("width"));
    const sourceHeight = viewBox.height || Number.parseFloat(svg.getAttribute("height"));
    const maxPage = 14400;
    const pageScale = Math.min(1, maxPage / Math.max(sourceWidth, sourceHeight));
    const width = sourceWidth * pageScale;
    const height = sourceHeight * pageScale;
    const pdf = new jsPDF({
      orientation: width >= height ? "landscape" : "portrait",
      unit: "pt",
      format: [width, height],
      compress: true,
      hotfixes: ["px_scaling"],
    });
    pdf.setProperties({
      title: exportName(state, "pdf"),
      subject: JSON.stringify(state.snapshot.provenance),
      creator: `rstim ${state.snapshot.provenance.rstim_version}`,
      keywords: `rstim,qp101,${state.snapshot.provenance.circuit_digest}`,
    });
    await svg2pdf(svg, pdf, { x: 0, y: 0, width, height });
    pdf.save(exportName(state, "pdf"));
  } catch (error) {
    showError(state, `PDF export failed: ${errorMessage(error)}`);
  } finally {
    setBusy(state, false);
  }
}

function svgWithProvenance(state) {
  const parser = new DOMParser();
  const documentNode = parser.parseFromString(state.snapshot.svg, "image/svg+xml");
  const svg = documentNode.documentElement;
  const metadata = documentNode.createElementNS("http://www.w3.org/2000/svg", "metadata");
  metadata.setAttribute("id", "rstim-shot-provenance");
  metadata.textContent = JSON.stringify({
    ...state.snapshot.provenance,
    source_name: state.sourceName,
    exported_at: new Date().toISOString(),
  });
  svg.insertBefore(metadata, svg.firstChild);
  return `<?xml version="1.0" encoding="UTF-8"?>\n${new XMLSerializer().serializeToString(svg)}`;
}

function exportName(state, extension) {
  const base = (state.sourceName || "rstim")
    .replace(/\.[^.]+$/, "")
    .replace(/[^a-zA-Z0-9_.-]+/g, "-") || "rstim";
  const shot = state.snapshot.shot.base;
  const mode = shot.kind === "sampled" ? "shot" : "noiseless";
  const edited = state.snapshot.provenance.overrides.length > 0 ? "-edited" : "";
  const date = new Date().toISOString().slice(0, 10).replaceAll("-", "");
  return `${base}-${mode}-${shot.seed}${edited}-${date}.${extension}`;
}

function download(blob, name) {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = name;
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

function outcomeLabel(outcome) {
  if (!outcome) return "—";
  if (outcome.kind === "identity") return "I";
  if (outcome.kind === "lost") return "L";
  if (outcome.kind === "pauli_pair") return `${pauliLabel(outcome.first)}${pauliLabel(outcome.second)}`;
  return outcome.kind.toUpperCase();
}

function decodeSnapshot(snapshot) {
  return typeof snapshot === "string" ? JSON.parse(snapshot) : snapshot;
}

function pauliLabel(pauli) {
  return typeof pauli === "string" ? pauli.toUpperCase() : String(pauli);
}

function randomSeed() {
  const words = new Uint32Array(2);
  crypto.getRandomValues(words);
  return [words[0], words[1]];
}

function nextPaint() {
  return new Promise((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(resolve));
  });
}

function setBusy(state, busy) {
  root.classList.toggle("shot-busy", busy);
  for (const button of root.querySelectorAll("button")) button.disabled = busy || button.dataset.originallyDisabled === "true";
  if (!busy && state.snapshot) updateToolbar(state);
}

function showError(state, message) {
  state.ui.error.textContent = message;
  state.ui.error.hidden = false;
}

function clearError(state) {
  state.ui.error.hidden = true;
  state.ui.error.textContent = "";
}

function showFatalError(error) {
  const loading = document.getElementById("shot-loading");
  const alert = document.getElementById("shot-error");
  if (loading) loading.hidden = true;
  if (alert) {
    alert.textContent = `The interactive viewer could not start. ${errorMessage(error)}`;
    alert.hidden = false;
  }
}

function errorMessage(error) {
  if (typeof error === "string") return error;
  return error?.message || String(error);
}

function cssEscape(value) {
  return CSS.escape(value);
}

function escapeHtml(value) {
  const span = document.createElement("span");
  span.textContent = String(value);
  return span.innerHTML;
}

function clamp(value, minimum, maximum) {
  return Math.min(maximum, Math.max(minimum, value));
}
