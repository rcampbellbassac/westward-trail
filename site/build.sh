#!/usr/bin/env sh
# Assemble the GitHub Pages site in dist/: the landing page with the game built in,
# plus the full-screen play page at play/.
#   ./site/build.sh          then   python3 -m http.server -d dist 8080
set -eu
cd "$(dirname "$0")/.."
./web/build.sh
rm -rf dist
mkdir -p dist/play
cp -r site/index.html site/shots dist/
cp web/main.js web/trail.wasm dist/
cp web/index.html web/main.js web/trail.wasm dist/play/
touch dist/.nojekyll
echo "Site ready in dist/"
