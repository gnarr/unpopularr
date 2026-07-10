#!/usr/bin/env bash
# Starts the mock Sonarr/Tautulli (port 39100) and the app (port 39101) for
# the Playwright suite. The app runs inside the project's Rust build image
# because the toolchain is not installed natively; cached cargo volumes keep
# warm starts fast.
set -euo pipefail
cd "$(dirname "$0")/.."

# The backend serves the SPA from web/dist, which is not committed (only its
# .gitkeep is). Build it every run so the tests always exercise the current
# sources; warm rebuilds take about a second.
[ -d web/node_modules ] || npm ci --prefix web
npm run build --prefix web

node e2e/mock-arr.mjs &
MOCK_PID=$!

cleanup() {
  docker stop unpopularr-e2e >/dev/null 2>&1 || true
  kill "$MOCK_PID" >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM

docker rm -f unpopularr-e2e >/dev/null 2>&1 || true
docker run --rm --name unpopularr-e2e --network host \
  -e CARGO_TERM_COLOR=never \
  -e UNPOPULARR_CONFIG=/app/e2e/config.toml \
  -e SONARR_HD_API_KEY=secret \
  -e RADARR_UHD_API_KEY=secret \
  -e LIDARR_MAIN_API_KEY=secret \
  -e TAUTULLI_API_KEY=secret \
  -v "$PWD":/app -w /app \
  -v unpopularr-cargo-registry:/usr/local/cargo/registry \
  -v unpopularr-cargo-git:/usr/local/cargo/git \
  -v unpopularr-target:/app/target \
  rust:1.94-slim-bookworm cargo run --locked
