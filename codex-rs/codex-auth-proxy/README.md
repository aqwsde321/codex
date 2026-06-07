# codex-auth-proxy

`codex-auth-proxy`는 프록시 서버 컴퓨터의 Codex/ChatGPT 로그인 auth를
사용해서 외부 Codex 클라이언트에 Responses API 호환 엔드포인트를 제공하는
작은 프록시입니다.

허용 엔드포인트:

- `GET /health`
- `POST /v1/responses`
- `GET /v1/models`

나머지 요청은 `403`으로 거부합니다.

## 서버 실행

자세한 실행 절차는 [PROXY_SERVER_RUN.md](./PROXY_SERVER_RUN.md)를 봅니다.

프록시 서버 컴퓨터에서 먼저 로그인합니다.

```shell
codex login
```

`codex-rs/codex-auth-proxy` 디렉터리에서 실행합니다.

```shell
export CODEX_PROXY_TOKEN='긴_랜덤_토큰'

./scripts/run-proxy.sh
```

기본값:

- listen: `0.0.0.0:8787`
- DB: `./codex-auth-proxy.sqlite`
- retention: 최신 완료 요청 `1000`개
- body limit: request/response body 각각 `1 MiB`

시작 로그에 외부 Codex에 넘길 `client_base_url`이 표시됩니다.

```text
client_base_url: http://<프록시_IP>:8787/v1
```

## 외부 Codex 설정

외부 컴퓨터 설정 절차는 [REMOTE_CLIENT_SETUP.md](./REMOTE_CLIENT_SETUP.md)를
봅니다.

핵심은 외부 Codex에서 같은 `CODEX_PROXY_TOKEN`을 사용하고, provider
`base_url`을 서버 로그의 `client_base_url`로 맞추는 것입니다.

```shell
export CODEX_PROXY_TOKEN='긴_랜덤_토큰'
export CODEX_AUTH_PROXY_BASE_URL='서버_로그의_client_base_url'

codex -p local-proxy \
  -c "model_providers.local-auth-proxy.base_url=\"$CODEX_AUTH_PROXY_BASE_URL\""
```

외부 컴퓨터의 파일 작업, shell 실행, 테스트 실행은 외부 컴퓨터에서 직접
일어나고, 모델 응답만 이 프록시를 통해 받습니다.

## 로그와 뷰어

`./scripts/run-proxy.sh`는 기본으로 SQLite 로그를 켭니다. 요청/응답 body는
`proxy_requests` 테이블에 저장되고, token `usage`가 있으면 토큰 컬럼도
채워집니다.

브라우저 뷰어:

```shell
./scripts/run-viewer.sh
```

기본 주소:

```text
http://127.0.0.1:8788
```

뷰어에서는 DB 크기와 row 통계, 검색, 필터, summary, flow 표시, tool I/O,
raw request/response를 확인할 수 있습니다. viewer는 민감한 로그를
보여주므로 loopback 주소에서만 실행됩니다.

## 보안 주의

- `CODEX_PROXY_TOKEN`은 OpenAI API key가 아니라 프록시 접근용 토큰입니다.
- 이 프록시를 호출할 수 있는 사용자는 프록시 서버 컴퓨터의 Codex 계정 사용량을
  소비할 수 있습니다.
- `CODEX_PROXY_TOKEN=test`는 테스트용입니다. 실제 사용 시 긴 랜덤값을 씁니다.
- SQLite DB에는 프롬프트, 코드, shell 출력, 패치 내용이 들어갈 수 있으므로
  민감 데이터로 취급합니다.
