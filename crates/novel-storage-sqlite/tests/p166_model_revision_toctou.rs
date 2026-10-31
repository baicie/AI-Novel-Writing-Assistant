//! P16.6 复审：模型编辑的 revision 并发窗口，以及自动发现不得清空用户覆盖。

use std::sync::Arc;

use novel_application::ProviderRegistry;
use novel_domain::{CapabilityField, ModelProfile, ModelSource, ProviderProtocol};
use novel_storage_sqlite::SqliteKernel;

async fn kernel() -> Arc<SqliteKernel> {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("revision.sqlite");
    // 目录必须活到测试结束：把它泄漏给进程（测试进程结束即回收）。
    std::mem::forget(directory);
    Arc::new(SqliteKernel::open(&path).await.unwrap())
}

fn model(overridden: Vec<CapabilityField>) -> ModelProfile {
    ModelProfile {
        provider_id: "gw".into(),
        model_id: "model".into(),
        display_name: Some("模型".into()),
        capabilities: Default::default(),
        source: ModelSource::User,
        overridden: overridden.into_iter().collect(),
    }
}

async fn seed(kernel: &Arc<SqliteKernel>) {
    kernel
        .upsert_provider_cas(
            &novel_domain::ProviderProfile {
                id: "gw".into(),
                display_name: Some("网关".into()),
                protocol: ProviderProtocol::OpenAiCompatible,
                base_url: "https://gw.example.com/v1".into(),
                credential: novel_domain::CredentialReference {
                    kind: novel_domain::CredentialKind::OsSecret,
                    reference: "gw#1".into(),
                },
                is_builtin: false,
                allow_localhost_http: false,
                sort_order: 0,
            },
            None,
            1,
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn stale_revision_cannot_write_model_after_concurrent_bump() {
    let kernel = kernel().await;
    seed(&kernel).await;
    let adapter: Arc<dyn ProviderRegistry> = kernel.clone();

    // 窗口 A 读到 revision = 1。
    let observed = kernel.provider_revision("gw").await.unwrap().unwrap();
    assert_eq!(observed, 1);

    // 窗口 B 抢先保存，版本前进到 2。
    kernel
        .upsert_provider_cas(
            &novel_domain::ProviderProfile {
                id: "gw".into(),
                display_name: Some("网关改".into()),
                protocol: ProviderProtocol::OpenAiCompatible,
                base_url: "https://gw.example.com/v1".into(),
                credential: novel_domain::CredentialReference {
                    kind: novel_domain::CredentialKind::OsSecret,
                    reference: "gw#2".into(),
                },
                is_builtin: false,
                allow_localhost_http: false,
                sort_order: 0,
            },
            Some(1),
            2,
        )
        .await
        .unwrap();

    // 窗口 A 仍用旧版本号写模型：必须被拒绝。
    let error = adapter
        .upsert_model(&model(vec![]), true, Some(observed), 3)
        .await
        .expect_err("旧版本号不得再写入模型");
    assert_eq!(
        error,
        novel_application::PortError::Conflict(
            "配置已被修改（期望版本 1，当前 2），请刷新后再试".into()
        )
    );

    // 模型没有落库。
    let models = kernel.list_models("gw").await.unwrap();
    assert!(models.is_empty(), "被拒绝的写入不能留下模型：{models:?}");
}

#[tokio::test]
async fn discovery_upsert_never_clears_user_capability_overrides() {
    let kernel = kernel().await;
    seed(&kernel).await;

    // 用户显式覆盖了上下文窗口。
    kernel
        .upsert_model(&model(vec![CapabilityField::ContextWindow]), true, 2)
        .await
        .unwrap();
    let stored = kernel.list_models("gw").await.unwrap();
    assert_eq!(stored.len(), 1);
    assert!(
        !stored[0].overridden.is_empty(),
        "用户覆盖必须被记录：{stored:?}"
    );

    // 自动发现再次写入同一模型（user_edit = false），不得清空用户覆盖。
    kernel.upsert_model(&model(vec![]), false, 3).await.unwrap();
    let after = kernel.list_models("gw").await.unwrap();
    assert!(
        !after[0].overridden.is_empty(),
        "发现结果不能清空用户覆盖：{after:?}"
    );

    // 用户主动清空覆盖时，覆盖列表才应该变空。
    kernel.upsert_model(&model(vec![]), true, 4).await.unwrap();
    let cleared = kernel.list_models("gw").await.unwrap();
    assert!(
        cleared[0].overridden.is_empty(),
        "用户主动清空应当生效：{cleared:?}"
    );
}
