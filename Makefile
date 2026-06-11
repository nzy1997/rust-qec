.PHONY: help test check build-site release bench-surface-smoke bench-surface-full surface-decoder-compare-smoke surface-decoder-compare-full

DEFAULT_BRANCH ?= master

# Cross-platform sed in-place: macOS needs -i '', Linux needs -i
SED_I := sed -i$(shell if [ "$$(uname)" = "Darwin" ]; then echo " ''"; fi)

help:
	@echo "Available targets:"
	@echo "  test                 - Run workspace tests"
	@echo "  check                - Run cargo check for the workspace"
	@echo "  build-site           - Build the QP101 GitHub Pages site into _site"
	@echo "  bench-surface-smoke  - Run the smoke surface decoder benchmark framework flow"
	@echo "  bench-surface-full   - Run the full surface decoder benchmark framework flow"
	@echo "  surface-decoder-compare-smoke - Run the smoke surface decoder comparison benchmark"
	@echo "  surface-decoder-compare-full  - Run the full surface decoder comparison benchmark"
	@echo "  release V=x.y.z      - Bump crate versions, commit, tag, and push a release"

test:
	cargo test --workspace

check:
	cargo check --workspace

build-site:
	rm -rf _site
	mkdir -p _site/examples _site/gallery
	cp site/index.html site/styles.css site/app.js _site/
	cp rstim/doc/qp101.schema.json _site/qp101.schema.json
	cp rstim/doc/QP101-ZY.md _site/QP101-ZY.md
	cp qp101-viz/examples/basic.qp101.json _site/examples/basic.qp101.json
	cp qp101-viz/examples/repeat-detector.qp101.json _site/examples/repeat-detector.qp101.json
	cp qp101-viz/examples/atom-loss-sample.qp101.json _site/examples/atom-loss-sample.qp101.json
	typst compile --format svg --root qp101-viz qp101-viz/examples/basic-site.typ _site/gallery/basic-site.svg
	typst compile --format svg --root qp101-viz qp101-viz/examples/repeat-detector-site.typ _site/gallery/repeat-detector-site.svg
	typst compile --format svg --root qp101-viz qp101-viz/examples/atom-loss-sample.typ _site/gallery/atom-loss-sample.svg

bench-surface-smoke:
	cargo run -p rsinter --bin rsinter -- bench run --spec benchmarks/surface_decoder/spec.toml --language rust --out benchmarks/out/surface_decoder/smoke-rust
	.venv-surface-decoder/bin/python -m benchmarks.python_runners.surface_decoder.run --spec benchmarks/surface_decoder/spec.toml --language python --out benchmarks/out/surface_decoder/smoke-python
	cargo run -p rsinter --bin rsinter -- bench merge --spec benchmarks/surface_decoder/spec.toml --input benchmarks/out/surface_decoder/smoke-rust/rmatching/test-run/results.jsonl --input benchmarks/out/surface_decoder/smoke-rust/rbposd/test-run/results.jsonl --input benchmarks/out/surface_decoder/smoke-rust/rilpqec/test-run/results.jsonl --input benchmarks/out/surface_decoder/smoke-python/pymatching/test-run/results.jsonl --input benchmarks/out/surface_decoder/smoke-python/ilpqec/test-run/results.jsonl --input benchmarks/out/surface_decoder/smoke-python/ldpc/test-run/results.jsonl --out benchmarks/out/surface_decoder/merged/smoke.jsonl
	cargo run -p rsinter --bin rsinter -- bench plot --spec benchmarks/surface_decoder/spec.toml --input benchmarks/out/surface_decoder/merged/smoke.jsonl --out benchmarks/out/surface_decoder/plots/smoke.svg

bench-surface-full:
	cargo run -p rsinter --bin rsinter -- bench run --spec benchmarks/surface_decoder/full.toml --language rust --out benchmarks/out/surface_decoder/full-rust
	.venv-surface-decoder/bin/python -m benchmarks.python_runners.surface_decoder.run --spec benchmarks/surface_decoder/full.toml --language python --out benchmarks/out/surface_decoder/full-python
	cargo run -p rsinter --bin rsinter -- bench merge --spec benchmarks/surface_decoder/full.toml --input benchmarks/out/surface_decoder/full-rust/rmatching/test-run/results.jsonl --input benchmarks/out/surface_decoder/full-rust/rbposd/test-run/results.jsonl --input benchmarks/out/surface_decoder/full-rust/rilpqec/test-run/results.jsonl --input benchmarks/out/surface_decoder/full-python/pymatching/test-run/results.jsonl --input benchmarks/out/surface_decoder/full-python/ilpqec/test-run/results.jsonl --input benchmarks/out/surface_decoder/full-python/ldpc/test-run/results.jsonl --out benchmarks/out/surface_decoder/merged/full.jsonl
	cargo run -p rsinter --bin rsinter -- bench plot --spec benchmarks/surface_decoder/full.toml --input benchmarks/out/surface_decoder/merged/full.jsonl --out benchmarks/out/surface_decoder/plots/full.svg

surface-decoder-compare-smoke:
	.venv-surface-decoder/bin/python -m benchmarks.surface_decoder_compare.run_compare --tier smoke
	.venv-surface-decoder/bin/python -m benchmarks.surface_decoder_compare.plot_compare --tier smoke

surface-decoder-compare-full:
	.venv-surface-decoder/bin/python -m benchmarks.surface_decoder_compare.run_compare --tier full
	.venv-surface-decoder/bin/python -m benchmarks.surface_decoder_compare.plot_compare --tier full

# Release a new version: make release V=0.2.0
release:
ifeq ($(strip $(V)),)
	$(error Usage: make release V=x.y.z)
endif
	$(SED_I) 's/^version = ".*"/version = "$(V)"/' rstim/Cargo.toml
	$(SED_I) 's/^version = ".*"/version = "$(V)"/' rsinter/Cargo.toml
	$(SED_I) 's/^version = ".*"/version = "$(V)"/' rbposd/Cargo.toml
	$(SED_I) 's/^version = ".*"/version = "$(V)"/' rmatching/Cargo.toml
	cargo check --workspace
	git add rstim/Cargo.toml rsinter/Cargo.toml rbposd/Cargo.toml rmatching/Cargo.toml
	git commit -m "release: v$(V)"
	git tag -a "v$(V)" -m "Release v$(V)"
	git push origin $(DEFAULT_BRANCH) --tags
	@echo "v$(V) pushed - GitHub Actions will create a GitHub Release"
