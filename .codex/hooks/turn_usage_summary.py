#!/usr/bin/env python3
import json
import sys
from pathlib import Path


BASELINE_TOKENS = 12_000


def n(value):
    try:
        return int(value)
    except (TypeError, ValueError):
        return 0


def fmt(value):
    return f"{n(value):,}"


def blended_total(usage):
    cached = max(n(usage.get("cached_input_tokens")), 0)
    input_tokens = max(n(usage.get("input_tokens")) - cached, 0)
    output_tokens = max(n(usage.get("output_tokens")), 0)
    return input_tokens + output_tokens


def latest_token_counts(transcript_path):
    if not transcript_path:
        return None, None
    path = Path(transcript_path)
    if not path.is_file():
        return None, None

    previous = None
    latest = None
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            try:
                item = json.loads(line)
            except json.JSONDecodeError:
                continue
            if item.get("type") != "event_msg":
                continue
            payload = item.get("payload") or {}
            if payload.get("type") != "token_count":
                continue
            info = payload.get("info")
            if info:
                previous = latest
                latest = payload
    return previous, latest


def quiet():
    print(json.dumps({"continue": True, "suppressOutput": True}))


def percent(value):
    if 0 < value < 0.1:
        return "<0.1%"
    if value < 10:
        return f"{value:.1f}%"
    return f"{round(value)}%"


def used_percent(rate_limits, key):
    if not isinstance(rate_limits, dict):
        return None
    window = rate_limits.get(key)
    if not isinstance(window, dict):
        return None
    try:
        return float(window.get("used_percent"))
    except (TypeError, ValueError):
        return None


def quota_turn_delta(previous_rate_limits, current_rate_limits):
    parts = []
    for label, key in (("5h", "primary"), ("1w", "secondary")):
        previous_used = used_percent(previous_rate_limits, key)
        current_used = used_percent(current_rate_limits, key)
        if previous_used is None or current_used is None:
            continue
        delta = current_used - previous_used
        if delta <= 0:
            continue
        parts.append(f"{label} +{percent(delta)}")

    if not parts:
        return None
    return ", ".join(parts)


def main():
    try:
        hook_input = json.load(sys.stdin)
    except json.JSONDecodeError:
        quiet()
        return

    previous_token_count, token_count = latest_token_counts(hook_input.get("transcript_path"))
    info = (token_count or {}).get("info")
    if not info:
        quiet()
        return

    last = info.get("last_token_usage") or {}
    context_window = info.get("model_context_window")
    turn_tokens = blended_total(last)

    parts = [f"Usage: {fmt(turn_tokens)} tok"]

    if context_window:
        turn_percent = (turn_tokens / n(context_window)) * 100
        parts.append(f"ctx +{percent(turn_percent)}")

    current_rate_limits = (token_count or {}).get("rate_limits")
    previous_rate_limits = (previous_token_count or {}).get("rate_limits")
    quota_delta = quota_turn_delta(previous_rate_limits, current_rate_limits)
    if quota_delta:
        parts.append(f"quota {quota_delta}")

    print(json.dumps({"continue": True, "systemMessage": "; ".join(parts)}))


if __name__ == "__main__":
    main()
