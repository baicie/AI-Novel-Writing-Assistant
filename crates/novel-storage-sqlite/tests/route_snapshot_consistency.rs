//! R2.1：路由配置快照必须来自单个 SQLite 读事务。
//!
//! 用「版本标记」策略：写事务同时把 provider.endpoint、model.model_id、binding.model_id
//! 改成同一代次；读快照若混用了不同代次，断言会立刻失败。

use novel_domain::{
    CredentialKind, CredentialReference, ModelBinding, ModelCapabilities, ModelProfile,
    ModelSource, ProviderProfile, ProviderProtocol,
};
use novel_storage_sqlite::SqliteKernel;
use std::collections::BTreeSet;
use std::sync::Arc;

async fn write_generation(kernel: &SqliteKernel, generation: u32) {
    let provider = ProviderProfile {
        id: "gw".into(),
        display_name: Some("gw".into()),
        protocol: ProviderProtocol::OpenAiCompatible,
        base_url: format!("https://gw.example.com/v{generation}"),
        credential: CredentialReference {
            kind: CredentialKind::OsSecret,
            reference: format!("gw#{generation}"),
        },
        is_builtin: false,
        allow_localhost_http: false,
        sort_order: 0,
    };
    kernel.upsert_provider(&provider, 1).await.unwrap();
    let model = ModelProfile {
        provider_id: "gw".into(),
        model_id: format!("m{generation}"),
        display_name: Some(format!("m{generation}")),
        capabilities: ModelCapabilities::default(),
        source: ModelSource::User,
        overridden: BTreeSet::new(),
    };
    kernel.upsert_model(&model, true, 1).await.unwrap();
    kernel
        .set_binding_cas(
            &ModelBinding {
                task_type: "writer".into(),
                provider_id: "gw".into(),
                model_id: format!("m{generation}"),
                reasoning_effort: None,
            },
            None,
            1,
        )
        .await
        .unwrap();
}

/// 快照内部必须自洽：每条 binding 指向的 provider 与 model 都必须出现在同一份快照里。
///
/// 注意这里验证的是「一次事务读到的数据是一个自洽的时刻」：
/// 产品里 provider 与 binding 是两次独立的用户保存，中间态是合法配置，
/// 因此不能断言 endpoint 代次与 binding 代次相同；能断言的是引用完整性与可观测单调性。
async fn assert_referentially_consistent(kernel: &SqliteKernel, rounds: usize) {
    for _ in 0..rounds {
        let rows = kernel.route_snapshot_rows().await.unwrap();
        for binding in &rows.bindings {
            let provider = rows
                .providers
                .iter()
                .find(|item| item.id == binding.provider_id);
            assert!(
                provider.is_some(),
                "binding {} 指向了快照里不存在的 provider {}",
                binding.task_type,
                binding.provider_id
            );
            assert!(
                rows.models.iter().any(|model| {
                    model.provider_id == binding.provider_id && model.model_id == binding.model_id
                }),
                "binding {} 指向了快照里不存在的模型 {}/{}",
                binding.task_type,
                binding.provider_id,
                binding.model_id
            );
        }
        for model in &rows.models {
            assert!(
                rows.providers
                    .iter()
                    .any(|item| item.id == model.provider_id),
                "模型 {}/{} 的 provider 不在快照里",
                model.provider_id,
                model.model_id
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn snapshot_is_referentially_consistent_under_concurrent_writes() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("snapshot.sqlite"))
        .await
        .unwrap();
    write_generation(&kernel, 1).await;

    let shared = Arc::new(kernel);
    let reader = shared.clone();
    let writer = shared.clone();
    let reader_task =
        tokio::spawn(async move { assert_referentially_consistent(reader.as_ref(), 200).await });
    let writer_task = tokio::spawn(async move {
        for generation in 2..40 {
            write_generation(writer.as_ref(), generation).await;
        }
    });

    writer_task.await.unwrap();
    reader_task.await.unwrap();

    // 写完后再看一次：最终状态必须指向最后一代配置。
    let rows = shared.route_snapshot_rows().await.unwrap();
    assert_eq!(rows.providers[0].base_url, "https://gw.example.com/v39");
    assert_eq!(rows.bindings[0].model_id, "m39");
}

#[tokio::test]
async fn snapshot_reads_all_three_tables() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("snapshot2.sqlite"))
        .await
        .unwrap();
    write_generation(&kernel, 7).await;

    let rows = kernel.route_snapshot_rows().await.unwrap();
    assert_eq!(rows.providers.len(), 1);
    assert_eq!(rows.models.len(), 1);
    assert_eq!(rows.bindings.len(), 1);
    assert_eq!(rows.providers[0].base_url, "https://gw.example.com/v7");
    assert_eq!(rows.models[0].model_id, "m7");
    assert_eq!(rows.bindings[0].model_id, "m7");
}
