use crate::viewer_analysis_script;
use crate::viewer_script;

const HEAD: &str = r#"<!doctype html>
<html lang="ko">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>codex-auth-proxy</title>
  <style>
"#;

const STYLE: &str = r#"
    :root {
      color-scheme: light;
      --bg: #f6f7f9;
      --panel: #ffffff;
      --line: #d9dee7;
      --text: #172033;
      --muted: #687386;
      --accent: #0f766e;
      --danger: #b42318;
      --danger-bg: #fff1f0;
      --warn: #9a6700;
      --warn-bg: #fff8c5;
      --info: #0969da;
      --info-bg: #ddf4ff;
      --code: #111827;
      --code-bg: #f1f5f9;
      --chip: #eef2f7;
    }

    * { box-sizing: border-box; }

    body {
      margin: 0;
      background: var(--bg);
      color: var(--text);
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      font-size: 14px;
    }

    header {
      height: 52px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 16px;
      padding: 0 18px;
      border-bottom: 1px solid var(--line);
      background: var(--panel);
    }

    h1 {
      margin: 0;
      font-size: 15px;
      font-weight: 650;
    }

    button {
      border: 1px solid var(--line);
      background: var(--panel);
      color: var(--text);
      height: 32px;
      padding: 0 10px;
      border-radius: 6px;
      font: inherit;
      cursor: pointer;
    }

    button:hover { border-color: #aeb7c6; }
    button.active { border-color: var(--accent); color: var(--accent); }

    .toolbar {
      display: flex;
      align-items: center;
      gap: 10px;
      min-width: 0;
    }

    .status {
      color: var(--muted);
      white-space: nowrap;
    }

    .db-stats {
      max-width: 52vw;
      overflow: hidden;
      color: var(--muted);
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    main {
      display: grid;
      grid-template-columns: minmax(260px, 340px) minmax(0, 1fr);
      height: calc(100vh - 52px);
      min-height: 0;
    }

    aside {
      border-right: 1px solid var(--line);
      background: var(--panel);
      min-width: 0;
      min-height: 0;
      overflow: hidden;
      display: flex;
      flex-direction: column;
    }

    .filters {
      flex: 0 0 auto;
      padding: 10px;
      border-bottom: 1px solid var(--line);
    }

    .search-input {
      width: 100%;
      height: 32px;
      margin-bottom: 8px;
      padding: 0 9px;
      border: 1px solid var(--line);
      border-radius: 6px;
      background: #fff;
      color: var(--text);
      font: inherit;
    }

    .search-input:focus {
      outline: 2px solid #b7ddd8;
      outline-offset: 0;
      border-color: var(--accent);
    }

    .quick-filters {
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
    }

    .filter-button {
      height: 26px;
      padding: 0 8px;
      font-size: 12px;
    }

    .list {
      flex: 1 1 auto;
      min-height: 0;
      overflow: auto;
    }

    .flow-step-badge {
      flex: 0 0 auto;
      min-width: 30px;
      height: 20px;
      padding: 0 6px;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      border-radius: 999px;
      background: var(--chip);
      color: var(--muted);
      font-size: 11px;
      font-weight: 700;
    }

    .row.flow-step-row {
      position: relative;
      width: calc(100% - 16px);
      margin-left: 8px;
      margin-right: 8px;
      padding-left: 12px;
      border-left: 1px solid #b7ddd8;
      border-right: 1px solid #b7ddd8;
      border-bottom-color: #dfeeea;
      background: #fbfcfe;
    }

    .row.flow-step-row::before {
      content: "";
      position: absolute;
      top: 0;
      bottom: 0;
      left: 0;
      width: 3px;
      background: #79bfb5;
      pointer-events: none;
    }

    .row.flow-run-start {
      margin-top: 8px;
      border-top: 1px solid #b7ddd8;
      border-radius: 7px 7px 0 0;
    }

    .row.flow-run-end {
      margin-bottom: 8px;
      border-bottom: 1px solid #b7ddd8;
      border-radius: 0 0 7px 7px;
    }

    .row.flow-run-start.flow-run-end {
      border-radius: 7px;
    }

    .row.active .flow-step-badge {
      background: var(--accent);
      color: #fff;
    }

    .row {
      width: 100%;
      height: auto;
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 7px 10px;
      padding: 11px 13px;
      border: 0;
      border-bottom: 1px solid var(--line);
      border-radius: 0;
      text-align: left;
    }

    .row.active { background: #eef7f5; }
    .row-title, .row-sub { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .row-title {
      display: flex;
      align-items: center;
      gap: 6px;
      min-width: 0;
      font-weight: 600;
    }
    .row-title-text {
      min-width: 0;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .row-sub, .row-meta, .empty { color: var(--muted); font-size: 12px; }
    .row-meta {
      display: flex;
      flex-wrap: wrap;
      justify-content: flex-end;
      gap: 4px 6px;
      text-align: right;
    }

    .row-badges {
      display: inline-flex;
      flex-wrap: wrap;
      justify-content: flex-end;
      gap: 3px;
    }

    .row-badge {
      border-radius: 999px;
      padding: 1px 5px;
      border: 1px solid var(--line);
      background: var(--chip);
      color: var(--muted);
      font-size: 10px;
      font-weight: 700;
      line-height: 1.25;
    }

    .row-badge.critical {
      border-color: #ffb4ab;
      background: var(--danger-bg);
      color: var(--danger);
    }

    .row-badge.warning {
      border-color: #eac54f;
      background: var(--warn-bg);
      color: var(--warn);
    }

    .row-badge.info {
      border-color: #9dd8ff;
      background: var(--info-bg);
      color: var(--info);
    }

    .search-highlight {
      border-radius: 3px;
      background: #ffe58f;
      color: inherit;
      padding: 0 1px;
    }

    .status-code {
      font-size: 12px;
      color: var(--accent);
      font-weight: 650;
    }

    .status-code.error { color: var(--danger); }

    section {
      min-width: 0;
      min-height: 0;
      padding: 16px;
      display: flex;
      flex-direction: column;
      overflow: hidden;
    }

    .detail-head {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 8px 16px;
      margin-bottom: 12px;
    }

    .detail-title {
      margin: 0;
      font-size: 17px;
      line-height: 1.35;
      overflow-wrap: anywhere;
    }

    .meta {
      display: flex;
      flex-wrap: wrap;
      gap: 8px 14px;
      color: var(--muted);
      font-size: 12px;
      margin-top: 6px;
    }

    .tabs {
      display: flex;
      gap: 6px;
      margin-bottom: 10px;
      flex-wrap: wrap;
      flex: 0 0 auto;
    }

    .tab.secondary {
      height: 28px;
      color: var(--muted);
      font-size: 12px;
    }

    .tab.secondary.active {
      color: var(--accent);
    }

    .body-surface {
      flex: 1 1 auto;
      min-height: 0;
      overflow: auto;
      padding: 13px;
      border: 1px solid var(--line);
      border-radius: 6px;
      background: var(--code-bg);
      color: var(--code);
      font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
      font-size: 12px;
      line-height: 1.5;
    }

    .body-surface.summary-mode { overflow: auto; }

    .attention {
      margin-bottom: 12px;
      padding: 12px;
      border: 1px solid var(--line);
      border-radius: 6px;
      background: #fff;
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }

    .attention h3 {
      margin: 0 0 8px;
      font-size: 13px;
      font-weight: 700;
    }

    .attention-list {
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
    }

    .badge {
      border-radius: 999px;
      padding: 3px 8px;
      border: 1px solid var(--line);
      background: var(--chip);
      color: var(--muted);
      font-size: 12px;
      cursor: help;
    }

    .badge.critical {
      border-color: #ffb4ab;
      background: var(--danger-bg);
      color: var(--danger);
    }

    .badge.warning {
      border-color: #eac54f;
      background: var(--warn-bg);
      color: var(--warn);
    }

    .badge.info {
      border-color: #9dd8ff;
      background: var(--info-bg);
      color: var(--info);
    }

    .summary-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
      gap: 12px;
      flex: 0 0 auto;
      min-height: 0;
      grid-auto-rows: auto;
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }

    .summary-content {
      min-height: 100%;
      display: flex;
      flex-direction: column;
      gap: 12px;
    }

    .main-cause {
      display: grid;
      grid-template-columns: auto auto minmax(0, 1fr);
      gap: 8px;
      align-items: baseline;
      padding: 9px 11px;
      border: 1px solid #eac54f;
      border-radius: 6px;
      background: var(--warn-bg);
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }

    .main-cause.quiet {
      border-color: var(--line);
      background: #fff;
    }

    .main-cause-label {
      color: var(--muted);
      font-size: 11px;
      font-weight: 700;
      text-transform: uppercase;
      white-space: nowrap;
    }

    .main-cause strong {
      font-size: 13px;
      white-space: nowrap;
    }

    .main-cause-text {
      min-width: 0;
      overflow: hidden;
      color: var(--text);
      font-size: 12px;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .search-match {
      display: grid;
      grid-template-columns: auto auto minmax(0, 1fr);
      gap: 8px;
      align-items: baseline;
      padding: 8px 11px;
      border: 1px solid #9dd8ff;
      border-radius: 6px;
      background: var(--info-bg);
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }

    .search-match-label {
      color: var(--muted);
      font-size: 11px;
      font-weight: 700;
      text-transform: uppercase;
      white-space: nowrap;
    }

    .search-match strong {
      color: var(--info);
      font-size: 13px;
      white-space: nowrap;
    }

    .search-match-snippet {
      min-width: 0;
      overflow: hidden;
      color: var(--text);
      font-size: 12px;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .key-info {
      flex: 0 0 auto;
      min-width: 0;
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }

    .key-info h3 {
      margin: 0 0 8px;
      font-size: 13px;
      font-weight: 700;
    }

    .key-info-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(210px, 1fr));
      gap: 8px;
    }

    .key-item {
      min-width: 0;
      min-height: 112px;
      display: flex;
      flex-direction: column;
      padding: 10px;
      border: 1px solid var(--line);
      border-radius: 6px;
      background: #fff;
    }

    .key-title {
      margin-bottom: 6px;
      color: var(--muted);
      font-size: 11px;
      font-weight: 700;
      text-transform: uppercase;
    }

    .key-text {
      min-height: 0;
      max-height: 98px;
      flex: 1 1 auto;
      margin: 0;
      overflow-x: hidden;
      overflow-y: auto;
      white-space: pre-wrap;
      overflow-wrap: anywhere;
      word-break: break-word;
      color: var(--text);
      font: 12px/1.45 ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
    }

    .action-list {
      min-height: 0;
      max-height: 112px;
      overflow-x: hidden;
      overflow-y: auto;
      display: flex;
      flex-direction: column;
      gap: 6px;
    }

    .action-row {
      min-width: 0;
      display: grid;
      grid-template-columns: auto minmax(0, 1fr);
      gap: 6px;
      align-items: baseline;
    }

    .action-name {
      border-radius: 999px;
      padding: 1px 7px;
      background: var(--chip);
      color: var(--muted);
      font-size: 11px;
      font-family: inherit;
      white-space: nowrap;
    }

    .action-summary {
      min-width: 0;
      white-space: pre-wrap;
      overflow-wrap: anywhere;
      word-break: break-word;
      color: var(--text);
      font: 12px/1.45 ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
    }

    .analysis-list, .tool-list {
      min-height: 0;
      overflow-x: hidden;
      overflow-y: auto;
      display: flex;
      flex-direction: column;
      gap: 8px;
    }

    .analysis-item, .tool-row {
      min-width: 0;
      padding: 8px;
      border-radius: 6px;
      background: #f8fafc;
    }

    .analysis-title, .tool-title {
      display: flex;
      align-items: baseline;
      justify-content: space-between;
      gap: 8px;
      margin-bottom: 4px;
      font-weight: 650;
    }

    .analysis-text, .tool-summary {
      white-space: pre-wrap;
      overflow-wrap: anywhere;
      word-break: break-word;
      color: var(--text);
      font: 12px/1.45 ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
    }

    .tool-row details {
      margin: 6px 0 0;
      border-left: 0;
      padding-left: 0;
    }

    .empty-text {
      color: var(--muted);
      font-size: 12px;
    }

    .panel, .message-card, .event-card {
      border: 1px solid var(--line);
      border-radius: 6px;
      background: #fff;
    }

    .panel { padding: 12px; }

    .summary-grid .panel {
      min-height: 0;
      display: flex;
      flex-direction: column;
      overflow: visible;
    }

    .summary-grid .analysis-list,
    .summary-grid .tool-list {
      flex: 1 1 auto;
      max-height: 260px;
    }

    .summary-grid .preview {
      max-height: 260px;
    }

    .body-surface > .panel {
      margin-bottom: 10px;
      color: var(--text);
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }

    .event-types-disclosure {
      flex: 0 0 auto;
      margin: 0;
      padding: 8px 10px;
      border: 1px solid var(--line);
      border-radius: 6px;
      background: #fff;
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }

    .event-types-disclosure summary, .chips-disclosure summary {
      justify-content: flex-start;
      color: var(--text);
      font-weight: 700;
    }

    .event-types-disclosure .chips, .chips-disclosure .chips {
      margin-top: 8px;
    }

    .chips-disclosure {
      margin-top: 8px;
    }

    .panel h3 {
      margin: 0 0 10px;
      font-size: 13px;
      font-weight: 700;
    }

    .metrics {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
      gap: 8px;
    }

    .metric {
      min-width: 0;
      padding: 8px;
      border-radius: 6px;
      background: #f8fafc;
    }

    .metric.critical {
      border: 1px solid #ffb4ab;
      background: var(--danger-bg);
    }

    .metric.warning {
      border: 1px solid #eac54f;
      background: var(--warn-bg);
    }

    .metric.info {
      border: 1px solid #9dd8ff;
      background: var(--info-bg);
    }

    .metric span {
      display: block;
      color: var(--muted);
      font-size: 11px;
      margin-bottom: 3px;
    }

    .metric [data-tip], .badge[data-tip] { cursor: help; }

    .viewer-tooltip {
      position: fixed;
      z-index: 1000;
      max-width: min(360px, calc(100vw - 24px));
      padding: 8px 10px;
      border-radius: 6px;
      background: #172033;
      color: #fff;
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      font-size: 12px;
      line-height: 1.4;
      white-space: pre-line;
      box-shadow: 0 8px 24px rgba(15, 23, 42, 0.22);
      pointer-events: none;
      opacity: 0;
      transform: translateY(4px);
      transition: opacity 80ms ease, transform 80ms ease;
    }

    .viewer-tooltip.visible {
      opacity: 1;
      transform: translateY(0);
    }

    .metric strong {
      display: block;
      overflow-wrap: anywhere;
      font-size: 13px;
    }

    .preview-title, .chips-title {
      color: var(--muted);
      font-size: 11px;
      margin: 10px 0 4px;
    }

    .preview-block {
      min-height: 0;
      display: flex;
      flex: 1 1 auto;
      flex-direction: column;
    }

    .chips {
      display: flex;
      flex-wrap: wrap;
      gap: 5px;
    }

    .chip, .count, .type-chip {
      color: var(--muted);
      background: var(--chip);
      border-radius: 999px;
      padding: 1px 7px;
      font-size: 11px;
      font-family: inherit;
    }

    .text-block {
      margin: 0;
      white-space: pre-wrap;
      overflow-wrap: anywhere;
      word-break: break-word;
      font: inherit;
    }

    .preview {
      min-height: 0;
      max-height: none;
      flex: 1 1 auto;
      overflow-x: hidden;
      overflow-y: auto;
      padding: 8px;
      border-radius: 6px;
      background: var(--code-bg);
      white-space: pre-wrap;
      overflow-wrap: anywhere;
      word-break: break-word;
      font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
      font-size: 12px;
    }

    .notice {
      color: var(--muted);
      margin: 0 0 10px;
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }

    details {
      margin: 2px 0 2px 14px;
      border-left: 1px solid #d5dce7;
      padding-left: 8px;
    }

    details.root {
      margin-left: 0;
      border-left: 0;
      padding-left: 0;
    }

    summary {
      cursor: pointer;
      user-select: none;
      min-height: 22px;
      display: flex;
      align-items: center;
      gap: 6px;
    }

    .key { color: #8a3ffc; }
    .string { color: #0f766e; }
    .number { color: #9a3412; }
    .boolean { color: #1d4ed8; }
    .null { color: #64748b; }

    .leaf {
      display: flex;
      gap: 8px;
      margin-left: 22px;
      min-height: 22px;
      align-items: baseline;
    }

    .message-card, .event-card {
      margin-bottom: 8px;
      padding: 7px 9px;
    }

    .message-card details, .event-card details, .long-string {
      margin-left: 0;
      border-left: 0;
      padding-left: 0;
    }

    .message-card summary, .event-card summary, .long-string summary {
      justify-content: space-between;
      color: var(--text);
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }

    .summary-name {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      font-weight: 650;
      min-width: 0;
    }

    .load-more {
      width: 100%;
      margin-top: 6px;
    }

    @media (max-width: 640px) {
      main {
        display: block;
        height: auto;
        min-height: calc(100vh - 52px);
      }
      aside { border-right: 0; border-bottom: 1px solid var(--line); }
      .filters { padding: 8px; }
      .list { height: 46vh; flex: 0 0 auto; }
      section {
        display: block;
        overflow: visible;
        padding: 12px;
      }
      .detail-head { grid-template-columns: 1fr; }
      .db-stats { display: none; }
      .body-surface { min-height: 320px; }
      .body-surface.summary-mode { overflow: auto; }
      .summary-content { min-height: auto; }
      .summary-grid { display: grid; }
      .main-cause { grid-template-columns: 1fr; }
      .main-cause strong, .main-cause-text { white-space: normal; }
      .search-match { grid-template-columns: 1fr; }
      .search-match strong, .search-match-snippet { white-space: normal; }
      .preview { min-height: 120px; max-height: 220px; }
    }
"#;

const BODY: &str = r#"
  </style>
</head>
<body>
  <header>
    <h1>codex-auth-proxy</h1>
    <div class="toolbar">
      <span id="db-stats" class="db-stats">DB loading</span>
      <span id="status" class="status">Loading</span>
      <button id="refresh" type="button">Refresh</button>
    </div>
  </header>
  <main>
    <aside>
      <div class="filters">
        <input id="search" class="search-input" type="search" autocomplete="off" placeholder="Search logs">
        <div class="quick-filters">
          <button class="filter-button active" type="button" data-filter="all">All</button>
          <button class="filter-button" type="button" data-filter="errors">Errors</button>
          <button class="filter-button" type="button" data-filter="slow">Slow</button>
          <button class="filter-button" type="button" data-filter="tokens">Tokens</button>
          <button class="filter-button" type="button" data-filter="truncated">Truncated</button>
        </div>
      </div>
      <div id="list" class="list"></div>
    </aside>
    <section>
      <div class="detail-head">
        <div>
          <h2 id="detail-title" class="detail-title">No request selected</h2>
          <div id="detail-meta" class="meta"></div>
        </div>
        <span id="detail-status" class="status-code"></span>
      </div>
      <div class="tabs">
        <button class="tab active" type="button" data-view="summary">Summary</button>
        <button class="tab" type="button" data-view="messages">Messages</button>
        <button class="tab" type="button" data-view="tools">Tool I/O</button>
        <button class="tab secondary" type="button" data-view="request">Request Tree</button>
        <button class="tab" type="button" data-view="text">Answer</button>
        <button class="tab secondary" type="button" data-view="events">Response Events</button>
        <button class="tab secondary" type="button" data-view="raw">Raw SSE</button>
        <button class="tab" type="button" data-view="error">Error</button>
      </div>
      <div id="body" class="body-surface"></div>
    </section>
  </main>
  <script>
"#;

const TAIL: &str = r#"
  </script>
</body>
</html>
"#;

pub(crate) fn html() -> String {
    let mut html = String::with_capacity(
        HEAD.len()
            + STYLE.len()
            + BODY.len()
            + viewer_analysis_script::JS.len()
            + viewer_script::JS.len()
            + TAIL.len(),
    );
    html.push_str(HEAD);
    html.push_str(STYLE);
    html.push_str(BODY);
    html.push_str(viewer_analysis_script::JS);
    html.push_str(viewer_script::JS);
    html.push_str(TAIL);
    html
}
