use std::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use novel_application::{Clock, FaultInjector, Ids};
use novel_llm::{LlmProvider, LlmRequest, LlmResponse, ProviderCapabilities, TokenUsage};
use tokio_util::sync::CancellationToken;

pub struct FakeClock {
    now: AtomicI64,
}

impl FakeClock {
    pub fn new(now: i64) -> Self {
        Self {
            now: AtomicI64::new(now),
        }
    }

    pub fn advance(&self, millis: i64) {
        self.now.fetch_add(millis, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now_ms(&self) -> i64 {
        self.now.load(Ordering::SeqCst)
    }
}

pub struct SequentialIds {
    next: AtomicU64,
}

impl SequentialIds {
    pub fn new() -> Self {
        Self {
            next: AtomicU64::new(1),
        }
    }
}

impl Default for SequentialIds {
    fn default() -> Self {
        Self::new()
    }
}

impl Ids for SequentialIds {
    fn new_id(&self) -> String {
        format!("id-{}", self.next.fetch_add(1, Ordering::SeqCst))
    }
}

pub struct MockLlm {
    calls: AtomicUsize,
    text: String,
    usage: TokenUsage,
}

impl MockLlm {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            text: text.into(),
            usage: TokenUsage::unknown(),
        }
    }

    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl LlmProvider for MockLlm {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            structured_output: false,
            tool_call: false,
            reasoning: false,
            context_limit: 8_000,
        }
    }

    async fn generate(
        &self,
        _request: LlmRequest,
        tokens: tokio::sync::mpsc::UnboundedSender<String>,
        cancel: CancellationToken,
    ) -> Result<LlmResponse, novel_llm::LlmError> {
        if cancel.is_cancelled() {
            return Err(novel_llm::LlmError::Cancelled);
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        let _ = tokens.send(self.text.clone());
        Ok(LlmResponse {
            text: self.text.clone(),
            usage: self.usage.clone(),
        })
    }
}

pub struct BlockingMockLlm {
    calls: AtomicUsize,
    text: String,
}

impl BlockingMockLlm {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            text: text.into(),
        }
    }

    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl LlmProvider for BlockingMockLlm {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            structured_output: false,
            tool_call: false,
            reasoning: false,
            context_limit: 8_000,
        }
    }

    async fn generate(
        &self,
        _request: LlmRequest,
        tokens: tokio::sync::mpsc::UnboundedSender<String>,
        cancel: CancellationToken,
    ) -> Result<LlmResponse, novel_llm::LlmError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        tokio::select! {
            _ = cancel.cancelled() => Err(novel_llm::LlmError::Cancelled),
            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                let _ = tokens.send(self.text.clone());
                Ok(LlmResponse {
                    text: self.text.clone(),
                    usage: TokenUsage::unknown(),
                })
            }
        }
    }
}

pub struct FailDraftAfterCandidate;

#[async_trait]
impl FaultInjector for FailDraftAfterCandidate {
    async fn fail_after_candidate(&self, step_id: &str) -> bool {
        step_id == "draft"
    }
}
