//! Beta-A1 M1 真实模型执行器（本地验收工具，不参与产品启动）。
//!
//! 安全门禁：
//! - 必须显式设置 `M1_AUTHORIZED=1`；
//! - 必须提供独立 `BIZ_NOVEL_DATA_DIR`；
//! - 必须提供 `M1_MAX_CALLS` 与 `M1_MAX_COST_CNY`；
//! - 密钥只从应用本地加密凭据存储读取（`secrets/` 回退目录），不从参数、日志或
//!   环境变量读取，也不打印。
//!
//! 执行固定的 M1 样本：Planner → Writer → Reviewer → 一次有界 Repair → 复验，
//! 停在最终确认门，不自动把候选提升为正式正文。产物写入数据目录的
//! `m1-<runId>.json`，供人工按 M1 复测包评分。

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use novel_application::{
    ModelSecrets, NoFault, OpenRunInput, PortError, Runtime, RuntimeDeps, SystemClock, TickOutcome,
    UuidIds,
};
use novel_llm::ProtocolRouterProvider;
use novel_platform::{OsSecretStore, SecretStore};
use novel_storage_sqlite::{MODEL_SECRET_REF, SqliteKernel};

const M1_TITLE: &str = "P17.4.7 模型评测样本";
const M1_DESCRIPTION: &str = "边陲小镇的少年在雪夜发现一枚会发光的旧玉佩，由此被卷入山门与一桩旧案。第1章：林动在雪夜发现玉佩，随后亲眼撞见老铁匠被灭口；目击行为成为他卷入旧案的因果起点。不得改写为事后到达，不得提前泄露后续旧案真相。";
const M1_CHAPTER_TITLE: &str = "第一章 雪夜";

struct SecureModelSecrets {
    store: Arc<OsSecretStore>,
}

struct DryRunSecrets;

#[async_trait::async_trait]
impl ModelSecrets for DryRunSecrets {
    async fn api_key(&self, _provider: &str) -> Result<Option<String>, PortError> {
        Ok(Some("dry-run-key".into()))
    }
}

#[async_trait::async_trait]
impl ModelSecrets for SecureModelSecrets {
    async fn api_key(&self, provider: &str) -> Result<Option<String>, PortError> {
        if let Some(key) = self
            .store
            .get(provider)
            .await
            .map_err(|_| PortError::Storage("系统凭据暂时不可用".into()))?
        {
            return Ok(Some(key));
        }
        self.store
            .get(MODEL_SECRET_REF)
            .await
            .map_err(|_| PortError::Storage("系统凭据暂时不可用".into()))
    }
}

fn env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn require_env(name: &str) -> Result<String, String> {
    env(name).ok_or_else(|| format!("缺少环境变量 {name}"))
}

#[tokio::main]
async fn main() {
    let result = if env("M1_SETUP").as_deref() == Some("1") {
        setup_key().await
    } else {
        run().await
    };
    if let Err(error) = result {
        eprintln!("M1_REFUSED: {error}");
        std::process::exit(2);
    }
}

/// 从 stdin 读取 Key（由终端 `read -s` 管道输入），写入应用同一套加密凭据存储。
/// Key 不出现在参数、环境变量或日志里。
async fn setup_key() -> Result<(), String> {
    let data_dir = PathBuf::from(require_env("BIZ_NOVEL_DATA_DIR")?);
    let endpoint = require_env("M1_ENDPOINT")?;
    let model = require_env("M1_MODEL")?;
    let mut key = String::new();
    std::io::stdin()
        .read_to_string(&mut key)
        .map_err(|_| "读取 stdin 的 Key 失败".to_string())?;
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err("stdin 没有提供 Key".into());
    }
    novel_platform::configure_secret_fallback(data_dir.join("secrets"));
    let kernel = Arc::new(
        SqliteKernel::open(&data_dir.join("novel.sqlite"))
            .await
            .map_err(|error| format!("打开验收数据库失败：{error}"))?,
    );
    let store = Arc::new(OsSecretStore);
    store
        .put(MODEL_SECRET_REF, &key)
        .await
        .map_err(|_| "写入本地加密凭据失败".to_string())?;
    drop(key);
    let allow_localhost = endpoint.starts_with("http://127.0.0.1")
        || endpoint.starts_with("http://localhost")
        || endpoint.starts_with("http://[::1]");
    kernel
        .save_model_setup(&endpoint, &model, allow_localhost, novel_platform::now_ms())
        .await
        .map_err(|error| format!("保存模型配置失败：{error}"))?;
    println!(
        "M1_SETUP_SAVED model={model} endpoint={endpoint} localhost={allow_localhost} key_written=true"
    );
    Ok(())
}

async fn run() -> Result<(), String> {
    if env("M1_AUTHORIZED").as_deref() != Some("1") {
        return Err(
            "未授权：先确认新 Key、模型、费用上限与调用次数，再设置 M1_AUTHORIZED=1".into(),
        );
    }
    let data_dir = PathBuf::from(require_env("BIZ_NOVEL_DATA_DIR")?);
    let max_calls: i64 = require_env("M1_MAX_CALLS")?
        .parse()
        .map_err(|_| "M1_MAX_CALLS 必须是整数".to_string())?;
    let max_cost: f64 = require_env("M1_MAX_COST_CNY")?
        .parse()
        .map_err(|_| "M1_MAX_COST_CNY 必须是数字".to_string())?;
    if max_calls <= 0 || max_cost <= 0.0 {
        return Err("调用次数与费用上限必须大于 0".into());
    }

    novel_platform::configure_secret_fallback(data_dir.join("secrets"));
    let db_path = data_dir.join("novel.sqlite");
    let kernel = Arc::new(
        SqliteKernel::open(&db_path)
            .await
            .map_err(|error| format!("打开验收数据库失败：{error}"))?,
    );
    let dry_run = env("M1_DRY_RUN").as_deref() == Some("1");
    if dry_run {
        let endpoint = require_env("M1_ENDPOINT")?;
        kernel
            .save_model_setup(&endpoint, "dry-run-model", true, novel_platform::now_ms())
            .await
            .map_err(|error| format!("写入 dry-run 模型配置失败：{error}"))?;
    }
    let setup = kernel
        .load_model_setup()
        .await
        .map_err(|error| format!("读取模型配置失败：{error}"))?
        .ok_or_else(|| {
            "验收数据目录里没有模型配置；请先用桌面应用的安全设置保存 endpoint、model 与 Key"
                .to_string()
        })?;
    let store = Arc::new(OsSecretStore);
    if !dry_run {
        let key = store
            .get(&setup.secret_ref)
            .await
            .map_err(|_| "读取系统凭据失败".to_string())?
            .ok_or_else(|| "安全存储里没有 API Key；请先在桌面应用中配置新 Key".to_string())?;
        if key.trim().is_empty() {
            return Err("安全存储里的 API Key 为空".into());
        }
        // 不打印 key，也不把它写入任何文件。
        drop(key);
    }

    if env("M1_PREFLIGHT").as_deref() == Some("1") {
        println!(
            "M1_PREFLIGHT_OK model={} endpoint={} key_present=true",
            setup.model, setup.endpoint
        );
        return Ok(());
    }

    let model = env("M1_MODEL").unwrap_or_else(|| setup.model.clone());
    let endpoint = env("M1_ENDPOINT").unwrap_or_else(|| setup.endpoint.clone());
    let secrets: Arc<dyn ModelSecrets> = if dry_run {
        Arc::new(DryRunSecrets)
    } else {
        Arc::new(SecureModelSecrets {
            store: store.clone(),
        })
    };
    let runtime = Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel.clone(),
        llm: Arc::new(ProtocolRouterProvider::new()),
        clock: Arc::new(SystemClock),
        ids: Arc::new(UuidIds),
        faults: Arc::new(NoFault),
        secrets,
        owner: "beta-a1-m1".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });

    let novel = runtime
        .director
        .create_novel(M1_TITLE, M1_DESCRIPTION)
        .await
        .map_err(|error| format!("创建验收小说失败：{error}"))?;
    let chapter = runtime
        .director
        .create_chapter(&novel.id, M1_CHAPTER_TITLE, 1)
        .await
        .map_err(|error| format!("创建验收章节失败：{error}"))?;

    let mut prompts = BTreeMap::new();
    prompts.insert("chapter_draft".into(), "chapter-draft-v3".into());
    prompts.insert("chapter_repair".into(), "chapter-repair-v2".into());
    let opened = runtime
        .director
        .open_run(OpenRunInput {
            command_id: "beta-a1-m1-open".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id.clone(),
            provider: "openai-compatible".into(),
            model: model.clone(),
            prompt_versions: prompts,
            token_budget: Some(8192),
            context_token_budget: Some(24_000),
            max_automatic_retries: 0,
            repair_budget: Some(1),
            endpoint: endpoint.clone(),
            allow_localhost_http: setup.allow_localhost_http,
            step_routes: BTreeMap::new(),
            credential_ref: Some(MODEL_SECRET_REF.into()),
        })
        .await
        .map_err(|error| format!("创建 M1 Run 失败：{error}"))?;
    let run_id = opened.run_id;
    println!(
        "M1_STARTED run_id={run_id} model={model} dry_run={dry_run} max_calls={max_calls} max_cost_cny={max_cost}"
    );

    let mut approvals = 0usize;
    for tick in 0..240 {
        match runtime
            .worker
            .tick()
            .await
            .map_err(|error| format!("Worker tick 失败：{error}"))?
        {
            TickOutcome::Advanced {
                status: next_status,
                run_id: ticked_run,
            } => {
                let status = next_status;
                println!("M1_TICK {tick} status={status}");
                let detail = runtime
                    .director
                    .get_run_detail(&novel.id, &ticked_run)
                    .await
                    .map_err(|error| format!("读取 Run 详情失败：{error}"))?;
                if detail.usage.calls > max_calls {
                    let _ = runtime
                        .director
                        .cancel(&format!("beta-a1-m1-cancel-{tick}"), &ticked_run)
                        .await;
                    println!("M1_BUDGET_STOP calls={}", detail.usage.calls);
                    break;
                }
                if status == "waiting_approval" {
                    let has_review = detail
                        .artifacts
                        .iter()
                        .any(|artifact| artifact.artifact_type == "consistency_review");
                    if has_review {
                        println!("M1_FINAL_GATE run_id={ticked_run}");
                        break;
                    }
                    runtime
                        .director
                        .approve(&format!("beta-a1-m1-approve-{approvals}"), &ticked_run)
                        .await
                        .map_err(|error| format!("确认规划门失败：{error}"))?;
                    approvals += 1;
                }
                if matches!(status.as_str(), "succeeded" | "failed" | "paused") {
                    break;
                }
            }
            TickOutcome::Idle => break,
            _ => break,
        }
    }

    let detail = runtime
        .director
        .get_run_detail(&novel.id, &run_id)
        .await
        .map_err(|error| format!("导出前读取 Run 详情失败：{error}"))?;
    let artifact_path = data_dir.join(format!("m1-{run_id}.json"));
    let json = serde_json::to_string_pretty(&detail)
        .map_err(|error| format!("序列化 M1 产物失败：{error}"))?;
    std::fs::write(&artifact_path, json).map_err(|error| format!("写入 M1 产物失败：{error}"))?;
    println!(
        "M1_RESULT run_id={run_id} status={} pause={:?}/{:?} approvals={approvals} calls={} prompt_tokens={} completion_tokens={}",
        detail.status,
        detail.pause_signal,
        detail.pause_reason,
        detail.usage.calls,
        detail.usage.prompt_tokens,
        detail.usage.completion_tokens
    );
    println!("M1_ARTIFACTS file={}", artifact_path.display());
    println!("M1_DATA_DIR={}", Path::new(&data_dir).display());
    if detail.status == "waiting_approval" {
        println!("M1_NOTE=候选已生成，停在最终确认门；未自动提升为正式正文。");
    }
    Ok(())
}
