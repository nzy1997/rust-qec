# Decode and Navigation Refinement

## Goal

Reduce duplication in the documentation site, make decoder benchmark plots readable without opening them separately, and give every page a clear location indicator in the top navigation.

## Information architecture

- Merge all content from **Run benchmark campaigns** into **Decode with three decoder families**.
- Remove the benchmark campaign card from the home page and remove **Bench** from the top navigation.
- Delete the `/benchmark-campaigns/` route, content entry, and template. No redirect is required because the site does not link to that route after this change.
- Keep benchmark evidence beside the decoder workflow on `/decoding/`.

The merged Decode page will read in this order:

1. Short decoder-family comparison.
2. Short smoke/full campaign commands and campaign summary.
3. Two checked visual comparisons, each occupying its own full-width row.
4. Local smoke/readiness evidence.

## Home page

- Remove the **Explore the workspace** and **RSMP v1 showcase** hero buttons.
- Keep **WHAT'S IN THE BOX** as the only home-page navigation module.
- Keep one card for Decode, with copy that covers both decoder selection and benchmark campaigns.

## Result presentation

- Decoder evidence cards that contain images use a single-column layout.
- The plot is shown at the full content width with no two-column text/plot split.
- Claims limits and reproduction details remain below the plot or inside the existing expandable details so the visual result stays primary.
- Non-image evidence cards remain compact.

## Active navigation

- Each content page supplies its own current navigation key to the shared base template.
- The matching top-navigation link receives `aria-current="page"` and an active class.
- The current item uses the accent color and a visible underline; hover and keyboard-focus behavior remain intact.
- Home, Simulate, DEMs, Decode, CSS Codes, RSMP v1, and QP101 each have one active state.

## Validation

- Site contract tests verify that the Bench navigation item, home campaign card, hero buttons, and benchmark campaign route are absent.
- Tests verify that decoder and campaign anchors/evidence are present on the Decode page.
- Tests verify that each page supplies an active navigation state and that active styling exists.
- The static site build checker validates the reduced route list, benchmark evidence assignments, and local references.
- Rebuild `_site`, run the existing Rust and Python site-contract suites, and refresh the local preview.
