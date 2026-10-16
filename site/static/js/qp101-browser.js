(function () {
  const ROOT = (document.body && document.body.dataset && document.body.dataset.root) || ".";
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
  fetch(ROOT + "/qp101.schema.json")
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
        <p><a href="${ROOT}/qp101.schema.json" download>Download qp101.schema.json</a></p>
      `;
    });
})();
