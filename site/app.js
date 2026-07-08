(function () {
  const navList = document.getElementById("schema-nav-list");
  const detail = document.getElementById("schema-detail");
  const status = document.getElementById("schema-status");

  const groups = [
    {
      label: "Document",
      nodes: [{ id: "document", label: "QP101 document", schemaPath: [] }],
    },
    {
      label: "Operations",
      defs: [
        "gateOperation",
        "repeatOperation",
        "tickOperation",
        "qubitCoordsOperation",
        "shiftCoordsOperation",
        "detectorOperation",
        "observableIncludeOperation",
        "noiseOperation",
        "annotationOperation",
      ],
    },
    {
      label: "Target references",
      defs: [
        "qubitTargetRef",
        "recTargetRef",
        "pauliTargetRef",
        "combinerTargetRef",
        "sweepTargetRef",
      ],
    },
    {
      label: "Shared definitions",
      defs: ["annotation", "annotationStyle", "display", "operation", "targetRef"],
    },
  ];

  function resolveRef(schema, ref) {
    if (!ref || !ref.startsWith("#/")) {
      return null;
    }
    return ref
      .slice(2)
      .split("/")
      .reduce((current, part) => {
        if (!current) {
          return null;
        }
        return current[part.replace(/~1/g, "/").replace(/~0/g, "~")];
      }, schema);
  }

  function schemaType(schema) {
    if (!schema) {
      return "unknown";
    }
    if (schema.const !== undefined) {
      return `const ${JSON.stringify(schema.const)}`;
    }
    if (schema.enum) {
      return `enum ${schema.enum.join(" | ")}`;
    }
    if (schema.type) {
      return Array.isArray(schema.type) ? schema.type.join(" | ") : schema.type;
    }
    if (schema.$ref) {
      return schema.$ref.replace("#/$defs/", "");
    }
    if (schema.oneOf) {
      return "oneOf";
    }
    if (schema.anyOf) {
      return "anyOf";
    }
    return "schema";
  }

  function titleFromKey(key) {
    return key
      .replace(/Operation$/, " operation")
      .replace(/TargetRef$/, " target")
      .replace(/([a-z])([A-Z])/g, "$1 $2")
      .replace(/^./, (char) => char.toUpperCase());
  }

  function collectNodes(schema) {
    return groups.map((group) => {
      if (group.nodes) {
        return group;
      }

      return {
        label: group.label,
        nodes: group.defs.map((defName) => ({
          id: defName,
          label: titleFromKey(defName),
          schemaPath: ["$defs", defName],
          schema: schema.$defs[defName],
        })),
      };
    });
  }

  function nodeSchema(root, node) {
    if (node.schema) {
      return node.schema;
    }
    if (node.schemaPath.length === 0) {
      return root;
    }
    return node.schemaPath.reduce((current, part) => current && current[part], root);
  }

  function renderMeta(schema, root) {
    const badges = [];
    badges.push(`<span class="badge">${escapeHtml(schemaType(schema))}</span>`);
    if (schema.required && schema.required.length) {
      badges.push(`<span class="badge">${schema.required.length} required</span>`);
    }
    if (schema.properties) {
      badges.push(`<span class="badge">${Object.keys(schema.properties).length} fields</span>`);
    }
    if (schema.oneOf) {
      badges.push(`<span class="badge">${schema.oneOf.length} variants</span>`);
    }
    if (schema.anyOf) {
      badges.push(`<span class="badge">${schema.anyOf.length} alternatives</span>`);
    }
    if (schema.$ref) {
      const resolved = resolveRef(root, schema.$ref);
      badges.push(`<span class="badge">ref ${escapeHtml(schemaType(resolved))}</span>`);
    }
    return `<div class="schema-meta">${badges.join("")}</div>`;
  }

  function renderVariantList(schema) {
    const variants = schema.oneOf || schema.anyOf;
    if (!variants) {
      return "";
    }
    const items = variants
      .map((variant) => {
        const label = variant.$ref ? variant.$ref.replace("#/$defs/", "") : schemaType(variant);
        return `<li><code>${escapeHtml(label)}</code></li>`;
      })
      .join("");
    return `<h4>Variants</h4><ul>${items}</ul>`;
  }

  function renderFields(schema, root) {
    if (!schema.properties) {
      return "";
    }

    const required = new Set(schema.required || []);
    const rows = Object.entries(schema.properties)
      .map(([name, property]) => {
        const resolved = property.$ref ? resolveRef(root, property.$ref) : property;
        const description = property.description || (resolved && resolved.description) || "";
        const requiredBadge = required.has(name)
          ? '<span class="badge">required</span>'
          : '<span class="badge">optional</span>';
        return `
          <div class="field">
            <div class="field-name">
              <code>${escapeHtml(name)}</code>
              ${requiredBadge}
              <span class="badge">${escapeHtml(schemaType(property))}</span>
            </div>
            ${description ? `<p class="field-description">${escapeHtml(description)}</p>` : ""}
          </div>
        `;
      })
      .join("");
    return `<h4>Fields</h4><div class="field-list">${rows}</div>`;
  }

  function escapeHtml(value) {
    return String(value)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function renderDetail(root, node) {
    const schema = nodeSchema(root, node);
    detail.innerHTML = `
      <h3>${escapeHtml(node.label)}</h3>
      ${renderMeta(schema, root)}
      ${schema.description ? `<p>${escapeHtml(schema.description)}</p>` : ""}
      ${renderFields(schema, root)}
      ${renderVariantList(schema)}
    `;
  }

  const benchmarkManifest = document.getElementById("benchmark-manifest");
  const checkedBenchmarkResults = document.getElementById("checked-benchmark-result-cards");
  const checkedBenchmarkItems = ["surface-decoder-full", "bb-circuit-full", "rstim-vs-stim-full"];

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
          <a href="${escapeHtml(artifact.path)}">${escapeHtml(fileName(artifact.path))}</a>
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
          <a href="${escapeHtml(image.path)}">
            <img src="${escapeHtml(image.path)}" alt="${escapeHtml(item.title || "Checked benchmark plot")}">
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

  function renderSourceLinks(paths) {
    if (!Array.isArray(paths) || !paths.length) {
      return "";
    }
    const links = paths
      .map((path) => `<li><a href="${escapeHtml(path)}">${escapeHtml(path)}</a></li>`)
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

  function renderNav(root, groupedNodes) {
    navList.innerHTML = "";
    const allButtons = [];

    groupedNodes.forEach((group) => {
      const label = document.createElement("div");
      label.className = "group-label";
      label.textContent = group.label;
      navList.appendChild(label);

      group.nodes.forEach((node) => {
        const button = document.createElement("button");
        button.type = "button";
        button.textContent = node.label;
        button.addEventListener("click", () => {
          allButtons.forEach((item) => item.classList.remove("active"));
          button.classList.add("active");
          renderDetail(root, node);
        });
        navList.appendChild(button);
        allButtons.push(button);
      });
    });

    if (allButtons.length) {
      allButtons[0].click();
    }
  }

  if (benchmarkManifest || checkedBenchmarkResults) {
    fetch("data/benchmark-site.json")
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
            <p><a href="data/benchmark-site.json">Open benchmark-site.json</a></p>
          `;
        }
        if (checkedBenchmarkResults) {
          checkedBenchmarkResults.classList.add("error");
          checkedBenchmarkResults.innerHTML = `
            <p>Checked benchmark results could not be loaded: ${escapeHtml(error.message)}</p>
            <p><a href="data/benchmark-site.json">Open benchmark-site.json</a></p>
          `;
        }
      });
  }

  fetch("qp101.schema.json")
    .then((response) => {
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }
      return response.json();
    })
    .then((schema) => {
      status.textContent = "Loaded";
      renderNav(schema, collectNodes(schema));
    })
    .catch((error) => {
      status.textContent = "Error";
      detail.classList.add("error");
      detail.innerHTML = `
        <h3>Schema could not be loaded</h3>
        <p>${escapeHtml(error.message)}</p>
        <p><a href="qp101.schema.json" download>Download qp101.schema.json</a></p>
      `;
    });
})();
