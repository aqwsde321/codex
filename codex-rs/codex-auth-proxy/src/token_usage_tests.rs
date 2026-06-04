use pretty_assertions::assert_eq;

use super::*;

#[test]
fn reads_response_usage_from_completed_sse_event() {
    let body = br#"event: response.completed
data: {"type":"response.completed","response":{"usage":{"input_tokens":120,"output_tokens":30,"total_tokens":150,"input_tokens_details":{"cached_tokens":80},"output_tokens_details":{"reasoning_tokens":12}}}}

"#;

    assert_eq!(
        token_usage_from_response_body(body),
        TokenUsage {
            input_tokens: Some(120),
            output_tokens: Some(30),
            total_tokens: Some(150),
            cached_input_tokens: Some(80),
            reasoning_output_tokens: Some(12),
        }
    );
}

#[test]
fn ignores_non_token_usage_objects() {
    let body = br#"event: response.created
data: {"response":{"usage":{"image_gen":{"input_tokens":0},"web_search":{"num_requests":0}}}}

"#;

    assert_eq!(token_usage_from_response_body(body), TokenUsage::default());
}

#[test]
fn keeps_latest_token_usage_event() {
    let body = br#"event: response.in_progress
data: {"response":{"usage":{"input_tokens":10,"output_tokens":1,"total_tokens":11}}}

event: response.completed
data: {"response":{"usage":{"input_tokens":10,"output_tokens":4,"total_tokens":14}}}

"#;

    assert_eq!(
        token_usage_from_response_body(body),
        TokenUsage {
            input_tokens: Some(10),
            output_tokens: Some(4),
            total_tokens: Some(14),
            cached_input_tokens: None,
            reasoning_output_tokens: None,
        }
    );
}
