+++
title = "Versions and migration"
template = "versions.html"
[extra]
root = ".."
nav = "versions"
+++
## Select a documentation version

`dev` follows the development branch. A numbered edition is the frozen documentation for that coordinated software release. The version menu lists published editions; development pages may describe behavior that is newer than the latest stable archive. Keep the edition, binary version and dataset provenance together when reproducing results.

A RustQEC tag identifies a coordinated source release. Individual crates can receive independent patch updates within the same major/minor line. The [API index](../rust-api-reference/) lists the versions built from this checkout; `rstim --version` and `rstim capabilities --format json` describe your installed binary.

## Upgrade deliberately

For an installed CLI, record capabilities before upgrading, retain original input files, then rerun a small known case. For Rust, update the intended dependency versions and commit Cargo.lock in your application. A native decoder feature is a build choice, not a new dataset schema or an expanded support promise.

The migration notes below come from the same canonical support contract, so code examples and compatibility rules have one maintained source. Historical release details are in [GitHub releases](https://github.com/nzy1997/rust-qec/releases).
