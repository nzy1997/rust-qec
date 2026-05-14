.PHONY: help test check release

DEFAULT_BRANCH ?= master

# Cross-platform sed in-place: macOS needs -i '', Linux needs -i
SED_I := sed -i$(shell if [ "$$(uname)" = "Darwin" ]; then echo " ''"; fi)

help:
	@echo "Available targets:"
	@echo "  test                 - Run workspace tests"
	@echo "  check                - Run cargo check for the workspace"
	@echo "  release V=x.y.z      - Bump crate versions, commit, tag, and push a release"

test:
	cargo test --workspace

check:
	cargo check --workspace

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
