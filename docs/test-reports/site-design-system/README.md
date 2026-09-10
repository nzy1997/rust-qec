# Documentation site layout and navigation

Captured from the local built site with the Chromium `captures the primary docs and Shot Lab surfaces for review` Playwright test. The home, quickstart, and Shot Lab use the same 1280px desktop viewport; captures include the full page.

- [Homepage](home.png): aligned layout, four workflow stages, explicit installation and library routes.
- [Quickstart](quickstart.png): persistent documentation navigation and an initially collapsed page contents list.
- [Shot Lab](shot-lab.png): shared page boundaries, circuit controls, and the selected-event panel.

Validation for this change:

```sh
make -o build-shot-viewer build-site
python3 tools/check_site_build.py _site
python3 tools/shot_viewer_assets.py
python3 -m unittest tools.test_build_docs_search
npm --prefix web/shot-viewer run test:e2e -- --workers=3
```

The site checker passes all 8 checks, the search builder passes all 6 tests, and Chromium plus Firefox pass all 96 interaction tests. Browser tests include 390–1920px layouts, search destinations, keyboard navigation, installation routes, figure exports, and Shot Lab editing. The viewer JavaScript and CSS bundles are synchronized with the embedded assets; unchanged checked-in WASM is reused for this documentation build.
