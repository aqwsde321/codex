#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

LISTEN="${CODEX_AUTH_PROXY_LISTEN:-0.0.0.0:8787}"
DB="${CODEX_AUTH_PROXY_DB:-./codex-auth-proxy.sqlite}"
TOKEN_ENV="${CODEX_AUTH_PROXY_TOKEN_ENV:-CODEX_PROXY_TOKEN}"
RETAIN_ROWS="${CODEX_AUTH_PROXY_RETAIN_ROWS:-1000}"
MAX_BODY_BYTES="${CODEX_AUTH_PROXY_MAX_BODY_BYTES:-1048576}"

if [[ -z "${!TOKEN_ENV:-}" ]]; then
  echo "Missing proxy token. Set $TOKEN_ENV before running this script." >&2
  echo "Example: export $TOKEN_ENV='change-this-long-random-value'" >&2
  exit 2
fi

cd "$WORKSPACE_DIR"

exec "${CARGO:-cargo}" run -p codex-auth-proxy -- \
  --listen "$LISTEN" \
  --proxy-token-env "$TOKEN_ENV" \
  --log-db "$DB" \
  --log-retain-rows "$RETAIN_ROWS" \
  --log-max-body-bytes "$MAX_BODY_BYTES" \
  "$@"
