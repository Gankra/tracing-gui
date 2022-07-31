#!/bin/sh

cargo build --release --lib --target wasm32-unknown-unknown
wasm-bindgen target/wasm32-unknown-unknown/release/tracing_gui.wasm --out-dir docs --no-modules --no-typescript
wasm-opt "docs/tracing_gui_bg.wasm" -O2 --fast-math -o "docs/tracing_gui_bg.wasm" # add -g to get debug symbols
python3 -m http.server & open http://localhost:8000/docs/index.html