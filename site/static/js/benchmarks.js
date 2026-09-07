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
  const evidenceContainers = Array.from(document.querySelectorAll("[data-evidence-items]"));

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
          <figcaption class="result-plot-caption">
            Checked figure for ${escapeHtml(item.title || "this benchmark result")}.
            <a class="result-plot-enlarge" href="${ROOT}/${escapeHtml(image.path)}">Open the full-size figure to inspect its axes, labels, and legend.</a>
          </figcaption>
        </figure>
      `,
      )
      .join("");
  }

  function renderCommandList(commands) {
    if (!Array.isArray(commands) || !commands.length) {
      return "<p>No reproduction command is listed.</p>";
    }
    const commandText = commands.join("\n");
    return `<pre class="result-commands" data-language="Shell · source checkout"><code>${escapeHtml(commandText)}</code></pre>`;
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
    return `https://github.com/nzy1997/rust-qec/blob/master/${path}`;
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
      return '<span class="provenance-muted">not recorded</span>';
    }
    if (Array.isArray(value)) {
      if (!value.length) {
        return '<span class="provenance-muted">empty</span>';
      }
      return `<ul class="provenance-value-list">${value.map((item) => `<li>${renderCompactValue(item)}</li>`).join("")}</ul>`;
    }
    if (typeof value === "object") {
      const entries = Object.entries(value);
      if (!entries.length) {
        return '<span class="provenance-muted">empty</span>';
      }
      return `<ul class="provenance-value-list">${entries
        .map(
          ([key, entryValue]) => `
            <li>
              <code>${escapeHtml(key)}</code>
              ${renderCompactValue(entryValue)}
            </li>
          `,
        )
        .join("")}</ul>`;
    }
    return `<span>${escapeHtml(value)}</span>`;
  }

  function githubRepositoryHref(repository) {
    if (typeof repository !== "string" || !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) {
      return "";
    }
    return `https://github.com/${repository
      .split("/")
      .map((part) => encodeURIComponent(part))
      .join("/")}`;
  }

  function renderExternalRepositoryCommits(entry) {
    if (entry && entry.status === "not_recorded") {
      return `<p class="provenance-muted">${escapeHtml(entry.reason || "External repository commits were not recorded.")}</p>`;
    }
    const recordedValue = entry && entry.value;
    if (recordedValue === null || recordedValue === undefined) {
      return '<p class="provenance-muted">No external repository commit value is recorded.</p>';
    }
    const commits = Array.isArray(recordedValue) ? recordedValue : [recordedValue];
    if (!commits.length) {
      return '<p class="provenance-muted">No external repository commits are listed.</p>';
    }
    return `<ul class="provenance-value-list external-repository-list">${commits
      .map((commitEntry) => {
        if (!commitEntry || typeof commitEntry !== "object" || Array.isArray(commitEntry)) {
          return `<li class="external-repository-entry">${renderCompactValue(commitEntry)}</li>`;
        }
        const repository = commitEntry.repository;
        const commit = commitEntry.commit;
        const repositoryHref = githubRepositoryHref(repository);
        const commitHref =
          repositoryHref && typeof commit === "string" && /^[0-9a-fA-F]{7,64}$/.test(commit)
            ? `${repositoryHref}/commit/${encodeURIComponent(commit)}`
            : "";
        const extraEntries = Object.entries(commitEntry).filter(
          ([key]) => key !== "repository" && key !== "commit",
        );
        return `
          <li class="external-repository-entry">
            <div>
              <strong>Repository:</strong>
              ${
                repositoryHref
                  ? `<a href="${escapeHtml(repositoryHref)}">${escapeHtml(repository)}</a>`
                  : renderCompactValue(repository || null)
              }
            </div>
            <div>
              <strong>Commit:</strong>
              ${
                commitHref
                  ? `<a href="${escapeHtml(commitHref)}"><code>${escapeHtml(commit)}</code></a>`
                  : renderCompactValue(commit || null)
              }
            </div>
            ${
              extraEntries.length
                ? `<ul class="provenance-value-list">${extraEntries
                    .map(
                      ([key, value]) => `<li><code>${escapeHtml(key)}</code> ${renderCompactValue(value)}</li>`,
                    )
                    .join("")}</ul>`
                : ""
            }
          </li>
        `;
      })
      .join("")}</ul>`;
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
            : field === "external_repository_commits"
              ? renderExternalRepositoryCommits(entry)
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

  function evidenceTierDescription(tier) {
    const descriptions = {
      full: "full benchmark run",
      smoke: "quick smoke run",
      readiness: "readiness check",
      release: "release-profile run",
      "local-pipeline": "local pipeline",
      "compatibility-gate": "compatibility gate",
      "regression-gate": "regression gate",
    };
    return descriptions[tier] || `run tier “${tier || "unspecified"}”`;
  }

  function renderEvidenceStatusSummary(family, item) {
    const tier = evidenceTierDescription(item.tier);
    let resultSummary;
    if (item.status === "existing") {
      resultSummary = `Checked artifacts are available for this ${tier}.`;
    } else if (item.status === "partial") {
      resultSummary = `Checked artifacts cover part of this ${tier}; the claims limit below defines its boundary.`;
    } else if (item.status === "local-only") {
      resultSummary = `This ${tier} is a local workflow; it does not provide checked site artifacts.`;
    } else {
      resultSummary = `This ${tier} has evidence status “${item.status || "unspecified"}”.`;
    }

    let familySummary = "";
    if (family.status === "partial") {
      familySummary = " The broader benchmark family has partial checked coverage.";
    } else if (family.status === "local-only") {
      familySummary = " The broader benchmark family is documented for local runs only.";
    }
    return `<p class="evidence-status-summary"><strong>Evidence status:</strong> ${escapeHtml(resultSummary + familySummary)}</p>`;
  }

  function renderEvidenceContainers(manifest) {
    evidenceContainers.forEach((container) => {
      const itemIds = String(container.dataset.evidenceItems || "")
        .split(/\s+/)
        .filter(Boolean);
      container.innerHTML = itemIds
        .map((itemId) => {
        const found = findEvidenceItem(manifest, itemId);
        if (!found) {
          return `<article class="result-card error"><h3>${escapeHtml(itemId)}</h3><p>Missing benchmark manifest item.</p></article>`;
        }
        const { family, item } = found;
        const plotHtml = renderImageArtifacts(item);
        return `
        <article
          class="result-card${plotHtml ? " has-plot" : ""}"
          data-family-status="${escapeHtml(family.status || "unspecified")}"
          data-item-status="${escapeHtml(item.status || "unspecified")}"
          data-evidence-tier="${escapeHtml(item.tier || "unspecified")}"
        >
          <div class="result-card-copy">
            <div class="manifest-heading">
              <div>
                <p class="eyebrow">${escapeHtml(family.title || family.id || "Benchmark family")}</p>
                <h3>${escapeHtml(item.title || item.id || "Benchmark evidence")}</h3>
              </div>
            </div>
            ${renderEvidenceStatusSummary(family, item)}
            <p><strong>Claims limit:</strong> ${escapeHtml(item.claims_limit || family.claims_limit || "No claims limit recorded.")}</p>
            ${renderTextList(item.caveats)}
            <details class="evidence-details evidence-reproduction">
              <summary>Reproduce this result</summary>
              <h4>Artifacts</h4>
              ${renderArtifactLinks(item)}
              <h4>Reproduction</h4>
              ${renderCommandList(item.commands)}
            </details>
            <details class="evidence-details evidence-provenance">
              <summary>Full provenance and sources</summary>
              <h4>Provenance</h4>
              ${renderProvenance(item.provenance)}
              <h4>Sources</h4>
              ${renderSourceLinks(item.provenance_sources || family.source_docs)}
            </details>
          </div>
          ${plotHtml ? `<div class="result-card-plot">${plotHtml}</div>` : ""}
        </article>
      `;
        })
        .join("");
    });
  }

  if (benchmarkManifest || evidenceContainers.length) {
    fetch(ROOT + "/data/benchmark-site.json")
      .then((response) => {
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        return response.json();
      })
      .then((manifest) => {
        renderBenchmarkManifest(manifest);
        renderEvidenceContainers(manifest);
      })
      .catch((error) => {
        if (benchmarkManifest) {
          benchmarkManifest.classList.add("error");
          benchmarkManifest.innerHTML = `
            <p>Benchmark manifest could not be loaded: ${escapeHtml(error.message)}</p>
            <p><a href="${ROOT}/data/benchmark-site.json">Open benchmark-site.json</a></p>
          `;
        }
        evidenceContainers.forEach((container) => {
          container.classList.add("error");
          container.innerHTML = `
              <p>Benchmark evidence could not be loaded: ${escapeHtml(error.message)}</p>
              <p><a href="${ROOT}/data/benchmark-site.json">Open benchmark-site.json</a></p>
            `;
        });
      });
  }
})();
