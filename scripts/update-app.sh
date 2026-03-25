#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
APP_PATH="$HOME/Applications/review-please.app"

cd "$ROOT_DIR"

git pull --ff-only
yarn install --frozen-lockfile
"$ROOT_DIR/scripts/install-app.sh"

osascript -e 'tell application "review-please" to quit' >/dev/null 2>&1 || true
open "$APP_PATH"

echo "updated and relaunched $APP_PATH"
