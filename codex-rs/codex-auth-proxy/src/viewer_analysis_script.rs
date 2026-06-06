pub(crate) const JS: &str = r#"
    const THRESHOLDS = {
      latencyWarningMs: 30000,
      latencyCriticalMs: 60000,
      bytesWarning: 1024 * 1024,
      bytesCritical: 5 * 1024 * 1024,
      inputTokensWarning: 100000,
      inputTokensCritical: 200000,
      outputTokensWarning: 8000,
      outputTokensCritical: 20000,
      totalTokensWarning: 120000,
      totalTokensCritical: 250000,
      reasoningTokensWarning: 8000,
      reasoningTokensCritical: 20000,
      toolOutputsWarning: 100,
      toolOutputsCritical: 250,
      toolOutputCharsWarning: 100000,
      toolOutputCharsCritical: 500000,
      eventsWarning: 500,
      eventsCritical: 1000,
      lowCacheMinInputTokens: 20000,
      lowCacheRatio: 0.2,
    };

    const TOOLTIP_TRANSLATIONS = {
      "This request has not been marked completed in the proxy log yet.": "아직 프록시 로그에서 완료 처리되지 않은 요청입니다.",
      "Upstream returned an error status. Check Error, Raw SSE, and the request body.": "업스트림 API가 오류 상태를 반환했습니다. Error, Raw SSE, 요청 본문을 확인하세요.",
      "The proxy recorded an error while forwarding or streaming this request. Check the Error tab first.": "프록시가 요청 전달 또는 스트리밍 중 오류를 기록했습니다. 먼저 Error 탭을 확인하세요.",
      "SSE events exist but no output text was extracted. Inspect Response Events or Raw SSE.": "SSE 이벤트는 있지만 추출된 응답 텍스트가 없습니다. Response Events 또는 Raw SSE를 확인하세요.",
      "The assistant response or recent tool output contains failure/error wording. Check Assistant answered and Request Messages.": "assistant 응답이나 최근 tool output에 실패/오류 관련 문구가 있습니다. Assistant answered와 Request Messages를 확인하세요.",
      "The upstream response took longer than the local threshold.": "업스트림 응답 시간이 로컬 기준보다 오래 걸렸습니다.",
      "Many tool outputs were sent back to the model. Check Request Messages for repeated shell output, file dumps, or test logs.": "많은 tool output이 다시 모델 컨텍스트로 전달됐습니다. 반복된 쉘 출력, 파일 덤프, 테스트 로그가 있는지 Request Messages를 확인하세요.",
      "Stored tool output text is large. Check Tool I/O for the biggest outputs and commands.": "저장된 tool output 텍스트가 큽니다. Tool I/O에서 가장 큰 출력과 명령을 확인하세요.",
      "The proxy stored only the first bytes of this request body in SQLite. Upstream traffic was not truncated.": "프록시는 이 요청 본문의 앞부분만 SQLite에 저장했습니다. 실제 업스트림 트래픽은 잘리지 않았습니다.",
      "The proxy stored only the first bytes of this response body in SQLite. Upstream traffic was not truncated.": "프록시는 이 응답 본문의 앞부분만 SQLite에 저장했습니다. 실제 업스트림 트래픽은 잘리지 않았습니다.",
      "The original request body is large. SQLite storage may be truncated depending on the proxy log body limit.": "원본 요청 본문이 큽니다. 프록시 로그 본문 제한 설정에 따라 SQLite 저장본은 잘렸을 수 있습니다.",
      "The original response body is large. SQLite storage may be truncated depending on the proxy log body limit.": "원본 응답 본문이 큽니다. 프록시 로그 본문 제한 설정에 따라 SQLite 저장본은 잘렸을 수 있습니다.",
      "The response contains many stream events. Use Response Events to inspect where output grew.": "응답에 많은 스트림 이벤트가 있습니다. 출력이 어디서 커졌는지 Response Events에서 확인하세요.",
      "Reported token usage is high. Use this as a cost/context signal, then inspect Request Messages to find the cause.": "보고된 토큰 사용량이 큽니다. 비용/컨텍스트 신호로 보고, 원인은 Request Messages에서 확인하세요.",
      "Input token usage is high but cached input ratio is low.": "입력 토큰 사용량은 큰데 캐시된 입력 비율이 낮습니다.",
      "No token usage was found in the upstream response. Token columns remain empty for this row.": "업스트림 응답에서 token usage를 찾지 못했습니다. 이 row의 토큰 컬럼은 비어 있습니다.",
      "Status, proxy errors, latency, body size, stream events, tool outputs, and token usage are within local thresholds.": "상태, 프록시 오류, 지연 시간, 본문 크기, 스트림 이벤트, tool output, 토큰 사용량이 로컬 기준 안에 있습니다.",
      "HTTP method used by the Codex client.": "Codex 클라이언트가 사용한 HTTP 메서드입니다.",
      "Proxy endpoint requested by the client.": "클라이언트가 요청한 프록시 엔드포인트입니다.",
      "Model value sent in the request body.": "요청 본문에 담겨 전송된 model 값입니다.",
      "Local time when the proxy received the request.": "프록시가 요청을 받은 로컬 시간입니다.",
      "Number of items in the Responses input array.": "Responses 요청의 input 배열에 들어 있는 item 개수입니다.",
      "Number of tools advertised to the model.": "모델에 제공된 tool 개수입니다.",
      "Number of tool result items sent back to the model. High values often mean shell output, file content, or repeated tool loops are growing context.": "모델로 다시 전달된 tool 결과 item 개수입니다. 값이 크면 쉘 출력, 파일 내용, 반복 tool 루프가 컨텍스트를 키우는 경우가 많습니다.",
      "Total stored characters across tool output input items in this request.": "이 요청 안의 tool output input item들에 저장된 총 글자 수입니다.",
      "Characters in the top-level instructions field.": "최상위 instructions 필드의 글자 수입니다.",
      "Original request body size seen by the proxy before any SQLite storage truncation.": "SQLite 저장 제한으로 잘리기 전, 프록시가 본 원본 요청 본문 크기입니다.",
      "Whether the request body text stored in SQLite was truncated by the proxy log body limit.": "SQLite에 저장된 요청 본문 텍스트가 프록시 로그 본문 제한으로 잘렸는지 여부입니다.",
      "HTTP status returned by the upstream API.": "업스트림 API가 반환한 HTTP 상태입니다.",
      "Elapsed time from proxy request start until the upstream response completed.": "프록시 요청 시작부터 업스트림 응답 수신 완료까지 걸린 시간입니다.",
      "Local time when the proxy finished receiving the upstream response.": "프록시가 업스트림 응답 수신을 완료한 로컬 시간입니다.",
      "Number of SSE events in the response. High counts can indicate long streaming output or many tool/result updates.": "응답에 포함된 SSE 이벤트 개수입니다. 값이 크면 긴 스트리밍 출력이나 많은 tool/result 업데이트를 의미할 수 있습니다.",
      "Distinct output items observed in response events.": "응답 이벤트에서 관찰된 고유 output item 개수입니다.",
      "Extracted assistant text from response output_text events.": "응답의 output_text 이벤트에서 추출한 assistant 텍스트입니다.",
      "Original response body size seen by the proxy before any SQLite storage truncation.": "SQLite 저장 제한으로 잘리기 전, 프록시가 본 원본 응답 본문 크기입니다.",
      "Whether the response body text stored in SQLite was truncated by the proxy log body limit.": "SQLite에 저장된 응답 본문 텍스트가 프록시 로그 본문 제한으로 잘렸는지 여부입니다.",
      "Prompt/context tokens sent to the model.": "모델에 전달된 프롬프트/컨텍스트 토큰 수입니다.",
      "Tokens generated by the model.": "모델이 생성한 토큰 수입니다.",
      "Total reported token usage for this response.": "이 응답에 대해 보고된 총 토큰 사용량입니다.",
      "Input tokens served from prompt cache. Higher is usually better for repeated Codex context.": "프롬프트 캐시에서 처리된 입력 토큰 수입니다. 반복되는 Codex 컨텍스트에서는 높을수록 보통 유리합니다.",
      "Tokens reported for model reasoning output.": "모델 reasoning output으로 보고된 토큰 수입니다.",
    };

    function attentionItems(detail, derived) {
      const request = derived.requestInfo;
      const items = [];
      if (!detail.completed_at) {
        items.push({
          severity: "info",
          title: "In progress",
          tip: "This request has not been marked completed in the proxy log yet.",
        });
      }
      addAttention(items, statusSeverity(detail.upstream_status), "HTTP error", "Upstream returned an error status. Check Error, Raw SSE, and the request body.");
      addAttention(items, detail.error ? "critical" : null, "Proxy error", "The proxy recorded an error while forwarding or streaming this request. Check the Error tab first.");
      if (derived.events.length > 0 && derived.responseText.length === 0 && !detail.error) {
        items.push({
          severity: "warning",
          title: "No response text",
          tip: "SSE events exist but no output text was extracted. Inspect Response Events or Raw SSE.",
        });
      }
      if (hasFailureText(derived.responseText) || hasFailureText(request.latestToolOutputText)) {
        items.push({
          severity: "warning",
          title: "Failure text",
          tip: "The assistant response or recent tool output contains failure/error wording. Check Assistant answered and Request Messages.",
        });
      }
      addAttention(items, latencySeverity(detail.latency_ms), "Slow response", "The upstream response took longer than the local threshold.");
      addAttention(items, countSeverity(request.toolOutputCount || 0, THRESHOLDS.toolOutputsWarning, THRESHOLDS.toolOutputsCritical), "Many tool outputs", "Many tool outputs were sent back to the model. Check Request Messages for repeated shell output, file dumps, or test logs.");
      addAttention(items, charsSeverity(request.toolOutputChars || 0), "Large tool output", "Stored tool output text is large. Check Tool I/O for the biggest outputs and commands.");
      addAttention(items, detail.request_body_truncated ? "warning" : null, "Request log truncated", "The proxy stored only the first bytes of this request body in SQLite. Upstream traffic was not truncated.");
      addAttention(items, detail.response_body_truncated ? "warning" : null, "Response log truncated", "The proxy stored only the first bytes of this response body in SQLite. Upstream traffic was not truncated.");
      addAttention(items, bytesSeverity(detail.request_bytes), "Large request body", "The original request body is large. SQLite storage may be truncated depending on the proxy log body limit.");
      addAttention(items, bytesSeverity(detail.response_bytes), "Large response body", "The original response body is large. SQLite storage may be truncated depending on the proxy log body limit.");
      addAttention(items, countSeverity(derived.events.length, THRESHOLDS.eventsWarning, THRESHOLDS.eventsCritical), "Many SSE events", "The response contains many stream events. Use Response Events to inspect where output grew.");
      addAttention(items, tokenUsageSeverity(detail), "Large context/cost", "Reported token usage is high. Use this as a cost/context signal, then inspect Request Messages to find the cause.");
      addAttention(items, cacheSeverity(detail.input_tokens, detail.cached_input_tokens), "Low cache", "Input token usage is high but cached input ratio is low.");
      if (detail.path === "/v1/responses" && detail.upstream_status === 200 && !hasTokenUsage(detail)) {
        items.push({
          severity: "info",
          title: "Usage not reported",
          tip: "No token usage was found in the upstream response. Token columns remain empty for this row.",
        });
      }
      if (items.length === 0) {
        items.push({
          severity: "info",
          title: "No highlighted issues",
          tip: "Status, proxy errors, latency, body size, stream events, tool outputs, and token usage are within local thresholds.",
        });
      }
      return items.sort((left, right) => severityRank(right.severity) - severityRank(left.severity));
    }

    function addAttention(items, severity, title, tip) {
      if (!severity) return;
      items.push({ severity, title, tip });
    }

    function attentionBlock(items) {
      const block = document.createElement("div");
      block.className = "attention";
      const heading = document.createElement("h3");
      heading.textContent = "Attention";
      const list = document.createElement("div");
      list.className = "attention-list";
      for (const item of items) {
        const badge = document.createElement("span");
        badge.className = `badge ${item.severity}`;
        badge.textContent = item.title;
        attachTooltip(badge, item.tip);
        list.appendChild(badge);
      }
      block.append(heading, list);
      return block;
    }

    function attachTooltip(element, text) {
      if (!text) return;
      const displayText = tooltipText(text);
      element.dataset.tip = displayText;
      element.setAttribute("aria-label", displayText);
      element.tabIndex = 0;
      element.addEventListener("mouseenter", (event) => showTooltip(displayText, event.clientX, event.clientY));
      element.addEventListener("mousemove", (event) => positionTooltip(event.clientX, event.clientY));
      element.addEventListener("mouseleave", hideTooltip);
      element.addEventListener("click", () => {
        const rect = element.getBoundingClientRect();
        showTooltip(displayText, rect.left + rect.width / 2, rect.bottom);
      });
      element.addEventListener("focus", () => {
        const rect = element.getBoundingClientRect();
        showTooltip(displayText, rect.left + rect.width / 2, rect.bottom);
      });
      element.addEventListener("blur", hideTooltip);
    }

    function tooltipText(text) {
      const translated = TOOLTIP_TRANSLATIONS[text];
      return translated ? `${text} (${translated})` : text;
    }

    function showTooltip(text, x, y) {
      const tooltip = viewerTooltip();
      tooltip.textContent = text;
      tooltip.classList.add("visible");
      positionTooltip(x, y);
    }

    function hideTooltip() {
      viewerTooltip().classList.remove("visible");
    }

    function positionTooltip(x, y) {
      const tooltip = viewerTooltip();
      const margin = 12;
      const offset = 14;
      const rect = tooltip.getBoundingClientRect();
      let left = x + offset;
      let top = y + offset;
      if (left + rect.width + margin > window.innerWidth) {
        left = window.innerWidth - rect.width - margin;
      }
      if (top + rect.height + margin > window.innerHeight) {
        top = y - rect.height - offset;
      }
      tooltip.style.left = `${Math.max(margin, left)}px`;
      tooltip.style.top = `${Math.max(margin, top)}px`;
    }

    function viewerTooltip() {
      let tooltip = document.querySelector(".viewer-tooltip");
      if (!tooltip) {
        tooltip = document.createElement("div");
        tooltip.className = "viewer-tooltip";
        document.body.appendChild(tooltip);
      }
      return tooltip;
    }

    function statusSeverity(status) {
      if (status == null) return null;
      return status >= 400 ? "critical" : null;
    }

    function latencySeverity(value) {
      if (value == null) return null;
      if (value >= THRESHOLDS.latencyCriticalMs) return "critical";
      if (value >= THRESHOLDS.latencyWarningMs) return "warning";
      return null;
    }

    function bytesSeverity(value) {
      if (value == null) return null;
      if (value >= THRESHOLDS.bytesCritical) return "critical";
      if (value >= THRESHOLDS.bytesWarning) return "warning";
      return null;
    }

    function charsSeverity(value) {
      if (value == null) return null;
      if (value >= THRESHOLDS.toolOutputCharsCritical) return "critical";
      if (value >= THRESHOLDS.toolOutputCharsWarning) return "warning";
      return null;
    }

    function countSeverity(value, warning, critical) {
      if (value == null) return null;
      if (value >= critical) return "critical";
      if (value >= warning) return "warning";
      return null;
    }

    function cacheSeverity(inputTokens, cachedInputTokens) {
      if (inputTokens == null || cachedInputTokens == null) return null;
      if (inputTokens < THRESHOLDS.lowCacheMinInputTokens) return null;
      const ratio = cachedInputTokens / inputTokens;
      return ratio < THRESHOLDS.lowCacheRatio ? "warning" : null;
    }

    function tokenUsageSeverity(detail) {
      if (
        detail.input_tokens >= THRESHOLDS.inputTokensWarning
        || detail.output_tokens >= THRESHOLDS.outputTokensWarning
        || detail.total_tokens >= THRESHOLDS.totalTokensWarning
        || detail.reasoning_output_tokens >= THRESHOLDS.reasoningTokensWarning
      ) {
        return "warning";
      }
      return null;
    }

    function hasFailureText(value) {
      return /(\b(error|failed|failure|exception|panic|fatal|traceback|denied|timeout)\b|INSTALL_FAILED|BUILD FAILED|오류|실패|불일치)/i.test(value || "");
    }

    function hasTokenUsage(detail) {
      return detail.input_tokens != null
        || detail.output_tokens != null
        || detail.total_tokens != null
        || detail.cached_input_tokens != null
        || detail.reasoning_output_tokens != null;
    }

    function severityRank(severity) {
      if (severity === "critical") return 3;
      if (severity === "warning") return 2;
      if (severity === "info") return 1;
      return 0;
    }
"#;
