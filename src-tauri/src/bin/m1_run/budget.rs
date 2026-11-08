//! M1 调用预算与费用门禁：请求前占用额度，未知计费不推断零成本。

use std::sync::{Arc, Mutex};

use novel_llm::{LlmError, LlmProvider, LlmRequest, LlmResponse, ProviderCapabilities, TokenUsage};

#[derive(Default)]
pub(crate) struct BudgetState {
    requests_started: u64,
    unknown_usage: u64,
}

/// 请求级调用预算：在真正发出 Provider 请求之前占用一次额度。结构化修复与正文生成
/// 走同一个 Provider 入口，因此分别占用一次预算；被拒绝的请求不会到达 Provider。
pub(crate) struct CallBudget {
    max_calls: u64,
    state: Mutex<BudgetState>,
}

impl CallBudget {
    pub(crate) fn new(max_calls: u64) -> Arc<Self> {
        Arc::new(Self {
            max_calls,
            state: Mutex::new(BudgetState::default()),
        })
    }

    pub(crate) fn try_start(&self) -> Result<(), LlmError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| LlmError::BudgetExceeded("M1 调用预算状态不可用".into()))?;
        if state.requests_started >= self.max_calls {
            return Err(LlmError::BudgetExceeded(format!(
                "M1 调用预算已用尽（{}/{} 次请求），请求未发出",
                state.requests_started, self.max_calls
            )));
        }
        state.requests_started += 1;
        Ok(())
    }

    fn record_unknown_usage(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.unknown_usage += 1;
        }
    }

    fn record_usage(&self, usage: &TokenUsage) {
        if usage.prompt_tokens.is_none() || usage.completion_tokens.is_none() {
            self.record_unknown_usage();
        }
    }

    pub(crate) fn snapshot(&self) -> (u64, u64) {
        self.state
            .lock()
            .map(|state| (state.requests_started, state.unknown_usage))
            .unwrap_or((0, 0))
    }
}

/// 包裹真实 Provider：请求前检查预算，响应后记录 usage；未知 usage 或失败（可能已
/// 计费）都只标记不确定性，不推断零成本。
pub(crate) struct BudgetedLlm {
    inner: Arc<dyn LlmProvider>,
    budget: Arc<CallBudget>,
}

impl BudgetedLlm {
    pub(crate) fn new(inner: Arc<dyn LlmProvider>, budget: Arc<CallBudget>) -> Self {
        Self { inner, budget }
    }
}

#[async_trait::async_trait]
impl LlmProvider for BudgetedLlm {
    fn capabilities(&self) -> ProviderCapabilities {
        self.inner.capabilities()
    }

    async fn generate(
        &self,
        request: LlmRequest,
        tokens: tokio::sync::mpsc::Sender<String>,
        cancel: tokio_util::sync::CancellationToken,
    ) -> Result<LlmResponse, LlmError> {
        self.generate_with_protocol(request, "openai-compatible", None, tokens, cancel)
            .await
    }

    async fn generate_with_protocol(
        &self,
        request: LlmRequest,
        protocol: &str,
        reasoning_effort: Option<&str>,
        tokens: tokio::sync::mpsc::Sender<String>,
        cancel: tokio_util::sync::CancellationToken,
    ) -> Result<LlmResponse, LlmError> {
        self.budget.try_start()?;
        let result = self
            .inner
            .generate_with_protocol(request, protocol, reasoning_effort, tokens, cancel)
            .await;
        match &result {
            Ok(response) => self.budget.record_usage(&response.usage),
            Err(_) => self.budget.record_unknown_usage(),
        }
        result
    }
}

/// 费用门禁：本地没有可信定价与请求前预留，无法强制执行人民币上限。真实模式必须由
/// 供应商侧硬额度兜底，并显式确认；否则拒绝运行，不用“看起来安全”的本地上限冒充。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CostGuard {
    DryRun,
    ProviderSide,
}

pub(crate) fn resolve_cost_guard(
    dry_run: bool,
    explicit: Option<&str>,
) -> Result<CostGuard, String> {
    if dry_run {
        return Ok(CostGuard::DryRun);
    }
    match explicit {
        Some("provider_side") => Ok(CostGuard::ProviderSide),
        _ => Err(
            "本地无法强制执行人民币费用上限：缺少可信定价与请求前预留，M1_MAX_COST_CNY 只是记录值。真实 M1 必须先在供应商侧设置硬额度，并显式设置 M1_COST_GUARD=provider_side；本次拒绝运行。"
                .into(),
        ),
    }
}
