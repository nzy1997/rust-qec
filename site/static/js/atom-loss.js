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

/* Decoder maturity labels are rendered from the version-bound support matrix
   (_site/support/envelope-support.json, copied verbatim from
   docs/envelope-support.json), never from hand-edited markup. The static
   badge defaults to Beta; it upgrades to "Supported · <release>" only when
   the matrix both declares the decoder supported and names the release whose
   evidence bundle carries the promise. Missing, failed, or pre-release
   (v0.3.0) evidence leaves the Beta label untouched, and each decoder is
   resolved independently so MLE cannot inherit Matching's badge. */
(() => {
  const DECODER_LABELS = {
    'envelope-matching': 'Envelope matching',
    'envelope-mle': 'Envelope MLE',
  };
  const upgradeDecoderBadges = async () => {
    const badges = [...document.querySelectorAll('[data-decoder-badge]')];
    if (!badges.length) return;
    let matrix;
    try {
      const response = await fetch(new URL('../support/envelope-support.json', location.href));
      if (!response.ok) return;
      matrix = await response.json();
    } catch {
      return;
    }
    const decoders = matrix && matrix.schema_version === 'rustqec.envelope-support.v1'
      ? matrix.decoders : null;
    if (!decoders) return;
    for (const badge of badges) {
      const name = badge.dataset.decoderBadge;
      const decoder = decoders[name];
      const published = decoder && decoder.published_support;
      if (!decoder || decoder.current_maturity !== 'supported' || !published
          || published.maturity !== 'supported'
          || typeof published.release !== 'string' || !published.release
          || typeof published.release_url !== 'string' || !published.release_url
          || typeof published.evidence_asset !== 'string' || !published.evidence_asset) {
        continue;
      }
      const link = document.createElement('a');
      link.href = published.release_url;
      link.textContent = `${DECODER_LABELS[name] || name} · Supported · ${published.release}`;
      link.title = `Verified by ${published.evidence_asset}`;
      badge.textContent = '';
      badge.appendChild(link);
      badge.dataset.release = published.release;
      badge.dataset.evidenceAsset = published.evidence_asset;
      badge.classList.add('loss-badge-supported');
    }
  };
  upgradeDecoderBadges();
})();
