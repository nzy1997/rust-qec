/* Progressive reading aids: the document and native navigation work without JS. */
(() => {
  const main = document.querySelector('main');
  if (!main) return;
  const toc = document.querySelector('.page-toc');
  const headings = [...main.querySelectorAll('h2')];
  if (toc && headings.length > 1 && document.body.dataset.page !== 'shot') {
    headings.forEach((heading, index) => {
      if (!heading.id) heading.id = `section-${index + 1}`;
      const link = document.createElement('a');
      link.href = `#${heading.id}`;
      link.textContent = heading.textContent;
      toc.querySelector('nav').append(link);
    });
    toc.hidden = false;
    document.querySelector('.reading-layout').classList.add('has-toc');
  }
  function addCopyButtons() {
    main.querySelectorAll('pre').forEach((pre) => {
      const code = pre.querySelector('code');
      if (!code || pre.dataset.output === 'true' || pre.previousElementSibling?.classList.contains('code-toolbar')) return;
      const bar = document.createElement('div');
      bar.className = 'code-toolbar';
      const label = document.createElement('span');
      label.textContent = pre.dataset.language || 'Code';
      const button = document.createElement('button');
      button.type = 'button';
      button.textContent = 'Copy';
      button.setAttribute('aria-label', `Copy ${label.textContent}`);
      const status = document.createElement('span');
      status.className = 'copy-status';
      status.setAttribute('role', 'status');
      button.addEventListener('click', async () => {
        try {
          await navigator.clipboard.writeText(code.textContent);
          status.textContent = 'Copied';
        } catch {
          status.textContent = 'Copy unavailable. Select the code and copy manually.';
        }
      });
      bar.append(label, status, button);
      pre.before(bar);
    });
  }
  addCopyButtons();
  new MutationObserver(addCopyButtons).observe(main, { childList: true, subtree: true });
  const groups = [...document.querySelectorAll('.nav-group')];
  groups.forEach((group) => {
    group.addEventListener('toggle', () => {
      if (group.open) groups.filter((other) => other !== group).forEach((other) => { other.open = false; });
    });
    group.addEventListener('keydown', (event) => {
      if (event.key === 'Escape') {
        group.open = false;
        group.querySelector('summary').focus();
      }
    });
  });
  document.addEventListener('click', (event) => {
    groups.filter((group) => !group.contains(event.target)).forEach((group) => { group.open = false; });
  });
})();
