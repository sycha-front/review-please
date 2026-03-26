#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
ENV_FILE="$ROOT_DIR/.env.release"
DEFAULT_REPO="sycha-front/pr-review-please"
TEMP_CONFIG=""

cleanup() {
  if [[ -n "$TEMP_CONFIG" && -f "$TEMP_CONFIG" ]]; then
    rm -f "$TEMP_CONFIG"
  fi
}
trap cleanup EXIT

fail() {
  echo "error: $*" >&2
  exit 1
}

if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
fi

if [[ -z "${TAURI_UPDATER_PUBLIC_KEY:-}" && -n "${TAURI_UPDATER_PUBLIC_KEY_FILE:-}" ]]; then
  TAURI_UPDATER_PUBLIC_KEY="$(cat "$TAURI_UPDATER_PUBLIC_KEY_FILE")"
  export TAURI_UPDATER_PUBLIC_KEY
fi

[[ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ]] || fail "TAURI_SIGNING_PRIVATE_KEY is required"
[[ -n "${TAURI_UPDATER_PUBLIC_KEY:-}" ]] || fail "TAURI_UPDATER_PUBLIC_KEY or TAURI_UPDATER_PUBLIC_KEY_FILE is required"

GITHUB_REPOSITORY="${GITHUB_REPOSITORY:-$DEFAULT_REPO}"
export GITHUB_REPOSITORY
export TAURI_UPDATER_ENDPOINT="${TAURI_UPDATER_ENDPOINT:-https://github.com/$GITHUB_REPOSITORY/releases/latest/download/latest.json}"

CARGO_VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$ROOT_DIR/src-tauri/Cargo.toml" | head -n 1)"
TAURI_VERSION="$(sed -n 's/.*"version": "\(.*\)".*/\1/p' "$ROOT_DIR/src-tauri/tauri.conf.json" | head -n 1)"

[[ -n "$CARGO_VERSION" ]] || fail "failed to read version from src-tauri/Cargo.toml"
[[ -n "$TAURI_VERSION" ]] || fail "failed to read version from src-tauri/tauri.conf.json"
[[ "$CARGO_VERSION" == "$TAURI_VERSION" ]] || fail "src-tauri/Cargo.toml version ($CARGO_VERSION) and src-tauri/tauri.conf.json version ($TAURI_VERSION) must match"

VERSION="$CARGO_VERSION"
RELEASE_TAG="${RELEASE_TAG:-v$VERSION}"
PUBLISHED_AT="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

case "$(uname -m)" in
  arm64|aarch64)
    PLATFORM_KEY="darwin-aarch64-app"
    ;;
  x86_64)
    PLATFORM_KEY="darwin-x86_64-app"
    ;;
  *)
    fail "unsupported macOS architecture: $(uname -m)"
    ;;
esac

OUTPUT_DIR="$ROOT_DIR/release/$RELEASE_TAG"
ARCHIVE_PATH="$ROOT_DIR/src-tauri/target/release/bundle/macos/review-please.app.tar.gz"
SIGNATURE_PATH="${ARCHIVE_PATH}.sig"
DMG_PATH="$(find "$ROOT_DIR/src-tauri/target/release/bundle/dmg" -maxdepth 1 -type f -name '*.dmg' 2>/dev/null | head -n 1 || true)"
TEMP_CONFIG="$(mktemp /tmp/review-please-tauri-release.XXXXXX.json)"

python3 - "$TEMP_CONFIG" "$TAURI_UPDATER_PUBLIC_KEY" "$TAURI_UPDATER_ENDPOINT" <<'PY'
import json
import sys

path, pubkey, endpoint = sys.argv[1:4]

with open(path, "w", encoding="utf-8") as f:
    json.dump(
        {
            "bundle": {
                "createUpdaterArtifacts": True,
            },
            "plugins": {
                "updater": {
                    "pubkey": pubkey,
                    "endpoints": [endpoint],
                }
            },
        },
        f,
        ensure_ascii=False,
    )
PY

cd "$ROOT_DIR"

yarn install --frozen-lockfile
yarn build
yarn tauri build --config "$TEMP_CONFIG"

[[ -f "$ARCHIVE_PATH" ]] || fail "missing updater archive: $ARCHIVE_PATH"
[[ -f "$SIGNATURE_PATH" ]] || fail "missing updater signature: $SIGNATURE_PATH"

rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"

cp "$ARCHIVE_PATH" "$OUTPUT_DIR/"
cp "$SIGNATURE_PATH" "$OUTPUT_DIR/"

if [[ -n "$DMG_PATH" ]]; then
  cp "$DMG_PATH" "$OUTPUT_DIR/"
fi

ARCHIVE_NAME="$(basename "$ARCHIVE_PATH")"
SIGNATURE_VALUE="$(tr -d '\n' < "$SIGNATURE_PATH")"

cat > "$OUTPUT_DIR/latest.json" <<EOF
{
  "version": "$VERSION",
  "notes": null,
  "pub_date": "$PUBLISHED_AT",
  "release_url": "https://github.com/$GITHUB_REPOSITORY/releases/tag/$RELEASE_TAG",
  "platforms": {
    "$PLATFORM_KEY": {
      "url": "https://github.com/$GITHUB_REPOSITORY/releases/download/$RELEASE_TAG/$ARCHIVE_NAME",
      "signature": "$SIGNATURE_VALUE"
    }
  }
}
EOF

echo
echo "Prepared release files in: $OUTPUT_DIR"
echo "Upload these assets to GitHub Release $RELEASE_TAG:"
echo "  - latest.json"
echo "  - $(basename "$ARCHIVE_PATH")"
echo "  - $(basename "$SIGNATURE_PATH")"
if [[ -n "$DMG_PATH" ]]; then
  echo "  - $(basename "$DMG_PATH")"
fi
