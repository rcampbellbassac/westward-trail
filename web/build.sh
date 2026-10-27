#!/usr/bin/env sh
# Build the game for the browser and copy it next to index.html.
#   ./web/build.sh          then   python3 -m http.server -d web 8080
set -eu
cd "$(dirname "$0")/.."
cargo build --release --target wasm32-unknown-unknown -p trail-web
cp target/wasm32-unknown-unknown/release/trail_web.wasm web/trail.wasm
ls -l web/trail.wasm
