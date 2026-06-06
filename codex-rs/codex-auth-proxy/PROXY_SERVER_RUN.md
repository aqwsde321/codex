# 프록시 서버 실행

이 문서는 프록시 서버 컴퓨터에서 `codex-auth-proxy`를 실행하는 절차입니다.

## 준비

프록시 서버 컴퓨터에서 Codex/ChatGPT 로그인 auth가 있어야 합니다.

```shell
codex login
```

현재 로그인 상태로 모델 호출이 가능한지도 한 번 확인합니다.

```shell
codex
```

## 현재 IP 확인

외부 컴퓨터가 접속할 프록시 서버 IP를 확인합니다.

macOS:

```shell
ipconfig getifaddr en0
```

Wi-Fi가 아니라 유선 LAN을 쓰는 경우 `en0` 대신 다른 인터페이스일 수
있습니다.

대안:

```shell
ifconfig | grep 'inet ' | grep -v '127.0.0.1'
```

## 서버 실행

repo의 `codex-rs` 디렉터리에서 실행합니다.

```shell
export CODEX_PROXY_TOKEN=test

cargo run -p codex-auth-proxy -- \
  --listen 0.0.0.0:8787 \
  --proxy-token-env CODEX_PROXY_TOKEN \
  --log-db ./codex-auth-proxy.sqlite
```

정상 실행되면 터미널에 아래와 비슷하게 표시됩니다.

```text
codex-auth-proxy listening on 0.0.0.0:8787
```

`--log-db`를 사용하면 기본으로 SQLite에 최신 완료 요청 1000개만 남깁니다.
이미 DB에 3000개가 쌓여 있어도 다시 실행하면 서버 시작 시 오래된 완료 row가
삭제됩니다. 이후 요청이 완료될 때마다 같은 정리가 다시 실행됩니다. 진행 중인
요청 row는 삭제하지 않습니다.

또한 기본으로 request body와 response body는 각각 최대 1 MiB만 SQLite에
저장합니다. upstream으로 보내고 받는 실제 요청/응답은 자르지 않고, DB에
저장하는 사본만 제한합니다. 원본 크기는 `request_bytes`, `response_bytes`에
남고, 잘렸는지는 `request_body_truncated`, `response_body_truncated`로
확인합니다.

다른 개수를 유지하려면 `--log-retain-rows`를 추가합니다.

```shell
cargo run -p codex-auth-proxy -- \
  --listen 0.0.0.0:8787 \
  --proxy-token-env CODEX_PROXY_TOKEN \
  --log-db ./codex-auth-proxy.sqlite \
  --log-retain-rows 3000
```

무제한으로 계속 저장하려면 명시적으로 `unlimited`를 사용합니다.

```shell
cargo run -p codex-auth-proxy -- \
  --listen 0.0.0.0:8787 \
  --proxy-token-env CODEX_PROXY_TOKEN \
  --log-db ./codex-auth-proxy.sqlite \
  --log-retain-rows unlimited
```

body 저장 크기 제한을 바꾸려면 `--log-max-body-bytes`를 추가합니다.

```shell
cargo run -p codex-auth-proxy -- \
  --listen 0.0.0.0:8787 \
  --proxy-token-env CODEX_PROXY_TOKEN \
  --log-db ./codex-auth-proxy.sqlite \
  --log-max-body-bytes 2097152
```

body도 무제한 저장하려면 명시적으로 `unlimited`를 사용합니다.

```shell
cargo run -p codex-auth-proxy -- \
  --listen 0.0.0.0:8787 \
  --proxy-token-env CODEX_PROXY_TOKEN \
  --log-db ./codex-auth-proxy.sqlite \
  --log-max-body-bytes unlimited
```

`--log-db`를 빼면 SQLite 요청/응답 저장 없이 프록시만 실행합니다.

```shell
export CODEX_PROXY_TOKEN=test

cargo run -p codex-auth-proxy -- \
  --listen 0.0.0.0:8787 \
  --proxy-token-env CODEX_PROXY_TOKEN
```

## 헬스체크

프록시 서버 컴퓨터에서 먼저 확인합니다.

```shell
curl -i \
  -H "Authorization: Bearer test" \
  http://127.0.0.1:8787/health
```

외부 컴퓨터에서는 `127.0.0.1` 대신 프록시 서버 IP를 사용합니다.

```shell
curl -i \
  -H "Authorization: Bearer test" \
  http://192.168.0.94:8787/health
```

정상이라면 `200 OK`와 `{"status":"ok"}` 응답이 나옵니다.

## 로그 확인

서버 터미널에는 요청 수신/완료 로그가 출력됩니다.

```text
request received id=... client_ip=... method=POST path=/v1/responses ...
request completed id=... status=200 response_bytes=... latency_ms=... error=-
```

`--log-db ./codex-auth-proxy.sqlite`를 사용하면 요청/응답 body가 SQLite에
저장됩니다. DBeaver에서 `codex-auth-proxy.sqlite` 파일을 열어
`proxy_requests` 테이블을 조회할 수 있습니다. upstream 응답에 token
`usage`가 포함되면 `input_tokens`, `output_tokens`, `total_tokens`,
`cached_input_tokens`, `reasoning_output_tokens` 컬럼도 채워집니다.
body 저장이 잘린 row는 `request_body_truncated`,
`response_body_truncated` 컬럼이 `true`로 표시됩니다.

row 삭제 후에도 SQLite 파일 크기는 바로 줄지 않을 수 있습니다. 실제 디스크
공간 회수가 필요하면 서버와 viewer를 끈 뒤 SQLite 도구에서 `VACUUM`을
실행합니다.

## 로컬 HTML 뷰어 실행

SQLite에 저장된 요청/응답을 브라우저에서 보려면 별도 터미널에서 viewer를
실행합니다.

```shell
cargo run -p codex-auth-proxy -- viewer \
  --db ./codex-auth-proxy.sqlite \
  --listen 127.0.0.1:8788
```

브라우저에서 아래 주소를 엽니다.

```text
http://127.0.0.1:8788
```

viewer에서는 먼저 요약 화면이 보입니다. 요청은 message 목록과 JSON tree로
나눠 볼 수 있고, 응답은 추출된 텍스트, SSE event별 접힘 뷰, raw SSE로
나눠 볼 수 있습니다. token usage가 기록된 row는 Summary에서 토큰 수를
확인할 수 있습니다. 긴 요청 문자열과 SSE event payload는 해당 row를 펼쳤을
때 렌더링됩니다. 왼쪽 목록 위의 검색창은 row 메타데이터와 저장된
request/response body 텍스트를 검색합니다. 빠른 필터로 에러, 느린 요청,
토큰 사용량이 큰 요청, body 저장이 잘린 요청만 볼 수 있습니다.

viewer는 민감한 요청/응답 내용을 보여주므로 loopback 주소에서만
실행됩니다. `0.0.0.0` 같은 외부 접속 주소로는 실행할 수 없습니다.

## 종료

서버가 실행 중인 터미널에서 `Ctrl-C`로 종료합니다.

서버가 꺼져 있으면 외부 컴퓨터의 `codex -p local-proxy`는 동작하지
않습니다.

## 주의사항

- `CODEX_PROXY_TOKEN=test`는 테스트용입니다. 실제 사용 시 긴 랜덤값으로
  바꿉니다.
- `--listen 0.0.0.0:8787`은 같은 네트워크의 다른 컴퓨터가 접속할 수 있게
  엽니다. 반드시 `--proxy-token-env CODEX_PROXY_TOKEN`을 같이 사용합니다.
- 이 프록시를 호출할 수 있는 사용자는 프록시 서버 컴퓨터의 Codex 계정
  사용량을 소비할 수 있습니다.
- SQLite DB에는 프롬프트, 코드, shell 출력, 패치 내용이 들어갈 수
  있으므로 민감 데이터로 취급합니다.
