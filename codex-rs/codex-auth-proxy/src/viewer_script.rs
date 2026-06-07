pub(crate) const JS: &str = r#"
    const DEFAULT_EVENT_LIMIT = 200;
    const MAX_REQUEST_MESSAGES = 120;
    const LONG_STRING_LIMIT = 1200;
    const PREVIEW_LIMIT = 2400;

    const state = {
      requests: [],
      selected: null,
      detail: null,
      derived: null,
      view: "summary",
      eventLimit: DEFAULT_EVENT_LIMIT,
      filter: "all",
      search: "",
      searchTimer: null,
      flow: null,
      flowSelected: null,
      flowError: null,
      flowLoading: false,
    };

    const listEl = document.getElementById("list");
    const searchEl = document.getElementById("search");
    const statusEl = document.getElementById("status");
    const detailTitleEl = document.getElementById("detail-title");
    const detailMetaEl = document.getElementById("detail-meta");
    const detailStatusEl = document.getElementById("detail-status");
    const bodyEl = document.getElementById("body");

    document.getElementById("refresh").addEventListener("click", () => loadRequests());
    searchEl.addEventListener("input", () => {
      state.search = searchEl.value;
      window.clearTimeout(state.searchTimer);
      state.searchTimer = window.setTimeout(() => loadRequests(), 250);
    });
    document.querySelectorAll(".filter-button").forEach((button) => {
      button.addEventListener("click", () => {
        state.filter = button.dataset.filter;
        document.querySelectorAll(".filter-button").forEach((item) => item.classList.toggle("active", item === button));
        loadRequests();
      });
    });
    document.querySelectorAll(".tab").forEach((button) => {
      button.addEventListener("click", () => {
        state.view = button.dataset.view;
        document.querySelectorAll(".tab").forEach((tab) => tab.classList.toggle("active", tab === button));
        renderDetail();
      });
    });

    async function loadRequests() {
      statusEl.textContent = "Loading";
      const requests = await fetchJson(requestsUrl());
      state.requests = requests;
      renderList();
      statusEl.textContent = `${requests.length} ${hasActiveListFilter() ? "matches" : "rows"}`;
      if (requests.length === 0) {
        state.selected = null;
        state.detail = null;
        state.derived = null;
        state.flow = null;
        state.flowSelected = null;
        state.flowError = null;
        state.flowLoading = false;
        renderDetail();
      } else if (!state.selected || !requests.some((request) => request.id === state.selected)) {
        await selectRequest(requests[0].id);
      } else if (state.selected) {
        await selectRequest(state.selected);
      }
    }

    async function selectRequest(id) {
      const keepFlow = state.flow && (state.flow.rows || []).some((row) => row.id === id);
      state.selected = id;
      state.eventLimit = DEFAULT_EVENT_LIMIT;
      if (keepFlow) {
        state.flowSelected = id;
      } else {
        state.flow = null;
        state.flowSelected = null;
      }
      state.flowError = null;
      state.flowLoading = false;
      state.detail = await fetchJson(`/api/requests/${encodeURIComponent(id)}`);
      state.derived = deriveDetail(state.detail);
      renderList();
      renderDetail();
      ensureFlowLoaded(id);
    }

    function renderList() {
      listEl.replaceChildren();
      if (state.selected) ensureFlowLoaded(state.selected);
      if (state.requests.length === 0) {
        const empty = document.createElement("div");
        empty.className = "empty";
        empty.style.padding = "14px";
        empty.textContent = hasActiveListFilter() ? "No matching requests" : "No requests";
        listEl.appendChild(empty);
        return;
      }

      const flowRows = currentFlowRows();
      const flowStepById = flowRows.length > 1
        ? new Map(flowRows.map((row, index) => [row.id, { index: index + 1, total: flowRows.length }]))
        : new Map();
      state.requests.forEach((request, index) => {
        const flowStep = flowStepById.get(request.id) || null;
        const previousInFlow = index > 0 && flowStepById.has(state.requests[index - 1].id);
        const nextInFlow = index + 1 < state.requests.length && flowStepById.has(state.requests[index + 1].id);
        listEl.appendChild(requestRow(request, flowStep, { previousInFlow, nextInFlow }));
      });
    }

    function currentFlowRows() {
      if (!state.flow || state.flowSelected !== state.selected) return [];
      return state.flow.rows || [];
    }

    function requestRow(request, flowStep, flowGroup) {
      const row = document.createElement("button");
      row.type = "button";
      row.className = "row";
      row.classList.toggle("flow-step-row", flowStep != null);
      if (flowStep != null) {
        row.classList.toggle("flow-run-start", !flowGroup.previousInFlow);
        row.classList.toggle("flow-run-end", !flowGroup.nextInFlow);
      }
      row.classList.toggle("active", request.id === state.selected);
      row.addEventListener("click", () => selectRequest(request.id));

      const title = document.createElement("div");
      title.className = "row-title";
      if (flowStep != null) {
        const step = document.createElement("span");
        step.className = "flow-step-badge";
        step.textContent = `${flowStep.index}/${flowStep.total}`;
        title.appendChild(step);
      }
      const titleText = document.createElement("span");
      titleText.className = "row-title-text";
      titleText.textContent = request.model || request.path;
      title.appendChild(titleText);

      const status = document.createElement("div");
      status.className = `status-code ${isErrorStatus(request.upstream_status) ? "error" : ""}`;
      status.textContent = request.upstream_status || "-";

      const sub = document.createElement("div");
      sub.className = "row-sub";
      sub.textContent = `${request.method} ${request.path}${request.query ? "?" + request.query : ""}`;

      const badges = rowBadges(request);
      const meta = document.createElement("div");
      meta.className = "row-meta";
      const metaText = document.createElement("span");
      metaText.textContent = rowMeta(request);
      meta.append(metaText, badges);
      row.append(title, status, sub, meta);
      return row;
    }

    function requestsUrl() {
      const params = new URLSearchParams();
      params.set("limit", "200");
      if (state.filter !== "all") params.set("filter", state.filter);
      const search = state.search.trim();
      if (search) params.set("q", search);
      return `/api/requests?${params.toString()}`;
    }

    function hasActiveListFilter() {
      return state.filter !== "all" || state.search.trim().length > 0;
    }

    function rowMeta(request) {
      const parts = [`${request.latency_ms ?? "-"} ms`];
      if (request.total_tokens != null) parts.push(`${formatCount(request.total_tokens)} tok`);
      return parts.join(" · ");
    }

    function rowBadges(request) {
      const badges = document.createElement("span");
      badges.className = "row-badges";
      const items = [];
      if (isErrorStatus(request.upstream_status) || request.error) items.push(["err", "critical", "HTTP/proxy error"]);
      if ((request.latency_ms || 0) >= THRESHOLDS.latencyWarningMs) items.push(["slow", "warning", "Slow response"]);
      if (rowHasHighTokens(request)) items.push(["tokens", "warning", "High token usage"]);
      if (request.request_body_truncated || request.response_body_truncated) items.push(["trunc", "warning", "Stored body truncated"]);

      for (const [label, severity, tip] of items) {
        const badge = document.createElement("span");
        badge.className = `row-badge ${severity}`;
        badge.textContent = label;
        attachTooltip(badge, tip);
        badges.appendChild(badge);
      }
      return badges;
    }

    function rowHasHighTokens(request) {
      return (request.input_tokens || 0) >= THRESHOLDS.inputTokensWarning
        || (request.output_tokens || 0) >= THRESHOLDS.outputTokensWarning
        || (request.total_tokens || 0) >= THRESHOLDS.totalTokensWarning
        || (request.reasoning_output_tokens || 0) >= THRESHOLDS.reasoningTokensWarning;
    }

    function renderDetail() {
      const detail = state.detail;
      const derived = state.derived;
      bodyEl.replaceChildren();
      bodyEl.classList.toggle("summary-mode", state.view === "summary");
      if (!detail || !derived) {
        detailTitleEl.textContent = "No request selected";
        detailMetaEl.replaceChildren();
        detailStatusEl.textContent = "";
        return;
      }

      detailTitleEl.textContent = `${detail.method} ${detail.path}`;
      detailStatusEl.textContent = detail.upstream_status || "-";
      detailStatusEl.className = `status-code ${isErrorStatus(detail.upstream_status) ? "error" : ""}`;
      renderMeta(detail);

      if (state.view === "summary") {
        renderSummary(detail, derived);
      } else if (state.view === "messages") {
        renderRequestMessages(derived.requestInfo);
      } else if (state.view === "tools") {
        renderToolIo(derived.requestInfo, derived.responseInfo);
      } else if (state.view === "request") {
        if (detail.request_body_truncated) {
          bodyEl.append(truncationNotice("Request body", detail.request_bytes));
        }
        renderJsonOrText(derived.requestJson, detail.request_body || "", "request");
      } else if (state.view === "text") {
        if (detail.response_body_truncated) {
          bodyEl.append(truncationNotice("Response body", detail.response_bytes));
        }
        bodyEl.append(textBlock(derived.responseText || "(empty)"));
      } else if (state.view === "events") {
        if (detail.response_body_truncated) {
          bodyEl.append(truncationNotice("Response body", detail.response_bytes));
        }
        renderSseEvents(derived.events);
      } else if (state.view === "raw") {
        if (detail.response_body_truncated) {
          bodyEl.append(truncationNotice("Response body", detail.response_bytes));
        }
        bodyEl.append(textBlock(detail.response_body || ""));
      } else {
        bodyEl.append(textBlock(detail.error || ""));
      }
    }

    function renderMeta(detail) {
      detailMetaEl.replaceChildren();
      const values = [
        ["id", detail.id],
        ["started", formatTimestamp(detail.started_at)],
        ["completed", formatTimestamp(detail.completed_at)],
        ["client", detail.client_ip || "-"],
        ["model", detail.model || "-"],
        ["duration", formatDurationMs(detail.latency_ms)],
        ["bytes", `${formatBytes(detail.request_bytes)} / ${formatBytes(detail.response_bytes)}`],
      ];
      for (const [name, value] of values) {
        const item = document.createElement("span");
        item.textContent = `${name}: ${value}`;
        detailMetaEl.appendChild(item);
      }
    }

    function deriveDetail(detail) {
      const requestJson = parseJson(detail.request_body || "");
      const events = parseSse(detail.response_body || "");
      return {
        requestJson,
        requestInfo: summarizeRequest(requestJson),
        events,
        responseText: extractResponseText(events),
        responseInfo: summarizeResponse(events),
      };
    }

    function summarizeRequest(parsed) {
      if (!parsed.ok || !parsed.value || typeof parsed.value !== "object") {
        return { parsed: false, messages: [], tools: [], latestText: "", inputCount: 0 };
      }

      const request = parsed.value;
      const input = Array.isArray(request.input) ? request.input : [];
      const tools = Array.isArray(request.tools) ? request.tools.map(describeTool) : [];
      const messages = input.map(describeInputItem);
      const latest = [...messages].reverse().find((item) => item.text);
      const latestUser = [...messages].reverse().find((item) => item.role === "user" && item.text);
      const toolCalls = input.map(describeToolCall).filter(Boolean);
      const callNames = new Map(toolCalls.filter((call) => call.callId).map((call) => [call.callId, call.name]));
      const toolOutputMessages = messages.filter(isToolOutputMessage);
      const toolOutputs = toolOutputMessages.map((item) => ({
        ...item,
        name: callNames.get(item.callId) || item.name || item.kind,
      })).sort((left, right) => right.chars - left.chars);
      return {
        parsed: true,
        model: request.model || "",
        instructionsChars: typeof request.instructions === "string" ? request.instructions.length : 0,
        inputCount: input.length,
        toolCount: tools.length,
        tools,
        messages,
        latestText: latest?.text || "",
        latestUserText: latestUser?.text || "",
        toolCalls,
        toolOutputs,
        latestToolOutputText: [...toolOutputMessages].reverse().find((item) => item.text)?.text || "",
        toolOutputCount: toolOutputs.length,
        toolOutputChars: toolOutputs.reduce((sum, item) => sum + item.chars, 0),
      };
    }

    function describeTool(tool) {
      if (!tool || typeof tool !== "object") return { name: String(tool), type: "" };
      const name = tool.name || tool.function?.name || tool.type || "(unnamed)";
      const type = tool.type || tool.function?.type || "";
      return { name, type };
    }

    function describeInputItem(item, index) {
      const text = inputItemText(item);
      const role = item && typeof item === "object" ? item.role || "" : "";
      const kind = item && typeof item === "object" ? item.type || role || "item" : "text";
      return {
        index: index + 1,
        role: role || "-",
        kind,
        name: item && typeof item === "object" ? item.name || "" : "",
        callId: item && typeof item === "object" ? item.call_id || "" : "",
        chars: text.length,
        text,
      };
    }

    function isToolOutputMessage(item) {
      return item.kind.includes("output") || item.kind.includes("call_result");
    }

    function describeToolCall(item, index) {
      if (!item || typeof item !== "object" || item.type !== "function_call") return null;
      const name = item.name || "function_call";
      const args = parseMaybeJson(item.arguments);
      const rawArgs = typeof item.arguments === "string" ? item.arguments : JSON.stringify(item.arguments ?? "");
      return {
        index: index + 1,
        name,
        callId: item.call_id || "",
        summary: summarizeToolCall(name, args, rawArgs),
        important: isImportantTool(name, args),
      };
    }

    function parseMaybeJson(value) {
      if (!value || typeof value !== "string") return value && typeof value === "object" ? value : {};
      try {
        const parsed = JSON.parse(value);
        return parsed && typeof parsed === "object" ? parsed : {};
      } catch {
        return {};
      }
    }

    function summarizeToolCall(name, args, rawArgs) {
      if (name === "exec_command") return args.cmd || preview(rawArgs, 180);
      if (name === "write_stdin") {
        const text = args.chars ? ` ${JSON.stringify(args.chars)}` : "";
        return `session ${args.session_id ?? "-"}${text}`;
      }
      if (name === "apply_patch") return summarizePatch(rawArgs);
      if (name === "view_image") return args.path || preview(rawArgs, 180);
      if (name === "web_search") return args.query || args.q || preview(rawArgs, 180);
      return args.cmd || args.path || args.prompt || args.question || preview(rawArgs, 180);
    }

    function summarizePatch(rawArgs) {
      const files = [...rawArgs.matchAll(/\*\*\* (?:Update|Add|Delete) File: ([^\n]+)/g)].map((match) => match[1]);
      return files.length > 0 ? files.join(", ") : preview(rawArgs, 180);
    }

    function isImportantTool(name, args) {
      if (["exec_command", "apply_patch", "view_image", "web_search"].includes(name)) return true;
      return Boolean(args?.cmd || args?.path || args?.prompt || args?.question);
    }

    function inputItemText(item) {
      if (typeof item === "string") return item;
      if (!item || typeof item !== "object") return String(item ?? "");
      return firstText([
        contentText(item.content),
        item.output,
        item.input,
        item.arguments,
        item.text,
        item.summary,
      ]);
    }

    function summarizeResponse(events) {
      const counts = new Map();
      const items = [];
      const seen = new Set();
      for (const event of events) {
        const name = event.event || "message";
        counts.set(name, (counts.get(name) || 0) + 1);
        const parsed = event.parsed;
        addOutputItem(parsed?.item, items, seen);
        addOutputItem(parsed?.output_item, items, seen);
        if (Array.isArray(parsed?.response?.output)) {
          for (const item of parsed.response.output) addOutputItem(item, items, seen);
        }
      }
      return { eventCounts: [...counts.entries()], items };
    }

    function addOutputItem(item, items, seen) {
      if (!item || typeof item !== "object") return;
      const id = item.id || item.call_id || JSON.stringify([item.type, item.name, item.status, outputItemText(item).slice(0, 80)]);
      const text = outputItemText(item);
      if (seen.has(id)) {
        const existing = items.find((candidate) => candidate.id === id);
        if (!existing) return;
        existing.name = item.name || item.role || existing.name;
        existing.status = item.status || existing.status;
        if (text) existing.text = text;
        return;
      }
      seen.add(id);
      items.push({
        id,
        type: item.type || "item",
        name: item.name || item.role || item.status || "",
        status: item.status || "",
        text,
      });
    }

    function outputItemText(item) {
      if (!item || typeof item !== "object") return "";
      return firstText([
        item.arguments,
        item.input,
        item.output,
        item.text,
        item.summary,
        contentText(item.content),
      ]);
    }

    function renderSummary(detail, derived) {
      const request = derived.requestInfo;
      const response = derived.responseInfo;
      const content = document.createElement("div");
      content.className = "summary-content";
      content.appendChild(keyInfoBlock(detail, derived));
      const growthItems = growthAnalysis(detail, derived);
      content.appendChild(mainCauseBlock(growthItems[0]));

      const grid = document.createElement("div");
      grid.className = "summary-grid";

      grid.append(
        panel(
          "Request",
          metrics([
            metric("Input items", request.inputCount || 0, "Number of items in the Responses input array."),
            metric("Tools", request.toolCount || 0, "Number of tools advertised to the model."),
            metric(
              "Tool outputs",
              request.toolOutputCount || 0,
              "Number of tool result items sent back to the model. High values often mean shell output, file content, or repeated tool loops are growing context.",
              countSeverity(request.toolOutputCount || 0, THRESHOLDS.toolOutputsWarning, THRESHOLDS.toolOutputsCritical)
            ),
            metric(
              "Tool output chars",
              formatChars(request.toolOutputChars || 0),
              "Total stored characters across tool output input items in this request.",
              charsSeverity(request.toolOutputChars || 0)
            ),
            metric("Instructions", formatChars(request.instructionsChars || 0), "Characters in the top-level instructions field."),
            metric(
              "Request size",
              formatBytes(detail.request_bytes),
              "Original request body size seen by the proxy before any SQLite storage truncation.",
              bytesSeverity(detail.request_bytes)
            ),
            detail.request_body_truncated ? metric(
              "Request stored",
              storedBodyStatus(detail.request_body_truncated),
              "Whether the request body text stored in SQLite was truncated by the proxy log body limit.",
              "warning"
            ) : null,
          ]),
          chipsDisclosure("Tool names", `${request.tools.length} tools`, request.tools.map((tool) => tool.name)),
          previewBlock("Latest request text", request.latestText || "(empty)")
        ),
        panel(
          "Response",
          metrics([
            metric(
              "Status",
              detail.upstream_status || "-",
              "HTTP status returned by the upstream API.",
              statusSeverity(detail.upstream_status)
            ),
            metric(
              "Duration",
              formatDurationMs(detail.latency_ms),
              "Elapsed time from proxy request start until the upstream response completed.",
              latencySeverity(detail.latency_ms)
            ),
            metric(
              "Events",
              derived.events.length,
              "Number of SSE events in the response. High counts can indicate long streaming output or many tool/result updates.",
              countSeverity(derived.events.length, THRESHOLDS.eventsWarning, THRESHOLDS.eventsCritical)
            ),
            metric("Text", formatChars(derived.responseText.length), "Extracted assistant text from response output_text events."),
            metric(
              "Response size",
              formatBytes(detail.response_bytes),
              "Original response body size seen by the proxy before any SQLite storage truncation.",
              bytesSeverity(detail.response_bytes)
            ),
            detail.response_body_truncated ? metric(
              "Response stored",
              storedBodyStatus(detail.response_body_truncated),
              "Whether the response body text stored in SQLite was truncated by the proxy log body limit.",
              "warning"
            ) : null,
            metric(
              "Input tokens",
              formatCount(detail.input_tokens),
              "Prompt/context tokens sent to the model."
            ),
            metric(
              "Total tokens",
              formatCount(detail.total_tokens),
              "Total reported token usage for this response."
            ),
            detail.cached_input_tokens != null ? metric(
              "Cached input",
              formatCount(detail.cached_input_tokens),
              "Input tokens served from prompt cache. Higher is usually better for repeated Codex context."
            ) : null,
            detail.reasoning_output_tokens ? metric(
              "Reasoning output",
              formatCount(detail.reasoning_output_tokens),
              "Tokens reported for model reasoning output."
            ) : null,
          ]),
          previewBlock("Response text", derived.responseText || "(empty)"),
          outputItemsBlock(response.items)
        ),
        panel(
          "Growth Analysis",
          analysisBlock(growthItems),
          toolOutputRankBlock(request.toolOutputs || [], 3)
        )
      );
      content.appendChild(grid);
      content.appendChild(eventTypesDisclosure(response.eventCounts));
      bodyEl.appendChild(content);
    }

    async function loadFlow(id) {
      state.flowLoading = true;
      try {
        const flow = await fetchJson(`/api/requests/${encodeURIComponent(id)}/flow`);
        if (state.selected !== id) return;
        state.flow = flow;
        state.flowError = null;
      } catch (error) {
        if (state.selected !== id) return;
        state.flowError = error;
      } finally {
        if (state.selected === id) state.flowLoading = false;
      }
      if (state.selected === id) renderList();
    }

    function ensureFlowLoaded(id) {
      if (state.flowSelected !== id) {
        state.flowSelected = id;
        state.flow = null;
        state.flowError = null;
        state.flowLoading = false;
      }
      if (state.flow || state.flowError || state.flowLoading) return;
      loadFlow(id);
    }

    function growthAnalysis(detail, derived) {
      const request = derived.requestInfo;
      const items = [];
      const topOutput = request.toolOutputs?.[0];
      if (topOutput && topOutput.chars > 0) {
        items.push({
          title: "Largest tool output",
          value: formatChars(topOutput.chars),
          text: `${toolOutputTitle(topOutput)} is the biggest stored tool output in this request.`,
        });
      }
      if ((request.toolOutputChars || 0) > 0) {
        items.push({
          title: "Tool output total",
          value: formatChars(request.toolOutputChars),
          text: `${request.toolOutputCount || 0} tool output items are being sent back into model context.`,
        });
      }
      if ((request.instructionsChars || 0) >= 20000) {
        items.push({
          title: "Large instructions",
          value: formatChars(request.instructionsChars),
          text: "The top-level instructions field is large before any request messages or tool outputs are counted.",
        });
      }
      if (detail.request_bytes >= THRESHOLDS.bytesWarning) {
        items.push({
          title: "Large request body",
          value: formatBytes(detail.request_bytes),
          text: "The full request body is large. Check Messages and Tool I/O to find the largest injected items.",
        });
      }
      if (derived.events.length >= THRESHOLDS.eventsWarning) {
        items.push({
          title: "Many SSE events",
          value: formatCount(derived.events.length),
          text: "The response stream contains many events, usually from long output text or incremental tool updates.",
        });
      }
      if (detail.input_tokens != null && detail.input_tokens >= THRESHOLDS.inputTokensWarning) {
        items.push({
          title: "High input tokens",
          value: formatCount(detail.input_tokens),
          text: "Token usage confirms a large model context. The cause is usually instructions, conversation history, or tool outputs.",
        });
      }
      if (items.length === 0) {
        items.push({
          title: "No clear growth signal",
          value: "-",
          text: "Request size, tool output size, SSE event count, and token usage are below local growth thresholds.",
        });
      }
      return items.slice(0, 3);
    }

    function mainCauseBlock(item) {
      const block = document.createElement("div");
      block.className = "main-cause";
      if (!item || item.title === "No clear growth signal") block.classList.add("quiet");

      const label = document.createElement("span");
      label.className = "main-cause-label";
      label.textContent = "Main cause";

      const title = document.createElement("strong");
      title.textContent = item ? `${item.title} · ${item.value}` : "No clear growth signal";

      const text = document.createElement("span");
      text.className = "main-cause-text";
      text.textContent = item?.text || "No obvious request growth signal was detected.";

      block.append(label, title, text);
      return block;
    }

    function analysisBlock(items) {
      const list = document.createElement("div");
      list.className = "analysis-list";
      for (const item of items) {
        const row = document.createElement("div");
        row.className = "analysis-item";
        const title = document.createElement("div");
        title.className = "analysis-title";
        const label = document.createElement("span");
        label.textContent = item.title;
        const value = document.createElement("span");
        value.className = "count";
        value.textContent = item.value;
        title.append(label, value);
        const text = document.createElement("div");
        text.className = "analysis-text";
        text.textContent = item.text;
        row.append(title, text);
        list.appendChild(row);
      }
      return list;
    }

    function toolOutputRankBlock(outputs, limit = 6, includeDetails = false) {
      const fragment = document.createDocumentFragment();
      const label = document.createElement("div");
      label.className = "chips-title";
      label.textContent = "Largest tool outputs";
      fragment.appendChild(label);

      const list = document.createElement("div");
      list.className = "tool-list";
      for (const output of outputs.slice(0, limit)) {
        const row = document.createElement("div");
        row.className = "tool-row";
        const title = document.createElement("div");
        title.className = "tool-title";
        const name = document.createElement("span");
        name.textContent = toolOutputTitle(output);
        const size = document.createElement("span");
        size.className = "count";
        size.textContent = formatChars(output.chars);
        title.append(name, size);
        const summary = document.createElement("div");
        summary.className = "tool-summary";
        summary.textContent = preview(output.text || "(empty)", includeDetails ? 600 : 220);
        row.append(title, summary);
        if (includeDetails && output.text) {
          const details = document.createElement("details");
          const detailTitle = document.createElement("summary");
          detailTitle.textContent = "Full output";
          details.appendChild(detailTitle);
          lazyDetails(details, () => textBlock(output.text));
          row.appendChild(details);
        }
        list.appendChild(row);
      }
      if (outputs.length === 0) {
        list.appendChild(emptyText("No tool outputs"));
      }
      fragment.appendChild(list);
      return fragment;
    }

    function toolOutputTitle(output) {
      return `#${output.index} ${output.name || output.kind}`;
    }

    function keyInfoBlock(detail, derived) {
      const request = derived.requestInfo;
      const problems = attentionItems(detail, derived).filter((item) => item.title !== "No highlighted issues");
      const section = document.createElement("div");
      section.className = "key-info";

      const heading = document.createElement("h3");
      heading.textContent = "Key Info";

      const grid = document.createElement("div");
      grid.className = "key-info-grid";
      grid.append(
        keyInfoItem("User asked", compactTextBlock(request.latestUserText || request.latestText || "(empty)")),
        keyInfoItem("Problems", problemBlock(problems)),
        keyInfoItem("Assistant answered", compactTextBlock(responseTextForSummary(detail, derived))),
        keyInfoItem("Actions", actionsBlock(importantActions(derived)))
      );

      section.append(heading, grid);
      return section;
    }

    function responseTextForSummary(detail, derived) {
      if (derived.responseText) return derived.responseText;
      if (derived.responseInfo.items.some((item) => item.type.includes("function_call"))) {
        return "No assistant text; response requested a tool action.";
      }
      return detail.completed_at ? "(empty)" : "Response not completed yet.";
    }

    function importantActions(derived) {
      const responseActions = derived.responseInfo.items
        .filter((item) => item.type.includes("function_call"))
        .map((item) => ({
          name: item.name || item.type,
          summary: summarizeToolCall(item.name || item.type, parseMaybeJson(item.text), item.text),
          important: true,
        }));
      if (responseActions.length > 0) return responseActions.slice(-8);

      const requestCalls = derived.requestInfo.toolCalls || [];
      const important = requestCalls.filter((call) => call.important);
      return (important.length > 0 ? important : requestCalls).slice(-8);
    }

    function keyInfoItem(title, child) {
      const item = document.createElement("div");
      item.className = "key-item";
      const label = document.createElement("div");
      label.className = "key-title";
      label.textContent = title;
      item.append(label, child);
      return item;
    }

    function compactTextBlock(value) {
      const block = document.createElement("pre");
      block.className = "key-text";
      block.textContent = preview(value, 900);
      return block;
    }

    function actionsBlock(actions) {
      const list = document.createElement("div");
      list.className = "action-list";
      if (actions.length === 0) {
        list.appendChild(emptyText("No tool actions"));
        return list;
      }
      for (const action of actions) {
        const row = document.createElement("div");
        row.className = "action-row";
        const name = document.createElement("span");
        name.className = "action-name";
        name.textContent = action.name;
        const summary = document.createElement("span");
        summary.className = "action-summary";
        summary.textContent = action.summary || "(empty)";
        row.append(name, summary);
        list.appendChild(row);
      }
      return list;
    }

    function problemBlock(items) {
      const list = document.createElement("div");
      list.className = "attention-list";
      if (items.length === 0) {
        list.appendChild(emptyText("No obvious problems"));
        return list;
      }
      for (const item of items) {
        const badge = document.createElement("span");
        badge.className = `badge ${item.severity}`;
        badge.textContent = item.title;
        attachTooltip(badge, item.tip);
        list.appendChild(badge);
      }
      return list;
    }

    function emptyText(value) {
      const item = document.createElement("span");
      item.className = "empty-text";
      item.textContent = value;
      return item;
    }

    function eventTypesDisclosure(eventCounts) {
      const details = document.createElement("details");
      details.className = "event-types-disclosure";

      const summary = document.createElement("summary");
      const title = document.createElement("span");
      title.textContent = "Event Types";
      const count = document.createElement("span");
      count.className = "count";
      count.textContent = `${eventCounts.length} types`;
      summary.append(title, count);

      details.appendChild(summary);
      details.appendChild(chipsBlock("", eventCounts.map(([name, count]) => `${name}: ${count}`)));
      return details;
    }

    function renderRequestMessages(request) {
      if (!request.parsed) {
        bodyEl.append(textBlock("Request body is not JSON."));
        return;
      }

      const messages = request.messages;
      const start = Math.max(0, messages.length - MAX_REQUEST_MESSAGES);
      if (start > 0) {
        bodyEl.append(notice(`Showing latest ${messages.length - start} of ${messages.length} input items.`));
      }
      for (const message of messages.slice(start)) {
        const card = document.createElement("div");
        card.className = "message-card";
        const details = document.createElement("details");
        details.open = message.index > messages.length - 3;
        const summary = document.createElement("summary");
        const name = document.createElement("span");
        name.className = "summary-name";
        name.textContent = `#${message.index} ${message.role} ${message.kind}`;
        const size = document.createElement("span");
        size.className = "count";
        size.textContent = formatChars(message.chars);
        summary.append(name, size);
        details.appendChild(summary);
        lazyDetails(details, () => textBlock(message.text || "(empty)"));
        card.appendChild(details);
        bodyEl.appendChild(card);
      }
      if (messages.length === 0) bodyEl.append(textBlock("(empty)"));
    }

    function renderToolIo(request, response) {
      if (!request.parsed) {
        bodyEl.append(textBlock("Request body is not JSON."));
        return;
      }

      const responseActions = response.items.filter((item) => item.type.includes("function_call"));
      bodyEl.append(notice(`${request.toolCalls.length} request tool calls, ${request.toolOutputs.length} tool outputs, ${responseActions.length} response tool actions.`));
      bodyEl.append(
        panel("Largest Tool Outputs", toolOutputRankBlock(request.toolOutputs, 12, true)),
        panel("Request Tool Calls", toolCallList(request.toolCalls)),
        panel("Response Tool Actions", responseToolActionList(responseActions))
      );
    }

    function toolCallList(calls) {
      const list = document.createElement("div");
      list.className = "tool-list";
      for (const call of calls) {
        const row = document.createElement("div");
        row.className = "tool-row";
        const title = document.createElement("div");
        title.className = "tool-title";
        const name = document.createElement("span");
        name.textContent = `#${call.index} ${call.name}`;
        const callId = document.createElement("span");
        callId.className = "count";
        callId.textContent = call.callId || "no call_id";
        title.append(name, callId);
        const summary = document.createElement("div");
        summary.className = "tool-summary";
        summary.textContent = call.summary || "(empty)";
        row.append(title, summary);
        list.appendChild(row);
      }
      if (calls.length === 0) {
        list.appendChild(emptyText("No request tool calls"));
      }
      return list;
    }

    function responseToolActionList(actions) {
      const list = document.createElement("div");
      list.className = "tool-list";
      for (const action of actions) {
        const row = document.createElement("div");
        row.className = "tool-row";
        const title = document.createElement("div");
        title.className = "tool-title";
        const name = document.createElement("span");
        name.textContent = action.name || action.type;
        const status = document.createElement("span");
        status.className = "count";
        status.textContent = action.status || action.type;
        title.append(name, status);
        const summary = document.createElement("div");
        summary.className = "tool-summary";
        summary.textContent = summarizeToolCall(action.name || action.type, parseMaybeJson(action.text), action.text);
        row.append(title, summary);
        list.appendChild(row);
      }
      if (actions.length === 0) {
        list.appendChild(emptyText("No response tool actions"));
      }
      return list;
    }

    function renderJsonOrText(parsed, raw, label) {
      if (parsed.ok) {
        bodyEl.append(jsonTree(parsed.value, label, true));
      } else {
        bodyEl.append(textBlock(raw));
      }
    }

    function renderSseEvents(events) {
      if (events.length === 0) {
        bodyEl.append(textBlock("(empty)"));
        return;
      }
      const limit = Math.min(state.eventLimit, events.length);
      bodyEl.append(notice(`Showing ${limit} of ${events.length} events. Event data is rendered when opened.`));
      for (const [index, event] of events.slice(0, limit).entries()) {
        bodyEl.appendChild(eventCard(event, index));
      }
      if (limit < events.length) {
        const button = document.createElement("button");
        button.type = "button";
        button.className = "load-more";
        button.textContent = `Show next ${Math.min(DEFAULT_EVENT_LIMIT, events.length - limit)} events`;
        button.addEventListener("click", () => {
          state.eventLimit += DEFAULT_EVENT_LIMIT;
          renderDetail();
        });
        bodyEl.appendChild(button);
      }
    }

    function eventCard(event, index) {
      const card = document.createElement("div");
      card.className = "event-card";
      const details = document.createElement("details");
      const summary = document.createElement("summary");
      const name = document.createElement("span");
      name.className = "summary-name";
      name.textContent = `${index + 1}. ${event.event || "message"}`;
      const size = document.createElement("span");
      size.className = "count";
      size.textContent = formatChars(event.data.length);
      summary.append(name, size);
      details.appendChild(summary);
      lazyDetails(details, () => event.parsed === undefined ? textBlock(event.data) : jsonTree(event.parsed, "data", false));
      card.appendChild(details);
      return card;
    }

    function jsonTree(value, label, root) {
      if (!isContainer(value)) return primitiveNode(value, label, root);

      const details = document.createElement("details");
      details.open = root;
      if (root) details.className = "root";
      const summary = document.createElement("summary");
      summary.append(labelSpan(label), typeChip(value), countChip(value));
      details.appendChild(summary);

      const entries = Array.isArray(value) ? value.entries() : Object.entries(value);
      for (const [key, child] of entries) {
        details.appendChild(jsonTree(child, String(key), false));
      }
      return details;
    }

    function primitiveNode(value, label, root) {
      if (typeof value === "string" && value.length > LONG_STRING_LIMIT) {
        return longStringNode(label, value, root);
      }
      const row = document.createElement("div");
      row.className = root ? "" : "leaf";
      row.append(labelSpan(label), primitiveSpan(value));
      return row;
    }

    function longStringNode(label, value, root) {
      const details = document.createElement("details");
      details.className = root ? "root long-string" : "long-string";
      const summary = document.createElement("summary");
      const name = document.createElement("span");
      name.className = "summary-name";
      name.append(labelSpan(label), document.createTextNode(` ${preview(value, 120)}`));
      const size = document.createElement("span");
      size.className = "count";
      size.textContent = formatChars(value.length);
      summary.append(name, size);
      details.appendChild(summary);
      lazyDetails(details, () => textBlock(value));
      return details;
    }

    function lazyDetails(details, build) {
      details.addEventListener("toggle", () => {
        if (!details.open || details.dataset.rendered) return;
        details.dataset.rendered = "true";
        details.appendChild(build());
      });
      if (details.open && !details.dataset.rendered) {
        details.dataset.rendered = "true";
        details.appendChild(build());
      }
    }

    function panel(title, ...children) {
      const item = document.createElement("div");
      item.className = "panel";
      const heading = document.createElement("h3");
      heading.textContent = title;
      item.appendChild(heading);
      item.append(...children.filter(Boolean));
      return item;
    }

    function metric(label, value, tip, severity) {
      return { label, value, tip, severity };
    }

    function metrics(values) {
      const grid = document.createElement("div");
      grid.className = "metrics";
      for (const value of values) {
        if (!value) continue;
        const item = Array.isArray(value) ? metric(value[0], value[1]) : value;
        const metricEl = document.createElement("div");
        metricEl.className = `metric ${item.severity || ""}`.trim();
        const label = document.createElement("span");
        label.textContent = item.label;
        const strong = document.createElement("strong");
        strong.textContent = String(item.value);
        if (item.tip) attachTooltip(metricEl, item.tip);
        metricEl.append(label, strong);
        grid.appendChild(metricEl);
      }
      return grid;
    }

    function chipsDisclosure(title, summaryText, values) {
      const details = document.createElement("details");
      details.className = "chips-disclosure";
      const summary = document.createElement("summary");
      const label = document.createElement("span");
      label.textContent = title;
      const count = document.createElement("span");
      count.className = "count";
      count.textContent = summaryText;
      summary.append(label, count);
      details.appendChild(summary);
      details.appendChild(chipsBlock("", values));
      return details;
    }

    function chipsBlock(title, values) {
      const fragment = document.createDocumentFragment();
      if (title) {
        const label = document.createElement("div");
        label.className = "chips-title";
        label.textContent = title;
        fragment.appendChild(label);
      }
      const chips = document.createElement("div");
      chips.className = "chips";
      const visible = values.filter(Boolean).slice(0, 32);
      for (const value of visible) {
        const chip = document.createElement("span");
        chip.className = "chip";
        chip.textContent = value;
        chips.appendChild(chip);
      }
      if (values.length > visible.length) {
        const more = document.createElement("span");
        more.className = "chip";
        more.textContent = `+${values.length - visible.length} more`;
        chips.appendChild(more);
      }
      if (visible.length === 0) {
        const empty = document.createElement("span");
        empty.className = "chip";
        empty.textContent = "(empty)";
        chips.appendChild(empty);
      }
      fragment.appendChild(chips);
      return fragment;
    }

    function previewBlock(title, value) {
      const container = document.createElement("div");
      container.className = "preview-block";
      const label = document.createElement("div");
      label.className = "preview-title";
      label.textContent = title;
      const block = document.createElement("pre");
      block.className = "preview";
      block.textContent = preview(value, PREVIEW_LIMIT);
      container.append(label, block);
      return container;
    }

    function outputItemsBlock(items) {
      const values = items.map((item) => {
        const name = [item.type, item.name, item.status].filter(Boolean).join(" ");
        return name || "item";
      });
      return chipsBlock("Output items", values);
    }

    function notice(value) {
      const item = document.createElement("p");
      item.className = "notice";
      item.textContent = value;
      return item;
    }

    function truncationNotice(label, originalBytes) {
      return notice(`${label} stored in SQLite was truncated. Original body size: ${formatBytes(originalBytes)}. Upstream traffic was not truncated.`);
    }

    function storedBodyStatus(truncated) {
      return truncated ? "truncated" : "complete";
    }

    function labelSpan(label) {
      const span = document.createElement("span");
      span.className = "key";
      span.textContent = label;
      return span;
    }

    function primitiveSpan(value) {
      const span = document.createElement("span");
      const type = value === null ? "null" : typeof value;
      span.className = type;
      span.textContent = type === "string" ? JSON.stringify(value) : String(value);
      return span;
    }

    function typeChip(value) {
      const span = document.createElement("span");
      span.className = "type-chip";
      span.textContent = Array.isArray(value) ? "array" : "object";
      return span;
    }

    function countChip(value) {
      const span = document.createElement("span");
      span.className = "count";
      span.textContent = `${Array.isArray(value) ? value.length : Object.keys(value).length} items`;
      return span;
    }

    function textBlock(value) {
      const pre = document.createElement("pre");
      pre.className = "text-block";
      pre.textContent = value;
      return pre;
    }

    function parseJson(value) {
      try {
        return { ok: true, value: JSON.parse(value) };
      } catch (error) {
        return { ok: false, error };
      }
    }

    function parseSse(raw) {
      return raw.split(/\n\n+/).map((block) => {
        const lines = block.split(/\n/);
        const event = lines.find((line) => line.startsWith("event:"))?.slice(6).trim() || "";
        const data = lines
          .filter((line) => line.startsWith("data:"))
          .map((line) => line.slice(5).trimStart())
          .join("\n");
        if (!data) return null;
        try {
          return { event, data, parsed: JSON.parse(data) };
        } catch {
          return { event, data };
        }
      }).filter(Boolean);
    }

    function extractResponseText(events) {
      let deltaText = "";
      let completedText = "";
      for (const event of events) {
        const parsed = event.parsed;
        if (!parsed) continue;
        if (event.event.includes("output_text.delta") && typeof parsed.delta === "string") {
          deltaText += parsed.delta;
        } else if (event.event.includes("output_text.done") && typeof parsed.text === "string") {
          completedText = parsed.text;
        }
        if (Array.isArray(parsed.response?.output)) {
          const outputText = parsed.response.output
            .filter((item) => item?.type === "message")
            .map(outputItemText)
            .filter(Boolean)
            .join("\n");
          if (outputText) completedText = outputText;
        }
      }
      return deltaText || completedText;
    }

    function contentText(content) {
      if (typeof content === "string") return content;
      if (Array.isArray(content)) return content.map(contentPartText).filter(Boolean).join("\n");
      return contentPartText(content);
    }

    function contentPartText(part) {
      if (typeof part === "string") return part;
      if (!part || typeof part !== "object") return "";
      return firstText([part.text, part.input_text, part.output_text, part.content, part.summary]);
    }

    function firstText(values) {
      for (const value of values) {
        if (typeof value === "string" && value.length > 0) return value;
        if (value && typeof value === "object") return JSON.stringify(value, null, 2);
      }
      return "";
    }

    function preview(value, limit) {
      const text = String(value ?? "");
      if (text.length <= limit) return text;
      return `${text.slice(0, limit)}\n... truncated ${text.length - limit} chars`;
    }

    function formatBytes(value) {
      if (value == null) return "-";
      const units = ["B", "KB", "MB", "GB"];
      let size = Number(value);
      let unit = 0;
      while (size >= 1024 && unit < units.length - 1) {
        size /= 1024;
        unit += 1;
      }
      return `${unit === 0 ? size : size.toFixed(1)} ${units[unit]}`;
    }

    function formatChars(value) {
      if (value == null) return "-";
      return `${Number(value).toLocaleString()} chars`;
    }

    function formatTimestamp(value) {
      if (!value) return "-";
      const seconds = Number(value);
      if (!Number.isFinite(seconds)) return String(value);
      const date = new Date(seconds * 1000);
      const pad = (part) => String(part).padStart(2, "0");
      return [
        date.getFullYear(),
        pad(date.getMonth() + 1),
        pad(date.getDate()),
      ].join("-") + " " + [
        pad(date.getHours()),
        pad(date.getMinutes()),
        pad(date.getSeconds()),
      ].join(":");
    }

    function formatDurationMs(value) {
      if (value == null) return "-";
      const ms = Number(value);
      if (!Number.isFinite(ms)) return String(value);
      if (ms < 1000) return `${ms} ms`;
      if (ms < 60000) return `${(ms / 1000).toFixed(1)} s`;
      const minutes = Math.floor(ms / 60000);
      const seconds = ((ms % 60000) / 1000).toFixed(1);
      return `${minutes}m ${seconds}s`;
    }

    function formatCount(value) {
      if (value == null) return "-";
      return Number(value).toLocaleString();
    }

    function isContainer(value) {
      return value !== null && typeof value === "object";
    }

    function isErrorStatus(status) {
      return typeof status === "number" && status >= 400;
    }

    async function fetchJson(url) {
      const response = await fetch(url);
      if (!response.ok) {
        throw new Error(`${response.status} ${response.statusText}`);
      }
      return response.json();
    }

    loadRequests().catch((error) => {
      statusEl.textContent = "Error";
      bodyEl.replaceChildren(textBlock(error.stack || String(error)));
    });
"#;
