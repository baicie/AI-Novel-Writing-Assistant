//! P16 端到端：真实 SQLite + 真实发现适配器（本地 mock 服务）走完
//! 建来源 → 取模型 → 确认 → 绑定 → 解析路由，全程不联网、不调用付费模型。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use novel_application::{
    CredentialAction, CredentialVault, PortError, ProviderDraft, ProviderService,
};
use novel_domain::{ModelBinding, ProviderProtocol};
use novel_llm::{DiscoveryRequest, OpenAiCompatibleDiscovery, ProviderModelDiscovery};
use novel_storage_sqlite::SqliteKernel;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct MemoryVault {
    values: Mutex<std::collections::HashMap<String, String>>,
}

#[async_trait]
impl CredentialVault for MemoryVault {
    async fn write(&self, reference: &str, secret: &str) -> Result<(), PortError> {
        self.values
            .lock()
            .unwrap()
            .insert(reference.to_string(), secret.to_string());
        Ok(())
    }

    async fn delete(&self, reference: &str) -> Result<(), PortError> {
        self.values.lock().unwrap().remove(reference);
        Ok(())
    }

    async fn present(&self, reference: &str) -> Result<bool, PortError> {
        Ok(self.values.lock().unwrap().contains_key(reference))
    }
}

async fn mock_models(body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buffer = vec![0u8; 4096];
            let _ = socket.read(&mut buffer).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = socket.write_all(response.as_bytes()).await;
        }
    });
    format!("http://127.0.0.1:{port}/v1")
}

#[tokio::test]
async fn provider_flow_create_discover_apply_bind_resolve() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&dir.path().join("novel.sqlite"))
            .await
            .unwrap(),
    );
    let vault = Arc::new(MemoryVault::default());
    let service = ProviderService::new(kernel.clone(), vault.clone(), kernel.clone());

    // 1. 建来源（带密钥，走意图表 + 凭据端口）
    let created = service
        .upsert_provider(
            ProviderDraft {
                id: "gw".into(),
                display_name: Some("本地网关".into()),
                protocol: ProviderProtocol::OpenAiCompatible,
                base_url: "http://127.0.0.1:1/v1".into(),
                allow_localhost_http: true,
                sort_order: 0,
                is_builtin: false,
            },
            CredentialAction::Replace("sk-test".into()),
            None,
            1,
        )
        .await
        .unwrap();
    assert!(created.credential_present);
    assert_eq!(created.revision, 1);

    // 2. 取模型（真实适配器打本地 mock 服务）
    let base =
        mock_models(r#"{"data":[{"id":"model-a","context_window":32000},{"id":"model-b"}]}"#).await;
    let outcome = OpenAiCompatibleDiscovery
        .discover(
            &DiscoveryRequest::new(base, "sk-test", true).with_timeout(Duration::from_millis(500)),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(outcome.models.len(), 2);

    // 3. 确认候选（只追加，CAS 用当前版本）
    let candidates: Vec<novel_domain::ModelProfile> = outcome
        .models
        .into_iter()
        .map(|model| novel_domain::ModelProfile {
            provider_id: "gw".into(),
            model_id: model.id,
            display_name: model.display_name,
            capabilities: novel_domain::ModelCapabilities {
                context_window: model.context_window,
                ..Default::default()
            },
            source: novel_domain::ModelSource::Discovered,
            overridden: Default::default(),
        })
        .collect();
    let applied = kernel
        .apply_discovered_models("gw", &candidates, &["model-a".to_string()], Some(1), 2)
        .await
        .unwrap();
    assert_eq!(applied.added, 1);
    assert_eq!(applied.revision, 2);
    assert_eq!(
        kernel.list_models("gw").await.unwrap().len(),
        1,
        "只加入被选中的模型"
    );

    // 4. 绑定并解析路由
    service
        .upsert_binding(
            ModelBinding {
                task_type: "writer".into(),
                provider_id: "gw".into(),
                model_id: "model-a".into(),
                reasoning_effort: None,
            },
            None,
            3,
        )
        .await
        .unwrap();
    let route = service.resolve_route("writer").await.unwrap().unwrap();
    assert_eq!(route.model_id, "model-a");
    assert_eq!(route.endpoint, "http://127.0.0.1:1/v1");
    assert_eq!(route.credential_ref, "gw#1", "密钥槽按版本命名");
    assert!(
        !serde_json::to_string(&route).unwrap().contains("sk-test"),
        "路由不含密钥"
    );

    // 5. 冻结路由校验 + 新库重开后仍在
    assert!(service.validate_frozen_route("gw", "model-a").await.is_ok());
    kernel.pool().close().await;
    let reopened = SqliteKernel::open(&dir.path().join("novel.sqlite"))
        .await
        .unwrap();
    assert_eq!(
        reopened.list_models("gw").await.unwrap().len(),
        1,
        "重启后配置还在"
    );
    assert_eq!(reopened.list_model_bindings().await.unwrap().len(), 1);
    reopened.pool().close().await;
}
