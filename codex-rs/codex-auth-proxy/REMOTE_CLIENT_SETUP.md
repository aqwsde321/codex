# 외부 Codex 클라이언트 설정

이 문서는 외부 컴퓨터의 Codex가 `codex-auth-proxy`를 모델 provider로
사용하도록 설정하는 절차입니다.

외부 컴퓨터는 자기 파일과 shell/test 실행을 직접 처리하고, 모델 응답만
프록시 서버를 통해 받습니다.

## 준비물

- 프록시 서버 시작 로그의 `client_base_url`
- 프록시 서버에서 설정한 `CODEX_PROXY_TOKEN`
- 외부 컴퓨터에 Codex가 설치되어 있고 `~/.codex` 디렉터리가 있는 상태

프록시 서버가 켜질 때 아래처럼 표시되는 값을 사용합니다.

```text
client_base_url: http://<프록시_IP>:8787/v1
```

`~/.codex`가 있는지 먼저 확인합니다.

```shell
ls -la ~/.codex
```

없다면 외부 컴퓨터에서 Codex를 한 번 실행하거나 로그인해서 기본 설정
디렉터리를 먼저 만들고 진행합니다.

## 초기 1회 설정

### 1. provider 추가

`~/.codex/config.toml` 맨 아래에 provider를 추가합니다.

```shell
cat >> ~/.codex/config.toml <<'EOF'

[model_providers.local-auth-proxy]
name = "local-auth-proxy"
base_url = "http://127.0.0.1:8787/v1"
wire_api = "responses"
env_key = "CODEX_PROXY_TOKEN"
EOF
```

`base_url`의 `127.0.0.1`은 기본값입니다. 실제 프록시 서버 IP는 실행할
때 `-c` 옵션으로 덮어씁니다.

주의: 이 명령은 한 번만 실행합니다. 여러 번 실행하면 같은 provider
블록이 중복으로 추가됩니다. 이미 추가되어 있다면 다시 붙여넣지 말고
기존 블록을 수정합니다.

### 2. profile 생성

```shell
cat > ~/.codex/local-proxy.config.toml <<'EOF'
model_provider = "local-auth-proxy"
model = "gpt-5.5"
model_reasoning_effort = "medium"
service_tier = "default"
EOF
```

`model_reasoning_effort`는 사고력 설정입니다. 기본 사고력으로 쓰려면
`medium`을 명시합니다. 가능한 값은 `none`, `minimal`, `low`, `medium`,
`high`, `xhigh`입니다.

`service_tier = "default"`는 일반 기본 라우팅 요청입니다. 빠른 라우팅을
쓰려면 `service_tier = "priority"`로 바꿉니다.

### 3. 실행

아래 명령에서 `test`는 실제 토큰으로, `CODEX_AUTH_PROXY_BASE_URL`은 프록시
서버 시작 로그의 `client_base_url` 값으로 바꿔서 실행합니다.

```shell
export CODEX_PROXY_TOKEN=test
export CODEX_AUTH_PROXY_BASE_URL='http://<프록시_IP>:8787/v1'

codex -p local-proxy \
  -c "model_providers.local-auth-proxy.base_url=\"$CODEX_AUTH_PROXY_BASE_URL\""
```

## IP가 바뀌었을 때

config 파일을 다시 수정하지 말고, 실행 명령의 IP만 새 IP로 바꿉니다.
프록시 서버 시작 로그에 새로 찍힌 `client_base_url`을 그대로 사용하면 됩니다.

```shell
export CODEX_PROXY_TOKEN=test
export CODEX_AUTH_PROXY_BASE_URL='새_client_base_url'

codex -p local-proxy \
  -c "model_providers.local-auth-proxy.base_url=\"$CODEX_AUTH_PROXY_BASE_URL\""
```

## 헬스체크

Codex 실행 전에 프록시 서버 접근이 되는지 확인할 수 있습니다.

```shell
curl -i \
  -H "Authorization: Bearer test" \
  http://<프록시_IP>:8787/health
```

정상이라면 `200 OK`와 `{"status":"ok"}` 응답이 나옵니다.

## 고정 IP로 사용할 때

프록시 서버 IP가 고정되어 있다면 `~/.codex/config.toml`의 `base_url`에
실제 IP를 넣고 실행 명령을 짧게 사용할 수 있습니다.

```toml
base_url = "http://<고정_프록시_IP>:8787/v1"
```

그 경우 실행은 아래처럼 하면 됩니다.

```shell
CODEX_PROXY_TOKEN=test codex -p local-proxy
```

## 주의사항

- `CODEX_PROXY_TOKEN`은 OpenAI API key가 아니라 프록시 접근용 토큰입니다.
- 실제 모델 사용량은 프록시 서버 컴퓨터의 Codex/ChatGPT 로그인 계정에서
  사용됩니다.
- 프록시 서버가 꺼져 있으면 `local-proxy` profile은 동작하지 않습니다.
- `test` 토큰은 테스트용입니다. 실제 사용 시 긴 랜덤값으로 바꿉니다.
