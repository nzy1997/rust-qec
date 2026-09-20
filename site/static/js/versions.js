(() => {
  const select = document.querySelector('#docs-version');
  if (!select) return;
  const root = new URL(`${document.body.dataset.root}/`, location.href);
  const fallbackParameter = 'docs-version-fallback';
  fetch(new URL('versions.json', root))
    .then((response) => {
      if (!response.ok) throw new Error('Version catalog unavailable');
      return response.json();
    })
    .then((catalog) => {
      if (catalog.current !== document.body.dataset.docsVersion || catalog.versions.length < 2) return;
      const currentVersion = catalog.versions.find((entry) => entry.id === catalog.current);
      const currentUrl = new URL(location.href);
      const fallbackTitle = currentUrl.searchParams.get(fallbackParameter);
      if (fallbackTitle && currentVersion) {
        const notice = document.createElement('p');
        notice.className = 'version-fallback-notice';
        notice.setAttribute('role', 'status');
        notice.textContent = `“${fallbackTitle}” is not available in ${currentVersion.label}. Showing this version's documentation home instead.`;
        document.querySelector('.version-strip')?.append(notice);
        currentUrl.searchParams.delete(fallbackParameter);
        history.replaceState(null, '', currentUrl);
      }
      for (const version of catalog.versions) {
        select.add(new Option(version.label, version.id, false, version.id === catalog.current));
      }
      select.addEventListener('change', () => {
        const version = catalog.versions.find((entry) => entry.id === select.value);
        if (!version) return;
        let route = location.pathname.slice(root.pathname.length).replace(/index\.html$/, '');
        const missingPage = !(route in version.pages);
        if (missingPage) route = '';
        const target = new URL(route, new URL(version.root, root));
        target.search = location.search;
        if (missingPage) {
          target.searchParams.set(fallbackParameter, document.title.replace(/ — RustQEC$/, ''));
        }
        let anchor;
        try { anchor = decodeURIComponent(location.hash.slice(1)); } catch { anchor = ''; }
        if (version.pages[route].includes(anchor)) target.hash = location.hash;
        location.assign(target.href);
      });
      document.querySelector('[data-version-label]').hidden = true;
      document.querySelector('label[for="docs-version"]').hidden = false;
      select.hidden = false;
    })
    // Offline or failed fetch: retain the server-rendered current-version label.
    .catch(() => {});
})();
