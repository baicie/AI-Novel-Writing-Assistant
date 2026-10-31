//! R6-F：只读协议（Anthropic 等）不得绑定到生成任务，后端必须以真实 SQLite 拒绝。

use std::sync::Arc;

use async_trait::async_trait;
use novel_application::{
    CredentialAction, CredentialVault, PortError, ProviderDraft, ProviderErrorCode,
    ProviderRegistry, ProviderService,
};
use novel_domain::ProviderProtocol;
use novel_storage_sqlite::SqliteKernel;

struct TestVault;

#[async_trait]
impl CredentialVault for TestVault {
    async fn write(&self, _reference: &str, _secret: &str) -> Result<(), PortError> {
        Ok(())
    }
    async fn delete(&self, _reference: &str) -> Result<(), PortError> {
        Ok(())
    }
    async fn present(&self, _reference: &str) -> Result<bool, PortError> {
        Ok(true)
    }
}

#[tokio::test]
async fn read_only_protocol_cannot_be_bound_to_generation_task() {
    let directory = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&directory.path().join("protocol.sqlite"))
            .await
            .unwrap(),
    );
    let vault = Arc::new(TestVault);
    let service = ProviderService::new(
        kernel.clone() as Arc<dyn ProviderRegistry>,
        vault as Arc<dyn CredentialVault>,
        kernel.clone() as Arc<dyn novel_application::CredentialIntents>,
    );

    // 只读协议：可以保存、可以发现模型，但不能用于生成。
    service
        .upsert_provider(
            ProviderDraft {
                id: "gw-anthropic".into(),
                display_name: Some("只读网关".into()),
                protocol: ProviderProtocol::AnthropicMessages,
                base_url: "https://api.anthropic.com".into(),
                allow_localhost_http: false,
                is_builtin: false,
                sort_order: 0,
            },
            CredentialAction::Replace("key-readonly".into()),
            None,
            1,
        )
        .await
        .unwrap();
    service
        .upsert_model(
            novel_domain::ModelProfile {
                provider_id: "gw-anthropic".into(),
                model_id: "claude-readonly".into(),
                display_name: Some("claude-readonly".into()),
                capabilities: Default::default(),
                source: novel_domain::ModelSource::User,
                overridden: Default::default(),
            },
            false,
            None,
            1,
        )
        .await
        .unwrap();

    // 绑定必须被拒绝：这是「绕过 UI 直接调 IPC」时后端要守住的那道门。
    let error = service
        .upsert_binding(
            novel_domain::ModelBinding {
                task_type: "writer".into(),
                provider_id: "gw-anthropic".into(),
                model_id: "claude-readonly".into(),
                reasoning_effort: None,
            },
            None,
            1,
        )
        .await
        .expect_err("只读协议不允许绑定生成任务");
    assert_eq!(error.code, ProviderErrorCode::NotSupported, "{error:?}");
    assert!(
        error.message.contains("只能读取模型列表"),
        "提示要说明原因：{}",
        error.message
    );

    // 数据库里不能留下任何绑定（通过端口读取，不依赖 sqlx）。
    let stored = service.list_bindings().await.unwrap();
    assert!(stored.is_empty(), "被拒绝的绑定不能落库：{stored:?}");

    // 只读协议也不允许被放进 RunContract 的环节路由。
    let routes = service.frozen_step_routes().await.unwrap();
    assert!(routes.is_empty(), "没有绑定时不应冻结出任何环节路由");
}
