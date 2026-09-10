/* Section search stays on this device; render all indexed content as text. */
(() => {
  const form = document.querySelector('.docs-search');
  const input = document.getElementById('docs-query');
  const status = document.getElementById('search-status');
  const results = document.getElementById('search-results');
  if (!form || !input) return;
  const root = new URL(`${document.body.dataset.root}/`, location.href);
  const normalize = (text) => text.toLowerCase().replace(/[-_]/g, ' ');
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
      status.textContent = 'Browse the index below, or search across the documentation.';
      return;
    }
    status.textContent = 'Searching…';
    try {
      indexPromise ||= fetch(form.dataset.searchIndex).then((response) => {
        if (!response.ok) throw new Error('Index unavailable');
        return response.json();
      }).catch((error) => { indexPromise = undefined; throw error; });
      const index = await indexPromise;
      if (request !== revision) return;
      const terms = normalize(query).split(/\s+/).filter(Boolean);
      if (!terms.length) { status.textContent = 'Enter a command, crate, or concept to search.'; return; }
      const phrase = terms.join(' ');
      const matches = index.filter((entry) => terms.every((term) => normalize(`${entry.page_title} ${entry.title} ${entry.text}`).includes(term)))
        .map((entry) => ({ ...entry, score: (normalize(entry.title).includes(phrase) ? 20 : 0)
          + terms.filter((term) => normalize(entry.title).includes(term)).length * 5
          + (normalize(entry.excerpt || '').includes(phrase) ? 3 : 0) }))
        .sort((a, b) => b.score - a.score || a.title.localeCompare(b.title));
      status.textContent = matches.length ? `${matches.length} matching sections${matches.length > 20 ? ' · showing the first 20' : ''}` : 'No matching sections. Try a command, crate, or shorter term, or browse below.';
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
        const sentences = prose.match(/[^.!?]+[.!?]?(?:\s+|$)/g) || [prose];
        const start = Math.max(0, sentences.findIndex((sentence) => terms.every((term) => normalize(sentence).includes(term))));
        const excerpt = sentences.slice(start).join('').trim();
        const shortened = excerpt.length > 260 ? `${excerpt.slice(0, 257).replace(/\s+\S*$/, '')}…` : excerpt;
        snippet.append(highlighted(shortened, terms));
        item.append(breadcrumb, link, snippet);
        results.append(item);
      }
    } catch {
      if (request === revision) status.textContent = 'Search is unavailable. Browse the index below or submit your query again to retry.';
    }
  }
  form.addEventListener('submit', (event) => { event.preventDefault(); clearTimeout(timer); search(); });
  let timer;
  input.addEventListener('input', () => { ++revision; clearTimeout(timer); timer = setTimeout(search, 150); });
  input.value = new URL(location.href).searchParams.get('q') || '';
  if (input.value) search();
})();
