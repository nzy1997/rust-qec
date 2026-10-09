(() => {
  const routes = {
    "simulator": { fallback: "get-started/#first-circuit", anchors: {
      "save-samples": "sampling-data/#sample", "circuit-simulation": "get-started/#first-circuit",
      "circuit-simulation-title": "get-started/#first-circuit", "simulation-results-title": "benchmarks/simulation/#simulation-results-title",
      "sampling-evidence-title": "benchmarks/simulation/#sampling-evidence-title" } },
    "detector-models": { fallback: "get-started/#detector-output", anchors: {
      "dem-extraction": "get-started/#detector-output", "dem-extraction-title": "get-started/#detector-output",
      "sample-dem": "reference/#help-rstim-sample_dem", "dem-results-title": "benchmarks/simulation/#dem-results-title",
      "dem-evidence-title": "benchmarks/simulation/#dem-evidence-title" } },
    "atom-loss-concepts": { fallback: "atom-loss/#loss-record", anchors: {
      "loss-record": "atom-loss/#loss-record", "data-boundary": "atom-loss/#sample-loss",
      "choose-decoder": "atom-loss/#choose-decoder", "supported-circuits": "support/#atom-loss-support-boundary",
      "next-step": "atom-loss/#model-loss" } }
  };
  const page = location.pathname.split("/").filter(Boolean).pop();
  const rule = routes[page];
  if (!rule) return;
  const hash = location.hash.slice(1);
  const target = rule.anchors[hash] || rule.fallback;
  const url = new URL("../" + target, location.href);
  url.search = location.search;
  location.replace(url.href);
})();
