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
  const compact = matchMedia('(max-width: 1249px)');
  const toc = document.querySelector('.page-toc');
  const disclosure = document.querySelector('.toc-disclosure');
  const sync = () => { if (disclosure) disclosure.open = desktop.matches; };
  const syncTocOffset = () => {
    const height = compact.matches && toc && !toc.hidden
      ? Math.ceil(toc.getBoundingClientRect().height) + 16
      : 0;
    document.documentElement.style.setProperty('--loss-toc-offset', `${height}px`);
  };
  const revealFragment = () => {
    const target = document.getElementById(decodeURIComponent(location.hash.slice(1)));
    if (target) target.scrollIntoView();
  };
  sync();
  syncTocOffset();
  desktop.addEventListener('change', sync);
  compact.addEventListener('change', syncTocOffset);
  if (toc) new ResizeObserver(syncTocOffset).observe(toc);
  window.addEventListener('hashchange', () => {
    if (!compact.matches || !disclosure) return;
    disclosure.open = false;
    syncTocOffset();
    requestAnimationFrame(revealFragment);
  });
  document.addEventListener('click', (event) => {
    const link = event.target.closest('.page-toc a[href^="#"]');
    if (!link || !compact.matches || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    const target = document.getElementById(decodeURIComponent(link.hash.slice(1)));
    if (!target) return;
    event.preventDefault();
    disclosure.open = false;
    syncTocOffset();
    if (location.hash !== link.hash) location.hash = link.hash;
    requestAnimationFrame(revealFragment);
  });
})();
