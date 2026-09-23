/* Section search stays on this device; render all indexed content as text. */
(() => {
  const form = document.querySelector('.nav-search');
  const input = document.getElementById('site-search');
  const panel = document.querySelector('[data-docs-search-results]');
  const status = document.getElementById('search-status');
  const results = document.getElementById('search-results');
  if (!form || !input || !panel || !status || !results) return;
  const root = new URL(`${document.body.dataset.root}/`, location.href);
  const normalize = (text) => text.toLowerCase().replace(/[-_]/g, ' ');
  function excerptAroundMatch(prose, terms, limit = 260) {
    if (prose.length <= limit) return prose;
    const normalized = normalize(prose);
    const positions = terms
      .map((term) => normalized.indexOf(term))
      .filter((position) => position >= 0);
    const matchPosition = positions.length ? Math.min(...positions) : 0;
    let start = Math.max(0, matchPosition - 80);
    let end = Math.min(prose.length, start + limit);
    if (end === prose.length) start = Math.max(0, end - limit);
    if (start > 0) {
      const nextSpace = prose.indexOf(' ', start);
      if (nextSpace >= 0 && nextSpace < matchPosition) start = nextSpace + 1;
    }
    if (end < prose.length) {
      const previousSpace = prose.lastIndexOf(' ', end);
      if (previousSpace > matchPosition) end = previousSpace;
    }
    return `${start > 0 ? '…' : ''}${prose.slice(start, end).trim()}${end < prose.length ? '…' : ''}`;
  }
  let indexPromise;
  let revision = 0;
  function highlighted(text, terms) {
    const fragment = document.createDocumentFragment();
    const escaped = terms.map((term) => term.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'));
    const pattern = new RegExp(`(${escaped.join('|')})`, 'gi');
    let last = 0;
    for (const match of text.matchAll(pattern)) {
      fragment.append(text.slice(last, match.index));
      const mark = document.createElement('mark');
      mark.textContent = match[0];
      fragment.append(mark);
      last = match.index + match[0].length;
    }
    fragment.append(text.slice(last));
    return fragment;
  }
  async function search() {
    const request = ++revision;
    const query = input.value.trim();
    const url = new URL(location.href);
    if (query) url.searchParams.set('q', query); else url.searchParams.delete('q');
    history.replaceState(null, '', url);
    results.replaceChildren();
    if (!query) {
      panel.hidden = true;
      status.textContent = '';
      return;
    }
    panel.hidden = false;
    status.textContent = 'Searching…';
    try {
      indexPromise ||= fetch(panel.dataset.searchIndex).then((response) => {
        if (!response.ok) throw new Error('Index unavailable');
        return response.json();
      }).catch((error) => { indexPromise = undefined; throw error; });
      const index = await indexPromise;
      if (request !== revision) return;
      const terms = normalize(query).split(/\s+/).filter(Boolean);
      if (!terms.length) { status.textContent = 'Enter a command, crate, or concept to search.'; return; }
      const phrase = terms.join(' ');
      const matches = index.filter((entry) => terms.every((term) => normalize(`${entry.page_title} ${entry.title} ${entry.text}`).includes(term)))
        .map((entry) => {
          const pageTitle = normalize(entry.page_title.replace(/ — RustQEC$/, ''));
          const focusedPageBonus = pageTitle.includes(phrase)
            ? Math.max(0, 30 - (pageTitle.length - phrase.length))
            : 0;
          return { ...entry, score: focusedPageBonus
            + terms.filter((term) => pageTitle.includes(term)).length * 5
            + (normalize(entry.title).includes(phrase) ? 20 : 0)
            + terms.filter((term) => normalize(entry.title).includes(term)).length * 5
            + (normalize(entry.excerpt || '').includes(phrase) ? 3 : 0) };
        })
        .sort((a, b) => b.score - a.score || a.title.localeCompare(b.title));
      status.textContent = matches.length ? `${matches.length} matching sections${matches.length > 20 ? ' · showing the first 20' : ''}` : 'No matching sections. Try a command, crate, or shorter term.';
      for (const entry of matches.slice(0, 20)) {
        const item = document.createElement('li');
        const breadcrumb = document.createElement('p');
        breadcrumb.className = 'search-path';
        breadcrumb.textContent = entry.page_title.replace(/ — RustQEC$/, '');
        const link = document.createElement('a');
        link.href = new URL(entry.path, root);
        link.append(highlighted(entry.title, terms));
        const snippet = document.createElement('p');
        const prose = entry.excerpt || 'Open this section for commands, examples, and reference details.';
        const shortened = excerptAroundMatch(prose, terms);
        snippet.append(highlighted(shortened, terms));
        item.append(breadcrumb, link, snippet);
        results.append(item);
      }
    } catch {
      if (request === revision) status.textContent = 'Search is unavailable. Use the documentation navigation or submit your query again to retry.';
    }
  }
  form.addEventListener('submit', (event) => { event.preventDefault(); clearTimeout(timer); search(); });
  let timer;
  input.addEventListener('input', () => { ++revision; clearTimeout(timer); timer = setTimeout(search, 150); });
  input.value = new URL(location.href).searchParams.get('q') || '';
  if (input.value) search().then(() => {
    if (location.hash === '#documentation-search') panel.scrollIntoView();
  });
})();
