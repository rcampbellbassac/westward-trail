#!/bin/bash
# Build the web game and copy it to your web server.
# Put the destination in web/deploy.env (not committed), for example:
#   DEPLOY_DEST=myserver:/srv/westward-trail/
#   DEPLOY_URL=https://trail.example.com/
set -euo pipefail
cd "$(dirname "$0")"
if [ ! -f deploy.env ]; then
    echo "Create web/deploy.env with DEPLOY_DEST (and optionally DEPLOY_URL) first." >&2
    exit 1
fi
. ./deploy.env
./build.sh
rsync -a --delete index.html main.js trail.wasm "$DEPLOY_DEST"
echo "Deployed. Open ${DEPLOY_URL:-your site}"
