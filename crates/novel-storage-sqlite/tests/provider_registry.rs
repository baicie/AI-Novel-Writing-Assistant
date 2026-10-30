//! P16.1：Provider / 模型注册表的真实 SQLite 集成测试。

use std::collections::BTreeSet;

use novel_domain::{
    CapabilityField, CredentialKind, CredentialReference, Modality, ModelBinding,
    ModelCapabilities, ModelProfile, ModelSource, ProviderProfile, ProviderProtocol,
};
use novel_storage_sqlite::SqliteKernel;
use tempfile::tempdir;

fn provider(id: &str, base_url: &str) -> ProviderProfile {
    ProviderProfile {
        id: id.into(),
        display_name: Some(format!("{id} 网关")),
        protocol: ProviderProtocol::OpenAiCompatible,
        base_url: base_url.into(),
        credential: CredentialReference {
            kind: CredentialKind::OsSecret,
            reference: id.into(),
        },
        is_builtin: false,
        allow_localhost_http: false,
        sort_order: 1,
    }
}

fn model(provider_id: &str, model_id: &str) -> ModelProfile {
    ModelProfile {
        provider_id: provider_id.into(),
        model_id: model_id.into(),
        display_name: Some(model_id.into()),
        capabilities: ModelCapabilities {
            context_window: Some(128_000),
            max_output_tokens: Some(8_000),
            supports_tools: Some(true),
            input_modalities: Some(vec![Modality::Text, Modality::Image]),
            reasoning_efforts: Some(BTreeSet::from(["low".to_string(), "high".to_string()])),
            ..Default::default()
        },
        source: ModelSource::Discovered,
        overridden: BTreeSet::new(),
    }
}

#[tokio::test]
async fn provider_registry_roundtrip() {
    let dir = tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("novel.sqlite"))
        .await
        .unwrap();

    kernel
        .upsert_provider(&provider("deepseek", "https://api.deepseek.com/v1"), 1)
        .await
        .unwrap();
    let providers = kernel.list_providers().await.unwrap();
    assert_eq!(providers.len(), 1, "应当只有一条 provider 记录");
    assert_eq!(providers[0].id, "deepseek");
    assert_eq!(providers[0].protocol, ProviderProtocol::OpenAiCompatible);
    assert_eq!(
        providers[0].credential.reference, "deepseek",
        "凭据只存引用"
    );

    kernel
        .upsert_provider(&provider("gw", "https://gw.example.com/v1"), 2)
        .await
        .unwrap();
    let ids: Vec<String> = kernel
        .list_providers()
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.id)
        .collect();
    assert_eq!(ids, vec!["deepseek".to_string(), "gw".to_string()]);
    kernel.pool().close().await;
}

#[tokio::test]
async fn models_keep_user_overrides() {
    let dir = tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("novel.sqlite"))
        .await
        .unwrap();
    kernel
        .upsert_provider(&provider("deepseek", "https://api.deepseek.com/v1"), 1)
        .await
        .unwrap();

    // 用户显式改了上下文并标记为覆盖
    let mut user_model = model("deepseek", "deepseek-chat");
    user_model.source = ModelSource::User;
    user_model.capabilities.context_window = Some(64_000);
    user_model.overridden = BTreeSet::from([CapabilityField::ContextWindow]);
    kernel.upsert_model(&user_model, true, 2).await.unwrap();

    let stored = kernel.list_models("deepseek").await.unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].capabilities.context_window, Some(64_000));
    assert!(
        stored[0]
            .overridden
            .contains(&CapabilityField::ContextWindow)
    );
    assert_eq!(
        stored[0]
            .capabilities
            .reasoning_efforts
            .as_ref()
            .unwrap()
            .len(),
        2,
        "JSON 列需要能往返"
    );
    kernel.pool().close().await;
}

#[tokio::test]
async fn deleting_provider_cascades_models_and_bindings() {
    let dir = tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("novel.sqlite"))
        .await
        .unwrap();
    kernel
        .upsert_provider(&provider("deepseek", "https://api.deepseek.com/v1"), 1)
        .await
        .unwrap();
    kernel
        .upsert_model(&model("deepseek", "deepseek-chat"), false, 2)
        .await
        .unwrap();
    kernel
        .set_model_binding(
            &ModelBinding {
                task_type: "chapter".into(),
                provider_id: "deepseek".into(),
                model_id: "deepseek-chat".into(),
                reasoning_effort: Some("high".into()),
            },
            3,
        )
        .await
        .unwrap();

    kernel
        .delete_provider_guarded("deepseek", true)
        .await
        .unwrap();
    assert!(kernel.list_providers().await.unwrap().is_empty());
    assert!(kernel.list_models("deepseek").await.unwrap().is_empty());
    assert!(kernel.list_model_bindings().await.unwrap().is_empty());
    kernel.pool().close().await;
}

#[tokio::test]
async fn bindings_are_upserted_per_task() {
    let dir = tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("novel.sqlite"))
        .await
        .unwrap();
    kernel
        .upsert_provider(&provider("deepseek", "https://api.deepseek.com/v1"), 1)
        .await
        .unwrap();
    let mut binding = ModelBinding {
        task_type: "chapter".into(),
        provider_id: "deepseek".into(),
        model_id: "deepseek-chat".into(),
        reasoning_effort: None,
    };
    kernel.set_model_binding(&binding, 2).await.unwrap();
    binding.model_id = "deepseek-reasoner".into();
    binding.reasoning_effort = Some("high".into());
    kernel.set_model_binding(&binding, 3).await.unwrap();

    let bindings = kernel.list_model_bindings().await.unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].model_id, "deepseek-reasoner");
    assert_eq!(bindings[0].reasoning_effort.as_deref(), Some("high"));
    kernel.pool().close().await;
}

/// 老版本写入的 `default` 行（只有老字段）必须仍能读，且新列取默认值。
/// 真实的「旧 schema 升级」路径由 `p151_historical_migration`（test-harness）覆盖。
#[tokio::test]
async fn legacy_shaped_default_row_stays_readable() {
    let dir = tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("novel.sqlite"))
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO provider_credential(id, provider, secret_ref, endpoint, model, allow_localhost_http, created_at)
         VALUES ('default','openai-compatible','openai-compatible','https://old.example.com/v1','old-model',0,1)",
    )
    .execute(kernel.pool())
    .await
    .unwrap();

    let legacy = kernel
        .load_model_setup()
        .await
        .unwrap()
        .expect("老配置要还能读");
    assert_eq!(legacy.endpoint, "https://old.example.com/v1");
    assert_eq!(legacy.model, "old-model");

    let providers = kernel.list_providers().await.unwrap();
    let default = providers
        .iter()
        .find(|item| item.id == "default")
        .expect("default 行仍在");
    assert_eq!(
        default.protocol,
        ProviderProtocol::OpenAiCompatible,
        "新列取默认值"
    );
    assert!(!default.is_builtin);
    kernel.pool().close().await;
}

#[tokio::test]
async fn migrations_are_idempotent() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("novel.sqlite");
    {
        let first = SqliteKernel::open(&path).await.unwrap();
        first.pool().close().await;
    }
    let second = SqliteKernel::open(&path).await.unwrap();
    let providers = second.list_providers().await.unwrap();
    assert!(providers.is_empty());
    second.pool().close().await;
}
fn candidate(provider_id: &str, model_id: &str, context: Option<u32>) -> ModelProfile {
    let mut profile = model(provider_id, model_id);
    profile.capabilities.context_window = context;
    profile.source = ModelSource::Discovered;
    profile
}

#[tokio::test]
async fn apply_adds_new_models_without_touching_others() {
    let dir = tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("novel.sqlite"))
        .await
        .unwrap();
    kernel
        .upsert_provider(&provider("gw", "https://gw.example.com/v1"), 1)
        .await
        .unwrap();
    kernel
        .upsert_model(&model("gw", "a"), false, 2)
        .await
        .unwrap();

    let outcome = kernel
        .apply_discovered_models(
            "gw",
            &[
                candidate("gw", "a", Some(1_000)),
                candidate("gw", "b", Some(2_000)),
            ],
            &["a".to_string(), "b".to_string()],
            Some(0),
            3,
        )
        .await
        .unwrap();
    assert_eq!(outcome.added, 1);
    assert_eq!(outcome.updated, 1);
    assert_eq!(outcome.revision, 1);

    let models = kernel.list_models("gw").await.unwrap();
    let ids: Vec<&str> = models.iter().map(|item| item.model_id.as_str()).collect();
    assert_eq!(ids, vec!["a", "b"], "未选中的模型不能被删");
    kernel.pool().close().await;
}

#[tokio::test]
async fn apply_with_empty_selection_is_a_noop() {
    let dir = tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("novel.sqlite"))
        .await
        .unwrap();
    kernel
        .upsert_provider(&provider("gw", "https://gw.example.com/v1"), 1)
        .await
        .unwrap();
    kernel
        .upsert_model(&model("gw", "a"), false, 2)
        .await
        .unwrap();

    let outcome = kernel
        .apply_discovered_models("gw", &[], &[], None, 3)
        .await
        .unwrap();
    assert_eq!(outcome.added, 0);
    assert_eq!(outcome.updated, 0);
    assert_eq!(
        kernel.list_models("gw").await.unwrap().len(),
        1,
        "空选择绝不能清空模型"
    );
    kernel.pool().close().await;
}

#[tokio::test]
async fn apply_keeps_user_overrides() {
    let dir = tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("novel.sqlite"))
        .await
        .unwrap();
    kernel
        .upsert_provider(&provider("gw", "https://gw.example.com/v1"), 1)
        .await
        .unwrap();
    let mut user_model = model("gw", "a");
    user_model.source = ModelSource::User;
    user_model.capabilities.context_window = Some(64_000);
    user_model.overridden = BTreeSet::from([CapabilityField::ContextWindow]);
    kernel.upsert_model(&user_model, true, 2).await.unwrap();

    let outcome = kernel
        .apply_discovered_models(
            "gw",
            &[candidate("gw", "a", Some(1_000_000))],
            &["a".to_string()],
            None,
            3,
        )
        .await
        .unwrap();
    assert_eq!(outcome.preserved, 1);

    let models = kernel.list_models("gw").await.unwrap();
    assert_eq!(
        models[0].capabilities.context_window,
        Some(64_000),
        "用户改过的上下文不能被远端覆盖"
    );
    assert_eq!(
        models[0].capabilities.supports_tools,
        Some(true),
        "未覆盖字段可以吸收远端值"
    );
    kernel.pool().close().await;
}

#[tokio::test]
async fn stale_discovery_is_rejected() {
    let dir = tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("novel.sqlite"))
        .await
        .unwrap();
    kernel
        .upsert_provider(&provider("gw", "https://gw.example.com/v1"), 1)
        .await
        .unwrap();
    kernel
        .apply_discovered_models(
            "gw",
            &[candidate("gw", "a", None)],
            &["a".to_string()],
            None,
            2,
        )
        .await
        .unwrap();

    let error = kernel
        .apply_discovered_models(
            "gw",
            &[candidate("gw", "b", None)],
            &["b".to_string()],
            Some(0),
            3,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(error, novel_application::PortError::Conflict(_)),
        "旧版本号必须冲突"
    );
    let ids: Vec<String> = kernel
        .list_models("gw")
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.model_id)
        .collect();
    assert_eq!(ids, vec!["a".to_string()], "冲突时不能写入任何东西");
    kernel.pool().close().await;
}

#[tokio::test]
async fn delete_is_blocked_while_bindings_exist() {
    let dir = tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("novel.sqlite"))
        .await
        .unwrap();
    kernel
        .upsert_provider(&provider("gw", "https://gw.example.com/v1"), 1)
        .await
        .unwrap();
    kernel
        .upsert_model(&model("gw", "a"), false, 2)
        .await
        .unwrap();
    kernel
        .set_model_binding(
            &ModelBinding {
                task_type: "chapter".into(),
                provider_id: "gw".into(),
                model_id: "a".into(),
                reasoning_effort: None,
            },
            3,
        )
        .await
        .unwrap();

    let error = kernel
        .delete_provider_guarded("gw", false)
        .await
        .unwrap_err();
    assert!(matches!(error, novel_application::PortError::Conflict(_)));
    assert_eq!(
        kernel.list_providers().await.unwrap().len(),
        1,
        "被引用时不能删"
    );

    kernel.delete_provider_guarded("gw", true).await.unwrap();
    assert!(kernel.list_providers().await.unwrap().is_empty());
    kernel.pool().close().await;
}

#[tokio::test]
async fn default_provider_delete_needs_confirmation() {
    let dir = tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("novel.sqlite"))
        .await
        .unwrap();
    kernel
        .save_model_setup("https://api.deepseek.com/v1", "deepseek-chat", false, 1)
        .await
        .unwrap();
    let error = kernel
        .delete_provider_guarded("default", false)
        .await
        .unwrap_err();
    assert!(
        matches!(error, novel_application::PortError::Conflict(_)),
        "默认配置需要有意识确认"
    );
    kernel.pool().close().await;
}
/// 硬门 1：旧接口写默认配置也要 bump revision，否则 Registry 的 CAS 挡不住旧写入。
#[tokio::test]
async fn legacy_save_bumps_provider_revision() {
    let dir = tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("novel.sqlite"))
        .await
        .unwrap();
    kernel
        .save_model_setup("https://api.deepseek.com/v1", "deepseek-chat", false, 1)
        .await
        .unwrap();
    assert_eq!(kernel.provider_revision("default").await.unwrap(), Some(1));
    kernel
        .save_model_setup("https://api.deepseek.com/v1", "deepseek-reasoner", false, 2)
        .await
        .unwrap();
    assert_eq!(
        kernel.provider_revision("default").await.unwrap(),
        Some(2),
        "旧接口每次写入都要 bump"
    );
    kernel.pool().close().await;
}
/// 凭据意图：写入 → 待补偿 → 完成后不再出现在待办里。
#[tokio::test]
async fn credential_intents_track_until_done() {
    let dir = tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("novel.sqlite"))
        .await
        .unwrap();
    kernel
        .insert_credential_intent("gw-replace-1", "gw", "gw", "replace", 1)
        .await
        .unwrap();
    let pending = kernel.pending_credential_intents().await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].action, "replace");
    assert_eq!(pending[0].state, "pending");

    kernel
        .finish_credential_intent("gw-replace-1", "done", None, 2)
        .await
        .unwrap();
    assert!(
        kernel
            .pending_credential_intents()
            .await
            .unwrap()
            .is_empty()
    );
    kernel.pool().close().await;
}
/// 多窗口并发：两个客户端拿同一版本号提交，只有一个能成功。
#[tokio::test]
async fn concurrent_updates_with_same_revision_conflict() {
    let dir = tempdir().unwrap();
    let kernel = std::sync::Arc::new(
        SqliteKernel::open(&dir.path().join("novel.sqlite"))
            .await
            .unwrap(),
    );
    kernel
        .upsert_provider_cas(&provider("gw", "https://gw.example.com/v1"), None, 1)
        .await
        .unwrap();
    let revision = kernel.provider_revision("gw").await.unwrap().unwrap();
    assert_eq!(revision, 1);

    let a = {
        let kernel = kernel.clone();
        tokio::spawn(async move {
            kernel
                .upsert_provider_cas(&provider("gw", "https://a.example.com/v1"), Some(1), 2)
                .await
        })
    };
    let b = {
        let kernel = kernel.clone();
        tokio::spawn(async move {
            kernel
                .upsert_provider_cas(&provider("gw", "https://b.example.com/v1"), Some(1), 2)
                .await
        })
    };
    let (a, b) = (a.await.unwrap(), b.await.unwrap());
    let successes = [&a, &b].iter().filter(|result| result.is_ok()).count();
    let conflicts = [&a, &b]
        .iter()
        .filter(|result| matches!(result, Err(novel_application::PortError::Conflict(_))))
        .count();
    assert_eq!(successes, 1, "只能有一个写入成功");
    assert_eq!(conflicts, 1, "另一个必须收到冲突而不是静默覆盖");
    assert_eq!(kernel.provider_revision("gw").await.unwrap(), Some(2));
    kernel.pool().close().await;
}
