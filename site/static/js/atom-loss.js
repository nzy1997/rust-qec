/* Preserve incoming evidence links after separating the tutorial and report. */
(() => {
  const legacy = new Set(['atom-loss-results', 'loss-correctness', 'loss-sampling-throughput',
    'loss-logical-error-rate', 'loss-accuracy-time', 'loss-seed-accuracy',
    'loss-timing-sweep', 'loss-adapter-stages']);
  const routeLegacy = () => {
    if (/\/atom-loss\/$/.test(location.pathname) && legacy.has(location.hash.slice(1))) {
      location.replace(new URL(`../atom-loss-evidence/${location.hash}`, location.href));
    }
  };
  routeLegacy();
  window.addEventListener('hashchange', routeLegacy);
  const desktop = matchMedia('(min-width: 1250px)');
  const disclosure = document.querySelector('.toc-disclosure');
  const sync = () => { if (disclosure) disclosure.open = desktop.matches; };
  sync();
  desktop.addEventListener('change', sync);
})();
