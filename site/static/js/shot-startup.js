(() => {
  const status = document.querySelector('[data-shot-startup]');
  const viewer = document.querySelector('#shot-viewer');
  const moduleScript = document.querySelector('[data-shot-module]');
  if (!status || !viewer) return;

  let settled = false;
  const recovery = status.querySelector('[data-shot-recovery]');
  const message = status.querySelector('[data-shot-message]');
  const showFailure = (reason) => {
    if (settled) return;
    status.hidden = false;
    status.dataset.state = 'error';
    status.setAttribute('role', 'alert');
    message.textContent = reason;
    recovery.hidden = false;
  };
  const showReady = () => {
    settled = true;
    status.hidden = true;
    observer.disconnect();
  };
  const inspect = () => {
    const workspace = viewer.querySelector('#shot-workspace');
    const empty = viewer.querySelector('#shot-empty');
    const error = viewer.querySelector('#shot-error:not([hidden])');
    if (error && error.textContent.trim()) {
      showFailure(`${error.textContent.trim()} Refresh the page, or use the non-interactive circuit guide below.`);
    } else if ((workspace && !workspace.hidden) || (empty && !empty.hidden)) {
      showReady();
    } else if (viewer.querySelector('#shot-loading')) {
      status.hidden = true;
    }
  };

  if (!('WebAssembly' in window)) {
    showFailure('This browser does not provide WebAssembly, which Shot Lab needs to run the simulator.');
    return;
  }

  const observer = new MutationObserver(inspect);
  observer.observe(viewer, { childList: true, subtree: true, attributes: true, attributeFilter: ['hidden'] });
  moduleScript?.addEventListener('error', () => {
    showFailure('The Shot Lab application could not be loaded. Check the connection and content blocker, then refresh the page.');
  });
  window.addEventListener('unhandledrejection', (event) => {
    const reason = event.reason?.message || String(event.reason || 'Unknown startup error');
    if (/wasm|webassembly|shot|circuit|fetch|load/i.test(reason)) showFailure(`Shot Lab could not start: ${reason}`);
  });
  window.setTimeout(() => {
    if (!settled && !viewer.querySelector('#shot-error:not([hidden])')) {
      showFailure('Shot Lab is taking longer than expected to start. Refresh the page or continue with the non-interactive circuit guide.');
    }
  }, 12000);
})();
