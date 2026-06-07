# codex-auth-proxy

`codex-auth-proxy` is a narrow Responses API proxy for using the proxy host's
Codex/ChatGPT login auth as the upstream credential.

It accepts:

- `GET /health`
- `POST /v1/responses`
- `GET /v1/models`

Everything else is rejected with `403`.

## Run the proxy host

For copy/paste server startup steps, see
[`PROXY_SERVER_RUN.md`](./PROXY_SERVER_RUN.md).

First, log in on the machine that will run the proxy:

```shell
codex login
```

Then start the proxy with a bearer token that remote clients must send:

```shell
export CODEX_PROXY_TOKEN='change-this-long-random-value'

codex-auth-proxy \
  --listen 0.0.0.0:8787 \
  --proxy-token-env CODEX_PROXY_TOKEN
```

To also persist proxied request/response bodies to SQLite for local inspection:

```shell
codex-auth-proxy \
  --listen 0.0.0.0:8787 \
  --proxy-token-env CODEX_PROXY_TOKEN \
  --log-db ./codex-auth-proxy.sqlite
```

For local-only testing, omit `--listen` and the proxy will bind to an ephemeral
loopback port. Non-loopback listeners require `--proxy-token-env` unless
`--allow-unauthenticated` is explicitly set.

Check that a remote client can reach the proxy with the configured token:

```shell
curl --fail \
  -H "Authorization: Bearer ${CODEX_PROXY_TOKEN}" \
  http://PROXY_HOST:8787/health
```

## Configure the remote Codex client

For copy/paste setup steps and IP-change usage, see
[`REMOTE_CLIENT_SETUP.md`](./REMOTE_CLIENT_SETUP.md).

On the machine running Codex against its own files, set the same proxy token:

```shell
export CODEX_PROXY_TOKEN='change-this-long-random-value'
```

Add a provider to `~/.codex/config.toml`:

```toml
[model_providers.local-codex-auth-proxy]
name = "local-codex-auth-proxy"
base_url = "http://PROXY_HOST:8787/v1"
wire_api = "responses"
env_key = "CODEX_PROXY_TOKEN"
```

Create `~/.codex/local-proxy.config.toml`:

```toml
model_provider = "local-codex-auth-proxy"
model = "gpt-5.5"
model_reasoning_effort = "medium"
service_tier = "default"
```

Then run:

```shell
codex -p local-proxy
```

## Inspect logged traffic

When `--log-db` is set, each proxied request is stored in the `proxy_requests`
table. The row is inserted when the upstream request starts, then updated with
the raw upstream response body when the response finishes. Streaming responses
are stored as raw SSE text. If the upstream response includes token `usage`,
the parsed token counts are stored in dedicated columns.

When `--log-db` is set, the proxy keeps only the newest 1000 completed request
rows by default. Use `--log-retain-rows ROWS` to choose a different limit, or
`--log-retain-rows unlimited` to disable pruning explicitly. Pruning runs when
the proxy opens the database and after each request completes, so an existing
database with more rows is reduced on the next startup. In-progress rows are
not deleted. SQLite may not immediately shrink the `.sqlite` file on disk after
rows are deleted; run `VACUUM` manually if reclaiming disk space is necessary.

The proxy also stores at most 1 MiB of request body text and 1 MiB of response
body text per row by default. The `request_bytes` and `response_bytes` columns
keep the original upstream body sizes, while `request_body_truncated` and
`response_body_truncated` indicate whether the stored SQLite text was cut. Use
`--log-max-body-bytes BYTES` to choose a different per-body limit, or
`--log-max-body-bytes unlimited` to store full bodies explicitly. Upstream
traffic is never truncated; only the SQLite copy is limited.

The generated SQLite file can be opened directly in tools such as DBeaver.
For a browser UI, run the local-only viewer:

```shell
codex-auth-proxy viewer \
  --db ./codex-auth-proxy.sqlite \
  --listen 127.0.0.1:8788
```

The viewer opens on a summary page first. It separates request messages,
collapsible request JSON, extracted response text, response SSE events, and raw
SSE. Large request strings and SSE event payloads are rendered only when their
rows are expanded. The request list includes quick filters for errors, slow
requests, high token usage, and truncated log rows. The search box scans row
metadata plus stored request/response body text. The Summary view includes a
Growth Analysis panel for the largest request growth signals, and the Tool I/O
tab groups tool calls and tool outputs with the largest outputs ranked first.
The left request list stays as a single chronological list. When a row belongs
to the selected request's flow, the row shows its flow step number inline and
the related rows are lightly outlined. The Flow tab shows the same grouping in
detail. Flow grouping first uses tool call
`call_id` links when a response tool call is followed by a matching request tool
output. If no call chain is available, it falls back to nearby `/v1/responses`
rows with the same User asked text.

Example query:

```sql
SELECT
  id,
  started_at,
  client_ip,
  method,
  path,
  model,
  upstream_status,
  latency_ms,
  request_bytes,
  response_bytes,
  request_body_truncated,
  response_body_truncated,
  input_tokens,
  output_tokens,
  total_tokens,
  cached_input_tokens,
  reasoning_output_tokens,
  error
FROM proxy_requests
ORDER BY started_at DESC;
```

## Security notes

The remote client token only authorizes access to this proxy. The proxy removes
the incoming `Authorization` and `Host` headers before forwarding upstream, then
adds the proxy host's current Codex/ChatGPT auth headers.

Do not expose the proxy without a strong `--proxy-token-env` value and network
controls. Anyone who can call the proxy can spend the proxy host's Codex account
quota through the accepted endpoints.

The SQLite log database can contain prompts, file contents, shell output,
patches, and error logs from `/v1/responses`. Treat it as sensitive data.
