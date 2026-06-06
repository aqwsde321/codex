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
      element.dataset.tip = text;
      element.setAttribute("aria-label", text);
      element.tabIndex = 0;
      element.addEventListener("mouseenter", (event) => showTooltip(text, event.clientX, event.clientY));
      element.addEventListener("mousemove", (event) => positionTooltip(event.clientX, event.clientY));
      element.addEventListener("mouseleave", hideTooltip);
      element.addEventListener("click", () => {
        const rect = element.getBoundingClientRect();
        showTooltip(text, rect.left + rect.width / 2, rect.bottom);
      });
      element.addEventListener("focus", () => {
        const rect = element.getBoundingClientRect();
        showTooltip(text, rect.left + rect.width / 2, rect.bottom);
      });
      element.addEventListener("blur", hideTooltip);
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
