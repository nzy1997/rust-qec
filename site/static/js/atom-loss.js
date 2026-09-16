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

/* Decoder maturity labels default to Beta. The support matrix declares the
   intended release, but is not proof that publication happened. A badge and
   its surrounding prose upgrade only after the release API shows the small
   verification marker that the workflow uploads *after* it downloads and
   verifies the published archives and evidence bundle. Missing markers leave
   every claim at Beta. */
(() => {
  const DECODER_LABELS = {
    'envelope-matching': 'Envelope matching',
    'envelope-mle': 'Envelope MLE',
  };
  const upgradeDecoderBadges = async () => {
    const badges = [...document.querySelectorAll('[data-decoder-badge]')];
    const names = new Set([
      ...badges.map(badge => badge.dataset.decoderBadge),
      ...[...document.querySelectorAll('[data-decoder-support-copy]')]
        .map(copy => copy.dataset.decoderSupportCopy),
      ...[...document.querySelectorAll('[data-evidence-link]')]
        .map(link => link.dataset.evidenceLink),
    ]);
    if (!names.size) return;
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
    for (const name of names) {
      const decoder = decoders[name];
      const published = decoder && decoder.published_support;
      if (!decoder || decoder.current_maturity !== 'supported' || !published
          || published.maturity !== 'supported'
          || typeof published.release !== 'string' || !published.release
          || typeof published.release_url !== 'string' || !published.release_url
          || typeof published.evidence_asset !== 'string' || !published.evidence_asset
          || typeof published.verification_asset !== 'string' || !published.verification_asset
          || typeof published.verification_url !== 'string' || !published.verification_url) {
        continue;
      }
      let release;
      try {
        const response = await fetch(published.verification_url, { cache: 'no-store' });
        if (!response.ok) continue;
        release = await response.json();
      } catch {
        continue;
      }
      const assets = new Map(Array.isArray(release && release.assets)
        ? release.assets.filter(asset => asset && asset.state === 'uploaded')
          .map(asset => [asset.name, asset])
        : []);
      const evidenceAsset = assets.get(published.evidence_asset);
      const sidecarAsset = assets.get(`${published.evidence_asset}.sha256`);
      const markerAsset = assets.get(published.verification_asset);
      const labelParts = typeof (markerAsset && markerAsset.label) === 'string'
        ? markerAsset.label.split(';') : [];
      const label = Object.fromEntries(labelParts.slice(1).map(part => {
        const equals = part.indexOf('=');
        return equals > 0 ? [part.slice(0, equals), part.slice(equals + 1)] : ['', ''];
      }));
      if (!release || release.tag_name !== published.release || release.draft === true
          || typeof release.published_at !== 'string' || !release.published_at
          || !evidenceAsset || !sidecarAsset || !markerAsset
          || labelParts[0] !== 'rustqec-envelope-verification-v1'
          || label.tag !== published.release || label.status !== 'pass'
          || !/^[0-9a-f]{40}$/.test(label.source || '')
          || label.evidence !== evidenceAsset.digest
          || label.marker !== markerAsset.digest
          || !(label.supported || '').split(',').includes(name)) {
        continue;
      }
      for (const badge of badges.filter(item => item.dataset.decoderBadge === name)) {
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
      for (const copy of document.querySelectorAll(`[data-decoder-support-copy="${name}"]`)) {
        copy.textContent = `Supported since ${published.release}`;
      }
      for (const placeholder of document.querySelectorAll(`[data-evidence-link="${name}"]`)) {
        const evidenceLink = document.createElement('a');
        evidenceLink.href = published.release_url;
        evidenceLink.dataset.evidenceLink = name;
        evidenceLink.textContent = 'version-bound evidence bundle';
        placeholder.replaceWith(evidenceLink);
      }
    }
  };
  upgradeDecoderBadges();
})();
