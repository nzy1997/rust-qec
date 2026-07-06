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

  if (benchmarkManifest) {
    fetch("data/benchmark-site.json")
      .then((response) => {
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        return response.json();
      })
      .then((manifest) => {
        renderBenchmarkManifest(manifest);
      })
      .catch((error) => {
        benchmarkManifest.classList.add("error");
        benchmarkManifest.innerHTML = `
          <p>Benchmark manifest could not be loaded: ${escapeHtml(error.message)}</p>
          <p><a href="data/benchmark-site.json">Open benchmark-site.json</a></p>
        `;
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
