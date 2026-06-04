use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub(crate) struct TokenUsage {
    pub(crate) input_tokens: Option<i64>,
    pub(crate) output_tokens: Option<i64>,
    pub(crate) total_tokens: Option<i64>,
    pub(crate) cached_input_tokens: Option<i64>,
    pub(crate) reasoning_output_tokens: Option<i64>,
}

impl TokenUsage {
    pub(crate) fn is_empty(self) -> bool {
        self.input_tokens.is_none()
            && self.output_tokens.is_none()
            && self.total_tokens.is_none()
            && self.cached_input_tokens.is_none()
            && self.reasoning_output_tokens.is_none()
    }
}

pub(crate) fn token_usage_from_response_body(body: &[u8]) -> TokenUsage {
    let body = String::from_utf8_lossy(body);
    let mut latest = TokenUsage::default();
    for data in sse_data_blocks(&body) {
        let Ok(value) = serde_json::from_str::<Value>(&data) else {
            continue;
        };
        if let Some(usage) = token_usage_from_event(&value) {
            latest = usage;
        }
    }
    latest
}

fn sse_data_blocks(body: &str) -> impl Iterator<Item = String> + '_ {
    body.split("\n\n").filter_map(|block| {
        let data = block
            .lines()
            .filter_map(|line| {
                line.trim_end_matches('\r')
                    .strip_prefix("data:")
                    .map(str::trim_start)
            })
            .collect::<Vec<_>>()
            .join("\n");
        if data.is_empty() { None } else { Some(data) }
    })
}

fn token_usage_from_event(value: &Value) -> Option<TokenUsage> {
    [
        value
            .get("response")
            .and_then(|response| response.get("usage")),
        value.get("usage"),
    ]
    .into_iter()
    .flatten()
    .filter_map(token_usage_from_value)
    .find(|usage| !usage.is_empty())
}

fn token_usage_from_value(value: &Value) -> Option<TokenUsage> {
    let usage = TokenUsage {
        input_tokens: integer_field(value, "input_tokens"),
        output_tokens: integer_field(value, "output_tokens"),
        total_tokens: integer_field(value, "total_tokens"),
        cached_input_tokens: integer_pointer(value, &["input_tokens_details", "cached_tokens"]),
        reasoning_output_tokens: integer_pointer(
            value,
            &["output_tokens_details", "reasoning_tokens"],
        ),
    };
    if usage.is_empty() { None } else { Some(usage) }
}

fn integer_field(value: &Value, field: &str) -> Option<i64> {
    integer_value(value.get(field)?)
}

fn integer_pointer(value: &Value, path: &[&str]) -> Option<i64> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    integer_value(current)
}

fn integer_value(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
}

#[cfg(test)]
#[path = "token_usage_tests.rs"]
mod tests;
