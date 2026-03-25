#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

cd "$ROOT_DIR"

yarn install --frozen-lockfile
yarn build
yarn tauri build --bundles app
