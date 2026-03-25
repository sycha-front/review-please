#!/bin/bash
set -euo pipefail

APP_PATH="$HOME/Applications/review-please.app"
PLIST_DIR="$HOME/Library/LaunchAgents"
PLIST_PATH="$PLIST_DIR/com.review-please.app.plist"
LABEL="com.review-please.app"

if [ ! -d "$APP_PATH" ]; then
  echo "app not found at $APP_PATH"
  echo "run ./scripts/install-app.sh first"
  exit 1
fi

mkdir -p "$PLIST_DIR"

cat > "$PLIST_PATH" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
  <dict>
    <key>Label</key>
    <string>$LABEL</string>
    <key>ProgramArguments</key>
    <array>
      <string>/usr/bin/open</string>
      <string>$APP_PATH</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
  </dict>
</plist>
PLIST

launchctl bootout "gui/$(id -u)/$LABEL" "$PLIST_PATH" >/dev/null 2>&1 || true
launchctl bootstrap "gui/$(id -u)" "$PLIST_PATH"

echo "enabled login launch: $PLIST_PATH"
