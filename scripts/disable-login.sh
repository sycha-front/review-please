#!/bin/bash
set -euo pipefail

PLIST_PATH="$HOME/Library/LaunchAgents/com.review-please.app.plist"
LABEL="com.review-please.app"

launchctl bootout "gui/$(id -u)/$LABEL" "$PLIST_PATH" >/dev/null 2>&1 || true
rm -f "$PLIST_PATH"

echo "disabled login launch"
