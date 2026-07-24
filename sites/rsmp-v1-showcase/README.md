# RSMP v1 Showcase Site

This Sites project publishes the RSMP v1 product showcase: introduction,
real CLI examples, measured compression evidence, and large-scale projections.

## Prerequisites

- Node.js `>=22.13.0`

## Local Commands

```bash
npm install
npm run dev
npm run build
npm test
```

`npm test` builds the vinext worker, server-renders the page, and verifies the
shared evidence/projection module.

## Evidence Boundary

All measured byte counts and projection formulas live in
`lib/rsmpEvidence.mjs`. The site presents pinned evidence only; projected
large-scale storage and runtime are linear estimates from the 1024-shot
surface-code case, not large-run measurements.
