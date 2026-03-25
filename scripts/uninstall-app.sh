#!/bin/bash
set -euo pipefail

APP_PATH="$HOME/Applications/review-please.app"

"$(cd "$(dirname "$0")" && pwd)/disable-login.sh"
osascript -e 'tell application "review-please" to quit' >/dev/null 2>&1 || true
rm -rf "$APP_PATH"

echo "removed $APP_PATH"
