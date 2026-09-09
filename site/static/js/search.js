/* Local, build-generated full-text search; no query is sent to a search service. */
(() => {
  const form = document.querySelector('.docs-search');
  const input = document.getElementById('docs-query');
  const status = document.getElementById('search-status');
  const results = document.getElementById('search-results');
  if (!form || !input) return;
  const root = new URL(`${document.body.dataset.root}/`, location.href);
  let indexPromise;
  let revision = 0;
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
      const terms = query.toLowerCase().split(/\s+/);
      const matches = index.filter((page) => terms.every((term) => `${page.title} ${page.text}`.toLowerCase().includes(term)))
        .map((page) => ({ ...page, score: terms.filter((term) => page.title.toLowerCase().includes(term)).length }))
        .sort((a, b) => b.score - a.score || a.title.localeCompare(b.title));
      status.textContent = matches.length ? `${matches.length} matching pages` : 'No matching pages. Try a command, crate, or shorter term, or browse below.';
      for (const page of matches) {
        const item = document.createElement('li');
        const link = document.createElement('a');
        link.href = new URL(page.path, root);
        link.textContent = page.title;
        const snippet = document.createElement('p');
        const offset = Math.max(0, page.text.toLowerCase().indexOf(terms[0]) - 65);
        snippet.textContent = `${offset ? '…' : ''}${page.text.slice(offset, offset + 230)}…`;
        item.append(link, snippet);
        results.append(item);
      }
    } catch {
      if (request === revision) status.textContent = 'Search is unavailable. Browse the index below or submit your query again to retry.';
    }
  }
  form.addEventListener('submit', (event) => { event.preventDefault(); search(); });
  let timer;
  input.addEventListener('input', () => { ++revision; clearTimeout(timer); timer = setTimeout(search, 150); });
  input.value = new URL(location.href).searchParams.get('q') || '';
  if (input.value) search();
})();
