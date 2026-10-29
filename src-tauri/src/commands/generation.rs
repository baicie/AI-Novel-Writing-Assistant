use novel_application::AppError;
use novel_llm::{LlmError, LlmRequest, TokenUsage};
use novel_platform::SecretStore;
use serde::{Deserialize, Serialize};
use tauri::State;
use tauri::ipc::Channel;
use tokio_util::sync::CancellationToken;

use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateTextRequest {
    pub request_id: String,
    pub prompt: String,
    pub max_output_tokens: Option<u64>,
    pub model: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextStreamEvent {
    pub kind: &'static str,
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub retryable: bool,
}

#[tauri::command]
pub async fn generate_text(
    state: State<'_, AppState>,
    request: GenerateTextRequest,
    channel: Channel<TextStreamEvent>,
) -> Result<(), AppError> {
    let request_id = request.request_id.trim().to_string();
    if request_id.is_empty() {
        return Err(AppError::validation("这次生成缺少请求编号。"));
    }
    if request.prompt.trim().is_empty() {
        return Err(AppError::validation("生成内容不能为空。"));
    }
    let setup = state
        .kernel
        .load_model_setup()
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::validation("还没有配置模型服务。"))?;
    let api_key = state
        .secrets
        .get(&setup.secret_ref)
        .await
        .map_err(|_| {
            AppError::from(novel_application::PortError::Storage(
                "系统凭据暂时不可用".into(),
            ))
        })?
        .unwrap_or_default();
    if api_key.trim().is_empty() {
        return Err(AppError::validation("还没有填写模型密钥。"));
    }
    let cancel = CancellationToken::new();
    {
        let mut cancellations = state
            .cancellations
            .lock()
            .map_err(|_| AppError::integrity("生成请求状态不可用。"))?;
        if cancellations.contains_key(&request_id) {
            return Err(AppError::conflict("这次生成已经在进行中。"));
        }
        cancellations.insert(request_id.clone(), cancel.clone());
    }
    let llm = state.llm.clone();
    let model = request
        .model
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(setup.model);
    let (sender, mut receiver) = tokio::sync::mpsc::channel(64);
    let event_request_id = request_id.clone();
    let forward_channel = channel.clone();
    let forward = tokio::spawn(async move {
        while let Some(text) = receiver.recv().await {
            let _ = forward_channel.send(TextStreamEvent {
                kind: "delta",
                request_id: event_request_id.clone(),
                text: Some(text),
                usage: None,
                code: None,
                message: None,
                retryable: false,
            });
        }
    });
    let result = llm
        .generate(
            LlmRequest {
                request_id: request_id.clone(),
                model,
                prompt: request.prompt,
                endpoint: setup.endpoint,
                api_key,
                allow_localhost_http: setup.allow_localhost_http,
                max_output_tokens: request.max_output_tokens,
            },
            sender,
            cancel,
        )
        .await;
    let _ = forward.await;
    if let Ok(mut cancellations) = state.cancellations.lock() {
        cancellations.remove(&request_id);
    }
    match result {
        Ok(response) => {
            let _ = channel.send(TextStreamEvent {
                kind: "done",
                request_id,
                text: Some(response.text),
                usage: Some(response.usage),
                code: None,
                message: None,
                retryable: false,
            });
            Ok(())
        }
        Err(error) => {
            let (code, message, retryable) = classify_llm_error(&error);
            let _ = channel.send(TextStreamEvent {
                kind: "error",
                request_id,
                text: None,
                usage: None,
                code: Some(code),
                message: Some(message),
                retryable,
            });
            Ok(())
        }
    }
}

#[tauri::command]
pub fn cancel_text_generation(state: State<'_, AppState>, request_id: String) {
    if let Ok(mut cancellations) = state.cancellations.lock()
        && let Some(token) = cancellations.remove(&request_id)
    {
        token.cancel();
    }
}

fn classify_llm_error(error: &LlmError) -> (String, String, bool) {
    match error {
        LlmError::RateLimit => ("MODEL_RATE_LIMIT".into(), error.to_string(), true),
        LlmError::Timeout => ("MODEL_TIMEOUT".into(), error.to_string(), true),
        LlmError::Cancelled => ("CANCELLED".into(), error.to_string(), false),
        LlmError::Unauthorized => ("MODEL_UNAUTHORIZED".into(), error.to_string(), false),
        LlmError::Unavailable { .. } => (
            "MODEL_UNAVAILABLE".into(),
            error.to_string(),
            error.retryable_without_new_charge(),
        ),
        LlmError::InvalidResponse(_)
        | LlmError::StructuredOutput(_)
        | LlmError::EndpointRejected(_) => ("INVALID_RESPONSE".into(), error.to_string(), false),
        LlmError::BudgetExceeded(_) => ("BUDGET_EXCEEDED".into(), error.to_string(), false),
    }
}
