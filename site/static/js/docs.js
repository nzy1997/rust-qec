/* Progressive reading aids: content, reference links and menus work without JS. */
(() => {
  const main = document.querySelector('main');
  if (!main) return;
  const navShell = document.querySelector('.nav-shell');
  const navOffset = () => navShell && getComputedStyle(navShell).position === 'sticky'
    ? Math.ceil(navShell.getBoundingClientRect().height) + 16 : 16;
  const syncNavOffset = () => document.documentElement.style.setProperty('--docs-nav-offset', `${navOffset()}px`);
  syncNavOffset();
  if (navShell) new ResizeObserver(syncNavOffset).observe(navShell);
  const toc = document.querySelector('.page-toc');
  // Interactive widgets replace their headings and provide their own navigation.
  const headings = [...main.querySelectorAll('h2, h3, h4')].filter((heading) => !heading.closest('[data-toc-skip]'));
  const tocLinks = new Map();
  if (toc && headings.filter((h) => h.tagName === 'H2').length > 1 && !['home', 'shot'].includes(document.body.dataset.page)) {
    let section;
    let children;
    headings.forEach((heading) => {
      const title = heading.textContent.trim();
      if (!heading.id) {
        const slug = title.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'section';
        let id = slug;
        for (let n = 2; document.getElementById(id); n++) id = `${slug}-${n}`;
        heading.id = id;
      }
      const link = document.createElement('a');
      link.href = `#${heading.id}`;
      link.textContent = title;
      if (heading.tagName === 'H2' || !section) {
        section = document.createElement('div');
        section.className = 'toc-section';
        section.append(link);
        toc.querySelector('nav').append(section);
        children = null;
      } else {
        if (!children) {
          children = document.createElement('details');
          const summary = document.createElement('summary');
          summary.textContent = 'Subsections';
          children.append(summary);
          section.append(children);
        }
        link.className = heading.tagName === 'H4' ? 'toc-depth-4' : 'toc-depth-3';
        children.append(link);
      }
      tocLinks.set(heading, link);
      const permalink = document.createElement('a');
      permalink.className = 'heading-anchor';
      permalink.href = link.href;
      permalink.textContent = '#';
      permalink.setAttribute('aria-label', `Permalink to ${title}`);
      heading.append(permalink);
    });
    toc.hidden = false;
    toc.querySelector('.toc-disclosure').open = false;
    document.querySelector('.reading-layout').classList.add('has-toc');
    let pending = false;
    function updateCurrent() {
      pending = false;
      const offset = navOffset() + 50;
      let current = headings[0];
      for (const heading of headings) {
        if (heading.getClientRects().length && heading.getBoundingClientRect().top <= offset) current = heading;
      }
      for (const [heading, link] of tocLinks) {
        if (heading === current) {
          link.setAttribute('aria-current', 'location');
        } else link.removeAttribute('aria-current');
      }
    }
    window.addEventListener('scroll', () => {
      if (!pending) { pending = true; requestAnimationFrame(updateCurrent); }
    }, { passive: true });
    updateCurrent();
  }
  // The initial fragment may precede generated heading IDs and measured navigation.
  function revealFragment() {
    try {
      const target = document.getElementById(decodeURIComponent(location.hash.slice(1)));
      if (!target) return;
      for (let node = target; node; node = node.parentElement) {
        if (node.tagName === 'DETAILS') node.open = true;
      }
      target.scrollIntoView();
    } catch { /* An invalid URL escape has no matching heading. */ }
  }
  requestAnimationFrame(revealFragment);
  window.addEventListener('hashchange', () => requestAnimationFrame(revealFragment));
  function languageFor(pre, code) {
    if (pre.dataset.language) return pre.dataset.language;
    const lang = code.className.match(/language-([\w-]+)/)?.[1] || pre.dataset.lang;
    if (lang) return ({ rust: 'Rust', json: 'JSON', toml: 'TOML', bash: 'Shell', sh: 'Shell', python: 'Python' })[lang] || lang;
    const text = code.textContent.trim();
    if (/^[{[]/.test(text)) return 'JSON';
    if (/^(?:cargo |rustqec |rstim |make |python3 |mkdir |curl |#.*\n)/.test(text)) return 'Shell';
    return 'Text';
  }
  function highlight(code, language) {
    if (!/^(Rust|JSON|TOML|Shell|Python)/.test(language)) return;
    // Modest lexical color; preserve exact text for copying and execution.
    const pattern = /("(?:\\.|[^"\\])*"|'[^'\n]*'|\/\/[^\n]*|#[^\n]*|\b(?:use|fn|let|mut|for|in|if|return|pub|true|false|null|Ok|Err)\b|\b\d+(?:\.\d+)?\b)/g;
    const text = code.textContent;
    const fragment = document.createDocumentFragment();
    let last = 0;
    for (const match of text.matchAll(pattern)) {
      fragment.append(text.slice(last, match.index));
      const span = document.createElement('span');
      span.className = /^["']/.test(match[0]) ? 'token-string' : /^(\/\/|#)/.test(match[0]) ? 'token-comment' : /^\d/.test(match[0]) ? 'token-number' : 'token-keyword';
      span.textContent = match[0];
      fragment.append(span);
      last = match.index + match[0].length;
    }
    fragment.append(text.slice(last));
    code.replaceChildren(fragment);
  }
  function enhanceContent() {
    main.querySelectorAll('pre').forEach((pre) => {
      const code = pre.querySelector('code');
      if (!code || pre.dataset.enhanced) return;
      pre.dataset.enhanced = 'true';
      const output = pre.dataset.output === 'true';
      const label = document.createElement('span');
      label.textContent = output ? (pre.dataset.language || 'Expected output') : languageFor(pre, code);
      const bar = document.createElement('div');
      bar.className = output ? 'code-toolbar output-toolbar' : 'code-toolbar';
      bar.append(label);
      if (!output) {
        highlight(code, label.textContent);
        const button = document.createElement('button');
        button.type = 'button';
        button.textContent = 'Copy';
        button.setAttribute('aria-label', `Copy ${label.textContent}`);
        const status = document.createElement('span');
        status.className = 'copy-status';
        status.setAttribute('role', 'status');
        button.addEventListener('click', async () => {
          try { await navigator.clipboard.writeText(code.textContent); status.textContent = 'Copied'; }
          catch { status.textContent = 'Copy unavailable. Select the code and copy manually.'; }
        });
        bar.append(status, button);
      }
      pre.before(bar);
    });
    main.querySelectorAll('.prose table').forEach((table) => {
      if (!table.parentElement.classList.contains('table-wrap')) {
        const wrap = document.createElement('div');
        wrap.className = 'table-wrap';
        table.before(wrap);
        wrap.append(table);
      }
      if (document.body.dataset.page === 'support') {
        const headers = [...table.querySelectorAll('thead th')];
        const level = headers.findIndex((th) => th.textContent.trim().toLowerCase() === 'level');
        if (level >= 0) for (const row of table.rows) row.cells[level]?.classList.add('support-level');
      }
    });
  }
  enhanceContent();
  new MutationObserver(enhanceContent).observe(main, { childList: true, subtree: true });
  let figureDialog;
  document.addEventListener('click', (event) => {
    const trigger = event.target.closest('[data-figure-viewer], .result-plot > a');
    if (!trigger || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    event.preventDefault();
    if (!figureDialog) {
      figureDialog = document.createElement('dialog');
      figureDialog.className = 'figure-dialog';
      figureDialog.setAttribute('aria-label', 'Benchmark figure');
      figureDialog.innerHTML = '<div class="figure-dialog-bar"><strong>Benchmark figure</strong><button type="button" data-zoom aria-pressed="false">Zoom in</button><button type="button" data-close>Close figure</button></div><div class="figure-dialog-view"><img alt=""></div>';
      figureDialog.querySelector('[data-close]').addEventListener('click', () => figureDialog.close());
      figureDialog.querySelector('[data-zoom]').addEventListener('click', (e) => {
        const zoomed = figureDialog.querySelector('img').classList.toggle('is-zoomed');
        e.target.textContent = zoomed ? 'Fit figure' : 'Zoom in';
        e.target.setAttribute('aria-pressed', String(zoomed));
      });
      document.body.append(figureDialog);
    }
    const source = trigger.closest('figure').querySelector('img');
    const image = figureDialog.querySelector('img');
    image.src = source.src;
    image.alt = source.alt;
    image.classList.remove('is-zoomed');
    const zoom = figureDialog.querySelector('[data-zoom]');
    zoom.textContent = 'Zoom in'; zoom.setAttribute('aria-pressed', 'false');
    figureDialog.showModal();
    figureDialog.querySelector('[data-close]').focus();
    figureDialog.addEventListener('close', () => trigger.focus({ preventScroll: true }), { once: true });
  });
  const mobile = document.querySelector('.mobile-navigation');
  const docsNavigation = document.querySelector('.docs-navigation');
  const desktopNavigation = matchMedia('(min-width: 1200px)');
  const desktopSidebar = matchMedia('(min-width: 900px)');
  const syncMenus = () => {
    if (mobile) mobile.open = desktopNavigation.matches;
    if (docsNavigation) docsNavigation.open = desktopSidebar.matches;
  };
  syncMenus();
  desktopNavigation.addEventListener('change', syncMenus);
  desktopSidebar.addEventListener('change', syncMenus);
  mobile?.addEventListener('keydown', (event) => {
    if (event.key === 'Escape' && !event.target.closest('.nav-group') && !desktopNavigation.matches) {
      mobile.open = false;
      mobile.querySelector('summary').focus();
    }
  });
  const groups = [...document.querySelectorAll('.nav-group')];
  groups.forEach((group) => {
    group.addEventListener('toggle', () => {
      if (group.open) groups.filter((other) => other !== group).forEach((other) => { other.open = false; });
    });
    group.addEventListener('keydown', (event) => {
      if (event.key === 'Escape') { group.open = false; group.querySelector('summary').focus(); }
    });
  });
  document.addEventListener('click', (event) => {
    groups.filter((group) => !group.contains(event.target)).forEach((group) => { group.open = false; });
  });
})();
