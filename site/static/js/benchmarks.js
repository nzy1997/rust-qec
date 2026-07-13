(function () {
  const ROOT = (document.body && document.body.dataset && document.body.dataset.root) || ".";

  function escapeHtml(value) {
    return String(value)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  const benchmarkManifest = document.getElementById("benchmark-manifest");
  const checkedBenchmarkResults = document.getElementById("checked-benchmark-result-cards");
  const checkedBenchmarkItems = [
    "surface-decoder-full",
    "bb-circuit-full",
    "rstim-vs-stim-correctness",
    "rstim-vs-stim-full",
    "rstim-vs-stim-release",
    "rstim-vs-stim-release-repetition-sample",
    "rstim-vs-stim-release-surface-detect",
    "rstim-vs-stim-release-dem-sample",
  ];

  function renderBadge(label, value) {
    return `<span class="badge">${escapeHtml(label)}: ${escapeHtml(value || "unspecified")}</span>`;
  }

  function renderBenchmarkManifest(manifest) {
    if (!benchmarkManifest) {
      return;
    }
    const families = Array.isArray(manifest.families) ? manifest.families : [];
    if (!families.length) {
      benchmarkManifest.innerHTML = "<p>No benchmark families are listed.</p>";
      return;
    }
    benchmarkManifest.innerHTML = families
      .map((family) => {
        const items = Array.isArray(family.evidence_items) ? family.evidence_items : [];
        const itemHtml = items
          .map((item) => `
            <article class="manifest-item">
              <div class="manifest-heading">
                <h4>${escapeHtml(item.title || item.id || "Evidence item")}</h4>
                <div class="schema-meta">
                  ${renderBadge("status", item.status)}
                  ${renderBadge("tier", item.tier)}
                </div>
              </div>
              <p><strong>Claims limit:</strong> ${escapeHtml(item.claims_limit || "No claims limit recorded.")}</p>
            </article>
          `)
          .join("");
        return `
          <article class="manifest-family">
            <div class="manifest-heading">
              <h3>${escapeHtml(family.title || family.id || "Benchmark family")}</h3>
              <div class="schema-meta">
                ${renderBadge("status", family.status)}
              </div>
            </div>
            <p><strong>Claims limit:</strong> ${escapeHtml(family.claims_limit || "No claims limit recorded.")}</p>
            <div class="manifest-items">${itemHtml}</div>
          </article>
        `;
      })
      .join("");
  }

  function findEvidenceItem(manifest, itemId) {
    const families = Array.isArray(manifest.families) ? manifest.families : [];
    for (const family of families) {
      const items = Array.isArray(family.evidence_items) ? family.evidence_items : [];
      const item = items.find((candidate) => candidate && candidate.id === itemId);
      if (item) {
        return { family, item };
      }
    }
    return null;
  }

  function fileName(path) {
    return String(path || "").split("/").pop() || String(path || "artifact");
  }

  function renderArtifactLinks(item) {
    const artifacts = Array.isArray(item.artifacts) ? item.artifacts : [];
    const checkedArtifacts = artifacts.filter(
      (artifact) => artifact && artifact.checked === true && artifact.path,
    );
    if (!checkedArtifacts.length) {
      return "<p>No checked artifacts are listed for this item.</p>";
    }
    const links = checkedArtifacts
      .map(
        (artifact) => `
        <li>
          <a href="${ROOT}/${escapeHtml(artifact.path)}">${escapeHtml(fileName(artifact.path))}</a>
          <span class="badge">${escapeHtml(artifact.kind || "artifact")}</span>
        </li>
      `,
      )
      .join("");
    return `<ul class="result-link-list">${links}</ul>`;
  }

  function renderImageArtifacts(item) {
    const artifacts = Array.isArray(item.artifacts) ? item.artifacts : [];
    const images = artifacts.filter(
      (artifact) => artifact && artifact.checked === true && artifact.kind === "image" && artifact.path,
    );
    if (!images.length) {
      return "";
    }
    return images
      .map(
        (image) => `
        <figure class="result-plot">
          <a href="${ROOT}/${escapeHtml(image.path)}">
            <img src="${ROOT}/${escapeHtml(image.path)}" alt="${escapeHtml(item.title || "Checked benchmark plot")}">
          </a>
        </figure>
      `,
      )
      .join("");
  }

  function renderCommandList(commands) {
    if (!Array.isArray(commands) || !commands.length) {
      return "<p>No reproduction command is listed.</p>";
    }
    const commandText = commands.map((command) => `$ ${command}`).join("\n");
    return `<pre class="result-commands"><code>${escapeHtml(commandText)}</code></pre>`;
  }

  function renderTextList(values) {
    if (!Array.isArray(values) || !values.length) {
      return "";
    }
    return `<ul class="result-note-list">${values.map((value) => `<li>${escapeHtml(value)}</li>`).join("")}</ul>`;
  }

  function repoSourceHref(path) {
    if (typeof path !== "string" || /^(?:https?:)?\/\//.test(path) || path.startsWith("#")) {
      return path;
    }
    return `https://github.com/nzy1997/rstim/blob/master/${path}`;
  }

  function renderSourceLinks(paths) {
    if (!Array.isArray(paths) || !paths.length) {
      return "";
    }
    const links = paths
      .map((path) => `<li><a href="${escapeHtml(repoSourceHref(path))}">${escapeHtml(path)}</a></li>`)
      .join("");
    return `<ul class="result-link-list source-links">${links}</ul>`;
  }

  function renderCompactValue(value) {
    if (value === null || value === undefined) {
      return "";
    }
    if (Array.isArray(value)) {
      if (!value.length) {
        return '<span class="provenance-muted">empty</span>';
      }
      return `<ul class="provenance-value-list">${value.map((item) => `<li>${escapeHtml(item)}</li>`).join("")}</ul>`;
    }
    if (typeof value === "object") {
      const entries = Object.entries(value);
      if (!entries.length) {
        return '<span class="provenance-muted">empty</span>';
      }
      return `<ul class="provenance-value-list">${entries
        .map(([key, entryValue]) => `<li><code>${escapeHtml(key)}</code>: ${escapeHtml(JSON.stringify(entryValue))}</li>`)
        .join("")}</ul>`;
    }
    return `<span>${escapeHtml(value)}</span>`;
  }

  function renderArtifactHashes(entry) {
    if (entry && entry.status === "not_recorded") {
      return `<p class="provenance-muted">${escapeHtml(entry.reason || "reason not recorded")}</p>`;
    }
    if (!entry || entry.status !== "recorded" || !entry.value || typeof entry.value !== "object") {
      return renderCompactValue(entry && entry.value);
    }
    const rows = Object.entries(entry.value)
      .map(([path, hashEntry]) => {
        const sha = hashEntry && typeof hashEntry === "object" ? hashEntry.sha256 : "";
        return `
          <li>
            <code>${escapeHtml(path)}</code>
            <span class="provenance-hash">${escapeHtml(sha || "sha256 not recorded")}</span>
          </li>
        `;
      })
      .join("");
    return `
      <p class="provenance-muted">${Object.keys(entry.value).length} checked artifact hashes recorded</p>
      <ul class="provenance-hash-list">${rows}</ul>
    `;
  }

  function renderProvenance(provenance) {
    if (!provenance || typeof provenance !== "object") {
      return "<p>No canonical provenance is recorded for this checked result.</p>";
    }
    const rows = Object.entries(provenance)
      .map(([field, entry]) => {
        if (field === "schema_version") {
          return `
            <li class="provenance-row">
              <div class="provenance-row-heading">
                <code>${escapeHtml(field)}</code>
                <span class="badge">recorded</span>
              </div>
              ${renderCompactValue(entry)}
            </li>
          `;
        }
        const status = entry && typeof entry === "object" ? entry.status : "unspecified";
        const body =
          field === "artifact_hashes"
            ? renderArtifactHashes(entry)
            : status === "not_recorded"
              ? `<p class="provenance-muted">${escapeHtml(entry.reason || "reason not recorded")}</p>`
              : renderCompactValue(entry && entry.value);
        return `
          <li class="provenance-row">
            <div class="provenance-row-heading">
              <code>${escapeHtml(field)}</code>
              <span class="badge">${escapeHtml(status)}</span>
            </div>
            ${body}
          </li>
        `;
      })
      .join("");
    return `<ul class="provenance-card-list">${rows}</ul>`;
  }

  function renderCheckedBenchmarkResults(manifest) {
    if (!checkedBenchmarkResults) {
      return;
    }
    checkedBenchmarkResults.innerHTML = checkedBenchmarkItems
      .map((itemId) => {
        const found = findEvidenceItem(manifest, itemId);
        if (!found) {
          return `<article class="result-card error"><h3>${escapeHtml(itemId)}</h3><p>Missing checked benchmark manifest item.</p></article>`;
        }
        const { family, item } = found;
        return `
        <article class="result-card">
          <div class="result-card-copy">
            <div class="manifest-heading">
              <div>
                <p class="eyebrow">${escapeHtml(family.title || family.id || "Benchmark family")}</p>
                <h3>${escapeHtml(item.title || item.id || "Checked benchmark result")}</h3>
              </div>
              <div class="schema-meta">
                ${renderBadge("family", family.status)}
                ${renderBadge("status", item.status)}
                ${renderBadge("tier", item.tier)}
              </div>
            </div>
            <p><strong>Claims limit:</strong> ${escapeHtml(item.claims_limit || family.claims_limit || "No claims limit recorded.")}</p>
            <h4>Artifacts</h4>
            ${renderArtifactLinks(item)}
            <h4>Reproduction</h4>
            ${renderCommandList(item.commands)}
            <h4>Provenance</h4>
            ${renderProvenance(item.provenance)}
            <h4>Caveats</h4>
            ${renderTextList(item.caveats)}
            <h4>Provenance Sources</h4>
            ${renderSourceLinks(item.provenance_sources || family.source_docs)}
          </div>
          <div class="result-card-plot">
            ${renderImageArtifacts(item)}
          </div>
        </article>
      `;
      })
      .join("");
  }

  if (benchmarkManifest || checkedBenchmarkResults) {
    fetch(ROOT + "/data/benchmark-site.json")
      .then((response) => {
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        return response.json();
      })
      .then((manifest) => {
        renderBenchmarkManifest(manifest);
        renderCheckedBenchmarkResults(manifest);
      })
      .catch((error) => {
        if (benchmarkManifest) {
          benchmarkManifest.classList.add("error");
          benchmarkManifest.innerHTML = `
            <p>Benchmark manifest could not be loaded: ${escapeHtml(error.message)}</p>
            <p><a href="${ROOT}/data/benchmark-site.json">Open benchmark-site.json</a></p>
          `;
        }
        if (checkedBenchmarkResults) {
          checkedBenchmarkResults.classList.add("error");
          checkedBenchmarkResults.innerHTML = `
            <p>Checked benchmark results could not be loaded: ${escapeHtml(error.message)}</p>
            <p><a href="${ROOT}/data/benchmark-site.json">Open benchmark-site.json</a></p>
          `;
        }
      });
  }
})();
