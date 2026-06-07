#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

LISTEN="${CODEX_AUTH_PROXY_VIEWER_LISTEN:-127.0.0.1:8788}"
DB="${CODEX_AUTH_PROXY_DB:-./codex-auth-proxy.sqlite}"

cd "$WORKSPACE_DIR"

exec "${CARGO:-cargo}" run -p codex-auth-proxy -- viewer \
  --db "$DB" \
  --listen "$LISTEN" \
  "$@"
