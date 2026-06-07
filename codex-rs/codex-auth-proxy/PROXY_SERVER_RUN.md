# 프록시 서버 실행

프록시 서버 컴퓨터에서 `codex-auth-proxy`를 켜고, 외부 Codex에 전달할 값을
확인하는 문서입니다.

## 1. 준비

프록시 서버 컴퓨터에서 Codex/ChatGPT 로그인 auth가 있어야 합니다.

```shell
codex login
```

로그인 상태 확인:

```shell
codex
```

## 2. 프록시 실행

`codex-rs/codex-auth-proxy` 디렉터리에서 실행합니다.

```shell
export CODEX_PROXY_TOKEN='긴_랜덤_토큰'

./scripts/run-proxy.sh
```

기본값:

| 항목 | 값 |
| --- | --- |
| listen | `0.0.0.0:8787` |
| DB | `./codex-auth-proxy.sqlite` |
| retention | 최신 완료 요청 `1000`개 |
| body limit | request/response body 각각 `1048576` bytes |

값을 바꿀 때만 환경변수를 추가합니다.

```shell
CODEX_AUTH_PROXY_LISTEN=0.0.0.0:8787 \
CODEX_AUTH_PROXY_DB=./codex-auth-proxy.sqlite \
CODEX_AUTH_PROXY_RETAIN_ROWS=3000 \
CODEX_AUTH_PROXY_MAX_BODY_BYTES=2097152 \
./scripts/run-proxy.sh
```

## 3. 외부 Codex에 전달할 값

서버가 켜지면 아래처럼 시작 로그가 나옵니다.

```text
codex-auth-proxy listening on 0.0.0.0:8787
  local_ip: <프록시_IP>
  health: http://<프록시_IP>:8787/health
  client_base_url: http://<프록시_IP>:8787/v1
  proxy_auth: bearer token from $CODEX_PROXY_TOKEN
  log_db: ./codex-auth-proxy.sqlite
  retention: 1000 completed rows
  body_limit: 1048576 bytes per request/response body
```

외부 Codex에 넘길 값은 두 개입니다.

- `CODEX_PROXY_TOKEN`
- `client_base_url`

외부 컴퓨터 설정은 [REMOTE_CLIENT_SETUP.md](./REMOTE_CLIENT_SETUP.md)를
사용합니다.

## 4. 헬스체크

프록시 서버 컴퓨터:

```shell
curl -i \
  -H "Authorization: Bearer ${CODEX_PROXY_TOKEN}" \
  http://127.0.0.1:8787/health
```

외부 컴퓨터:

```shell
curl -i \
  -H "Authorization: Bearer ${CODEX_PROXY_TOKEN}" \
  http://<프록시_IP>:8787/health
```

정상이면 `200 OK`와 `{"status":"ok"}`가 나옵니다.

## 5. 뷰어 실행

별도 터미널에서 실행합니다.

```shell
./scripts/run-viewer.sh
```

브라우저 주소:

```text
http://127.0.0.1:8788
```

상단에는 DB 크기, 전체 row 수, 완료/진행 중 row 수, 마지막 요청 시간이
표시됩니다.

다른 DB나 포트를 쓰는 경우:

```shell
CODEX_AUTH_PROXY_DB=./codex-auth-proxy.sqlite \
CODEX_AUTH_PROXY_VIEWER_LISTEN=127.0.0.1:8790 \
./scripts/run-viewer.sh
```

viewer는 SQLite 로그 내용을 보여주므로 외부 공개 주소로 열지 않습니다.

## 6. 종료

프록시나 viewer가 실행 중인 터미널에서 `Ctrl-C`를 누릅니다.

서버가 꺼져 있으면 외부 컴퓨터의 `codex -p local-proxy`는 동작하지 않습니다.

## 주의사항

- `CODEX_PROXY_TOKEN=test`는 테스트용입니다. 실제 사용 시 긴 랜덤값으로
  바꿉니다.
- `0.0.0.0:8787`은 같은 네트워크의 다른 컴퓨터가 접속할 수 있게 엽니다.
- 프록시를 호출할 수 있는 사용자는 프록시 서버 컴퓨터의 Codex 계정 사용량을
  소비할 수 있습니다.
- SQLite DB에는 프롬프트, 코드, shell 출력, 패치 내용이 들어갈 수 있으므로
  민감 데이터로 취급합니다.
