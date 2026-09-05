#!/usr/bin/env bash
set -euo pipefail

repo_root="${SHOT_VIEWER_REPO_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
repo_root="$(cd "$repo_root" && pwd)"
cd "$repo_root"

wasm_bindgen="${WASM_BINDGEN:-$(command -v wasm-bindgen || true)}"
if [[ -z "$wasm_bindgen" ]]; then
  echo "wasm-bindgen 0.2.126 is required; install it with: cargo install wasm-bindgen-cli --version 0.2.126 --locked" >&2
  exit 1
fi

wasm_bindgen_version="$("$wasm_bindgen" --version | awk '{print $2}')"
if [[ "$wasm_bindgen_version" != "0.2.126" ]]; then
  echo "wasm-bindgen 0.2.126 is required, found $wasm_bindgen_version" >&2
  exit 1
fi

wasm_cc="${SHOT_WASM_CC:-}"
if [[ -z "$wasm_cc" && -x /opt/homebrew/opt/llvm/bin/clang ]]; then
  wasm_cc=/opt/homebrew/opt/llvm/bin/clang
fi
if [[ -z "$wasm_cc" ]]; then
  wasm_cc="$(command -v clang || true)"
fi
if [[ -z "$wasm_cc" ]] || ! "$wasm_cc" --print-targets 2>/dev/null | grep -q wasm32; then
  echo "a clang build with the wasm32 target is required (or set SHOT_WASM_CC)" >&2
  exit 1
fi

rustc_path="${RUSTC:-$(rustup which rustc 2>/dev/null || command -v rustc)}"
cargo_path="${CARGO:-$(rustup which cargo 2>/dev/null || command -v cargo)}"
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
mkdir -p site/static/interactive/pkg rstim/assets/shot-viewer/pkg

RUSTC="$rustc_path" CC_wasm32_unknown_unknown="$wasm_cc" \
  "$cargo_path" build --locked -p rstim-shot-web --target wasm32-unknown-unknown --release
"$wasm_bindgen" "$target_dir/wasm32-unknown-unknown/release/rstim_shot_web.wasm" \
  --target web \
  --out-dir site/static/interactive/pkg \
  --out-name rstim_shot_web

npm --prefix web/shot-viewer ci
npm --prefix web/shot-viewer run build
perl -pi -e 's/[ \t]+$//' site/static/interactive/app.js

cp site/static/interactive/app.js rstim/assets/shot-viewer/app.js
cp site/static/interactive/shot-viewer.css rstim/assets/shot-viewer/shot-viewer.css
cp site/static/interactive/pkg/rstim_shot_web_bg.wasm \
  rstim/assets/shot-viewer/pkg/rstim_shot_web_bg.wasm
python3 tools/shot_viewer_assets.py --write
