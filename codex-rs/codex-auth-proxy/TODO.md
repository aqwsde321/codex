# codex-auth-proxy TODO

## SQLite request/response logging follow-ups

The baseline `--log-db` support stores one row per proxied request, including
raw request body and raw upstream response body. Streaming responses are stored
as raw SSE text after the response finishes. Token usage is parsed from upstream
SSE response events when usage fields are present.

When `--log-db` is set, old completed rows are pruned on startup and after each
request completion. The default is 1000 completed rows. Use
`--log-retain-rows ROWS` to choose a different limit or
`--log-retain-rows unlimited` to disable pruning explicitly.

Current schema:

```sql
CREATE TABLE proxy_requests (
  id TEXT PRIMARY KEY,
  started_at TEXT NOT NULL,
  completed_at TEXT,
  client_ip TEXT,
  method TEXT NOT NULL,
  path TEXT NOT NULL,
  query TEXT,
  model TEXT,
  upstream_status INTEGER,
  latency_ms INTEGER,
  request_bytes INTEGER,
  response_bytes INTEGER,
  input_tokens INTEGER,
  output_tokens INTEGER,
  total_tokens INTEGER,
  cached_input_tokens INTEGER,
  reasoning_output_tokens INTEGER,
  request_body TEXT,
  response_body TEXT,
  error TEXT
);
```

Future improvements:

- Add a max body size option for request/response storage.
- Add optional redaction for known sensitive fields in JSON request bodies.
- Add a manual smoke test note for opening the generated `.sqlite` file in
  DBeaver.

Security note:

Raw `/v1/responses` bodies can include prompts, code, file contents, shell
output, patches, and error logs. Treat the SQLite file as sensitive data.
