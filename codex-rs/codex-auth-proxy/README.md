# codex-auth-proxy

`codex-auth-proxy` is a narrow Responses API proxy for using the proxy host's
Codex/ChatGPT login auth as the upstream credential.

It accepts:

- `GET /health`
- `POST /v1/responses`
- `GET /v1/models`

Everything else is rejected with `403`.

## Run the proxy host

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

[profiles.local-proxy]
model_provider = "local-codex-auth-proxy"
```

Then run:

```shell
codex -p local-proxy
```

## Security notes

The remote client token only authorizes access to this proxy. The proxy removes
the incoming `Authorization` and `Host` headers before forwarding upstream, then
adds the proxy host's current Codex/ChatGPT auth headers.

Do not expose the proxy without a strong `--proxy-token-env` value and network
controls. Anyone who can call the proxy can spend the proxy host's Codex account
quota through the accepted endpoints.
