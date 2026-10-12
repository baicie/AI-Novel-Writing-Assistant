use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncBufReadExt};
use tokio_util::sync::CancellationToken;

use crate::error::LlmError;
use crate::provider::TokenUsage;

#[derive(Debug, Clone, PartialEq)]
pub enum SsePiece {
    Delta(String),
    Usage(TokenUsage),
    Done,
    Ignore,
}

pub fn parse_sse_line(line: &str) -> Result<SsePiece, LlmError> {
    let line = line.trim();
    if line.is_empty() || line.starts_with(':') {
        return Ok(SsePiece::Ignore);
    }
    let Some(data) = line.strip_prefix("data:") else {
        return Ok(SsePiece::Ignore);
    };
    let data = data.trim();
    if data == "[DONE]" {
        return Ok(SsePiece::Done);
    }
    let value: Value = serde_json::from_str(data)
        .map_err(|_| LlmError::InvalidResponse("模型流不是 JSON".into()))?;
    if let Some(usage) = value.get("usage").filter(|usage| !usage.is_null()) {
        return Ok(SsePiece::Usage(read_usage(usage)));
    }
    let content = value
        .pointer("/choices/0/delta/content")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if content.is_empty() {
        Ok(SsePiece::Ignore)
    } else {
        Ok(SsePiece::Delta(content.to_string()))
    }
}

fn read_usage(value: &Value) -> TokenUsage {
    TokenUsage {
        prompt_tokens: optional_u64(value, "prompt_tokens"),
        completion_tokens: optional_u64(value, "completion_tokens"),
    }
}

fn optional_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

pub async fn consume_sse<R>(
    reader: R,
    tokens: &tokio::sync::mpsc::UnboundedSender<String>,
    cancel: &CancellationToken,
) -> Result<(String, TokenUsage), LlmError>
where
    R: AsyncBufRead + Unpin,
{
    let mut lines = reader.lines();
    let mut text = String::new();
    let mut usage = TokenUsage::unknown();
    let mut started = false;
    loop {
        if cancel.is_cancelled() {
            return Err(LlmError::Cancelled);
        }
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(_) if started => {
                return Err(LlmError::Unavailable {
                    billed: true,
                    reason: "模型流在返回中途断开".into(),
                });
            }
            Err(_) => {
                return Err(LlmError::Unavailable {
                    billed: false,
                    reason: "模型流没有开始".into(),
                });
            }
        };
        match parse_sse_line(&line)? {
            SsePiece::Delta(delta) => {
                started = true;
                text.push_str(&delta);
                let _ = tokens.send(delta);
            }
            SsePiece::Usage(next) => {
                if next.prompt_tokens.is_some() {
                    usage.prompt_tokens = next.prompt_tokens;
                }
                if next.completion_tokens.is_some() {
                    usage.completion_tokens = next.completion_tokens;
                }
            }
            SsePiece::Done => break,
            SsePiece::Ignore => {}
        }
    }
    Ok((text, usage))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn missing_usage_stays_unknown() {
        let piece = parse_sse_line(r#"data: {"choices":[{"delta":{"content":"你"}}]}"#).unwrap();
        assert_eq!(piece, SsePiece::Delta("你".into()));
        let usage = parse_sse_line(r#"data: {"usage":{"prompt_tokens":3}}"#).unwrap();
        match usage {
            SsePiece::Usage(usage) => {
                assert_eq!(usage.prompt_tokens, Some(3));
                assert_eq!(usage.completion_tokens, None);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[tokio::test]
    async fn cancel_stops_before_reading_the_rest() {
        let payload = "data: {\"choices\":[{\"delta\":{\"content\":\"甲\"}}]}\n\ndata: [DONE]\n";
        let reader = BufReader::new(payload.as_bytes());
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let cancel = CancellationToken::new();
        cancel.cancel();
        let err = consume_sse(reader, &tx, &cancel).await.unwrap_err();
        assert_eq!(err, LlmError::Cancelled);
    }

    #[tokio::test]
    async fn parses_a_small_stream() {
        let payload = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"林\"}}]}\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"动\"}}]}\n",
            "data: {\"usage\":{\"completion_tokens\":2}}\n",
            "data: [DONE]\n"
        );
        let reader = BufReader::new(payload.as_bytes());
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let (text, usage) = consume_sse(reader, &tx, &CancellationToken::new())
            .await
            .unwrap();
        drop(tx);
        assert_eq!(text, "林动");
        assert_eq!(usage.prompt_tokens, None);
        assert_eq!(usage.completion_tokens, Some(2));
        assert_eq!(rx.recv().await.as_deref(), Some("林"));
    }
}
