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
use novel_llm::{ProtocolRouterProvider, ProviderPolicy};
use novel_platform::{OsSecretStore, SecretStore};
use novel_storage_sqlite::{MODEL_SECRET_REF, SqliteKernel};

const M1_TITLE: &str = "P17.4.7 模型评测样本";
const M1_DESCRIPTION: &str = "边陲小镇的少年在雪夜发现一枚会发光的旧玉佩，由此被卷入山门与一桩旧案。第1章：林动在雪夜发现玉佩，随后亲眼撞见老铁匠被灭口；目击行为成为他卷入旧案的因果起点。不得改写为事后到达，不得提前泄露后续旧案真相。";
const M1_CHAPTER_TITLE: &str = "第一章 雪夜";

/// M1 复测固定使用的 Prompt 版本。`story_macro` 与 `chapter_list` 是新 Planner 契约的
/// 成对版本，缺一不可；`open_run` 也会拒绝只固定一半的请求。
fn m1_prompt_versions() -> BTreeMap<String, String> {
    let mut prompts = BTreeMap::new();
    prompts.insert("story_macro".into(), "story-macro-v2".into());
    prompts.insert("chapter_list".into(), "chapter-list-v3".into());
    prompts.insert("chapter_draft".into(), "chapter-draft-v3".into());
    prompts.insert("chapter_repair".into(), "chapter-repair-v2".into());
    prompts
}

/// M1 执行器的 Provider 策略。
///
/// 产品默认的 120 秒是整包总超时（含响应体），真实模型写结构化产物时可能在流式返回
/// 途中被切断。验收执行器单独放宽到 300 秒，仍可用 `M1_TIMEOUT_SECS` 覆盖。
/// `max_attempts=1` 让每个调用预算单位只对应一次真实请求，避免 Provider 内部重试
/// 绕过调用上限。
fn m1_provider_policy() -> ProviderPolicy {
    let timeout_secs = env("M1_TIMEOUT_SECS")
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value >= 30)
        .unwrap_or(300);
    ProviderPolicy {
        timeout: std::time::Duration::from_secs(timeout_secs),
        max_attempts: 1,
        ..ProviderPolicy::default()
    }
}

/// 脱敏版本清单：只有版本名，不含 endpoint、凭据或提示词正文。
fn prompt_versions_line(prompts: &BTreeMap<String, String>) -> String {
    prompts
        .iter()
        .map(|(step, version)| format!("{step}={version}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[path = "m1_run/budget.rs"]
mod budget;

use budget::{BudgetedLlm, CallBudget, resolve_cost_guard};

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
    } else if env("M1_PROBE").as_deref() == Some("1") {
        probe_run().await
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
    let cost_guard = resolve_cost_guard(dry_run, env("M1_COST_GUARD").as_deref())?;
    let budget = CallBudget::new(max_calls as u64);
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
        llm: Arc::new(BudgetedLlm::new(
            Arc::new(ProtocolRouterProvider::with_openai_policy(
                m1_provider_policy(),
            )),
            budget.clone(),
        )),
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

    let nonce = env("M1_RUN_NONCE").unwrap_or_else(|| novel_platform::now_ms().to_string());
    let prompts = m1_prompt_versions();
    println!("M1_PROMPTS {}", prompt_versions_line(&prompts));
    println!(
        "M1_BUDGET_GUARD max_calls={max_calls} max_cost_cny={max_cost} cost_guard={cost_guard:?} local_cost_enforcement=disabled"
    );
    let opened = runtime
        .director
        .open_run(OpenRunInput {
            command_id: format!("beta-a1-m1-open-{nonce}"),
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
                        .cancel(&format!("beta-a1-m1-cancel-{nonce}-{tick}"), &ticked_run)
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
                        .approve(
                            &format!("beta-a1-m1-approve-{nonce}-{approvals}"),
                            &ticked_run,
                        )
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
    let (requests_started, unknown_usage) = budget.snapshot();
    println!(
        "M1_RESULT run_id={run_id} status={} pause={:?}/{:?} approvals={approvals} calls={} requests_started={requests_started} unknown_usage={unknown_usage} prompt_tokens={} completion_tokens={}",
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

// ---------------------------------------------------------------------------
// 失败路径探针：同一冻结上下文 + 一份“事后到达”的违规候选，验证 Reviewer 与 Repair。
// 只做 3 次真实调用：review → repair → re-review；不修改主 Run。
// ---------------------------------------------------------------------------

const VIOLATING_CANDIDATE: &str = "林动揣着玉佩赶回铁匠铺时，院门半开着，雪地上有一串凌乱的脚印。他喊了两声师父，没人应。走进院子，老铁匠已经倒在炉子旁边的雪地里，胸口一片暗红，人早就没气了。炉火快熄了，雪从破了的窗纸里飘进来。林动不知道是谁下的手，也不知道师父临死前看见了什么。他蹲下去，手抖得厉害，只看见师父的眼睛还睁着。雪落下来，很快盖住了那些脚印。他把玉佩攥在手里，站了很久，才想起要去叫人。";

const PROBE_REPAIR_OUTPUT_TOKENS: u64 = 16_384;

const REVIEW_INSTRUCTION: &str = "\n审校要求：违反 hard 章节义务时必须填写 obligationId，且只能使用上下文中的义务 ID；没有采用 advisory 建议不构成问题；evidence 必须逐字引用候选正文并能在候选正文中定位；不得引用历史正文。\n";

fn build_review_prompt(context_text: &str, candidate: &str) -> Result<String, String> {
    let asset = novel_prompt::resolve("consistency_review", "consistency-review-v2")
        .map_err(|_| "consistency-review-v2 不存在".to_string())?;
    let mut context = context_text.to_string();
    context.push_str(REVIEW_INSTRUCTION);
    context.push_str("\n候选正文：");
    context.push_str(candidate);
    let mut vars = BTreeMap::new();
    vars.insert("context".to_string(), context);
    novel_prompt::render(asset.template, &vars).map_err(|_| "渲染审校提示词失败".to_string())
}

fn parse_review(text: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str::<serde_json::Value>(text.trim())
        .map_err(|_| "Reviewer 返回的不是合法 JSON".to_string())
}

fn review_issues(value: &serde_json::Value) -> Vec<serde_json::Value> {
    value
        .get("issues")
        .or_else(|| value.get("issueDetails"))
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn normalized(text: &str) -> String {
    text.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn issue_cites(issues: &[serde_json::Value], obligation_id: &str, candidate: &str) -> bool {
    issues.iter().any(|issue| {
        issue
            .get("obligationId")
            .and_then(serde_json::Value::as_str)
            == Some(obligation_id)
            && issue
                .get("evidence")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|evidence| normalized(candidate).contains(&normalized(evidence)))
    })
}

#[allow(clippy::too_many_arguments)]
async fn call_model(
    llm: &dyn novel_llm::LlmProvider,
    api_key: &str,
    endpoint: &str,
    model: &str,
    allow_localhost: bool,
    request_id: &str,
    prompt: String,
    max_output_tokens: u64,
) -> Result<(String, novel_llm::TokenUsage), String> {
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<String>(64);
    let collect = tokio::spawn(async move {
        let mut text = String::new();
        while let Some(delta) = receiver.recv().await {
            text.push_str(&delta);
        }
        text
    });
    let response = llm
        .generate_with_protocol(
            novel_llm::LlmRequest {
                request_id: request_id.to_string(),
                model: model.to_string(),
                prompt,
                endpoint: endpoint.to_string(),
                api_key: api_key.to_string(),
                allow_localhost_http: allow_localhost,
                max_output_tokens: Some(max_output_tokens),
            },
            "openai-compatible",
            None,
            sender,
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .map_err(|error| format!("模型调用失败：{error}"))?;
    let raw = collect.await.unwrap_or_default();
    let text = if raw.trim().is_empty() {
        response.text
    } else {
        raw
    };
    Ok((text, response.usage))
}

async fn probe_run() -> Result<(), String> {
    if env("M1_AUTHORIZED").as_deref() != Some("1") {
        return Err("未授权：M1_PROBE 也必须显式设置 M1_AUTHORIZED=1".into());
    }
    let data_dir = PathBuf::from(require_env("BIZ_NOVEL_DATA_DIR")?);
    let run_id = require_env("M1_RUN_ID")?;
    let novel_id = require_env("M1_NOVEL_ID")?;
    let max_calls: i64 = require_env("M1_MAX_CALLS")?
        .parse()
        .map_err(|_| "M1_MAX_CALLS 必须是整数".to_string())?;
    if max_calls < 3 {
        return Err("失败路径探针需要至少 3 次调用上限".into());
    }
    let _max_cost: f64 = require_env("M1_MAX_COST_CNY")?
        .parse()
        .map_err(|_| "M1_MAX_COST_CNY 必须是数字".to_string())?;
    let cost_guard = resolve_cost_guard(false, env("M1_COST_GUARD").as_deref())?;
    println!(
        "M1_BUDGET_GUARD max_calls={max_calls} cost_guard={cost_guard:?} local_cost_enforcement=disabled"
    );

    novel_platform::configure_secret_fallback(data_dir.join("secrets"));
    let kernel = Arc::new(
        SqliteKernel::open(&data_dir.join("novel.sqlite"))
            .await
            .map_err(|error| format!("打开验收数据库失败：{error}"))?,
    );
    let setup = kernel
        .load_model_setup()
        .await
        .map_err(|error| format!("读取模型配置失败：{error}"))?
        .ok_or_else(|| "验收数据目录里没有模型配置".to_string())?;
    let store = Arc::new(OsSecretStore);
    let api_key = store
        .get(&setup.secret_ref)
        .await
        .map_err(|_| "读取系统凭据失败".to_string())?
        .ok_or_else(|| "安全存储里没有 API Key".to_string())?;
    let secrets = Arc::new(SecureModelSecrets { store });
    let budget = CallBudget::new(max_calls as u64);
    let runtime = Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel.clone(),
        llm: Arc::new(BudgetedLlm::new(
            Arc::new(ProtocolRouterProvider::with_openai_policy(
                m1_provider_policy(),
            )),
            budget.clone(),
        )),
        clock: Arc::new(SystemClock),
        ids: Arc::new(UuidIds),
        faults: Arc::new(NoFault),
        secrets,
        owner: "beta-a1-m1-probe".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });

    let detail = runtime
        .director
        .get_run_detail(&novel_id, &run_id)
        .await
        .map_err(|error| format!("读取 Run 详情失败：{error}"))?;
    let artifacts = detail.artifacts;
    let llm = BudgetedLlm::new(
        Arc::new(ProtocolRouterProvider::with_openai_policy(
            m1_provider_policy(),
        )),
        budget.clone(),
    );
    let context = artifacts
        .iter()
        .filter(|artifact| artifact.artifact_type == "context")
        .max_by_key(|artifact| artifact.version)
        .ok_or_else(|| "Run 没有 context 产物".to_string())?;
    let context_value: serde_json::Value =
        serde_json::from_str(&context.payload_json).map_err(|_| "context 产物损坏".to_string())?;
    let context_text = context_value
        .get("promptText")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "context 缺少 promptText".to_string())?
        .to_string();
    let obligations_text = context_value
        .get("obligationsText")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("（没有结构化义务；沿用已确认上下文，不得改写已确认事件）")
        .to_string();

    let review_prompt = build_review_prompt(&context_text, VIOLATING_CANDIDATE)?;
    let (review1_raw, usage1) = call_model(
        &llm,
        &api_key,
        &setup.endpoint,
        &setup.model,
        setup.allow_localhost_http,
        "m1-probe-review-1",
        review_prompt,
        8_192,
    )
    .await?;
    let review1 = parse_review(&review1_raw)?;
    let issues1 = review_issues(&review1);
    let reviewer_pass = issue_cites(&issues1, "ch1-req-1", VIOLATING_CANDIDATE);
    println!(
        "M1_PROBE_REVIEW issues={} reviewer_cites_req1={reviewer_pass}",
        issues1.len()
    );

    if !reviewer_pass {
        let path = data_dir.join(format!("m1-probe-{run_id}.json"));
        let probe = serde_json::json!({
            "runId": run_id,
            "violatingCandidate": VIOLATING_CANDIDATE,
            "review1": review1_raw,
            "review1Parsed": review1,
            "reviewerCitesReq1": false,
            "repair": null,
            "review2": null,
            "usage": {"review1": usage_json(&usage1)},
            "verdict": "M1_FAIL_REVIEWER"
        });
        std::fs::write(&path, serde_json::to_string_pretty(&probe).unwrap())
            .map_err(|error| format!("写入探针产物失败：{error}"))?;
        println!("M1_PROBE_RESULT reviewer=FAIL repair=NOT_RUN rereview=NOT_RUN");
        println!("M1_PROBE_ARTIFACTS file={}", path.display());
        return Ok(());
    }

    let repair_asset = novel_prompt::resolve("chapter_repair", "chapter-repair-v2")
        .map_err(|_| "chapter-repair-v2 不存在".to_string())?;
    let mut vars = BTreeMap::new();
    vars.insert("obligations".to_string(), obligations_text);
    vars.insert("context".to_string(), context_text.clone());
    vars.insert(
        "issues".to_string(),
        serde_json::to_string(&issues1).map_err(|_| "审校问题无法序列化".to_string())?,
    );
    vars.insert("candidate".to_string(), VIOLATING_CANDIDATE.to_string());
    let repair_prompt = novel_prompt::render(repair_asset.template, &vars)
        .map_err(|_| "渲染修复提示词失败".to_string())?;
    let (repair_raw, usage2) = call_model(
        &llm,
        &api_key,
        &setup.endpoint,
        &setup.model,
        setup.allow_localhost_http,
        "m1-probe-repair",
        repair_prompt,
        PROBE_REPAIR_OUTPUT_TOKENS,
    )
    .await?;
    let repaired = repair_raw.trim().to_string();
    let repair_nonempty = !repaired.is_empty();
    let repaired_has_witness =
        repaired.contains("亲眼") && (repaired.contains("灰斗篷") || repaired.contains("凶手"));
    println!(
        "M1_PROBE_REPAIR nonempty={repair_nonempty} witness_marker={repaired_has_witness} chars={} budget={PROBE_REPAIR_OUTPUT_TOKENS}",
        repaired.chars().count()
    );

    let review2_prompt = build_review_prompt(&context_text, &repaired)?;
    let (review2_raw, usage3) = call_model(
        &llm,
        &api_key,
        &setup.endpoint,
        &setup.model,
        setup.allow_localhost_http,
        "m1-probe-review-2",
        review2_prompt,
        8_192,
    )
    .await?;
    let review2 = parse_review(&review2_raw)?;
    let issues2 = review_issues(&review2);
    let residual_req1 = issue_cites(&issues2, "ch1-req-1", &repaired);
    println!(
        "M1_PROBE_REREVIEW issues={} residual_req1={residual_req1}",
        issues2.len()
    );

    let path = data_dir.join(format!("m1-probe-{run_id}.json"));
    let verdict = if repair_nonempty && !residual_req1 {
        "M1_READY_FOR_AUTHOR_REVIEW"
    } else {
        "M1_FAIL_REPAIR"
    };
    let probe = serde_json::json!({
        "runId": run_id,
        "violatingCandidate": VIOLATING_CANDIDATE,
        "review1": review1_raw,
        "review1Parsed": review1,
        "reviewerCitesReq1": reviewer_pass,
        "repair": repaired,
        "repairWitnessMarker": repaired_has_witness,
        "review2": review2_raw,
        "review2Parsed": review2,
        "residualReq1": residual_req1,
        "usage": {
            "review1": usage_json(&usage1),
            "repair": usage_json(&usage2),
            "review2": usage_json(&usage3)
        },
        "verdict": verdict
    });
    std::fs::write(&path, serde_json::to_string_pretty(&probe).unwrap())
        .map_err(|error| format!("写入探针产物失败：{error}"))?;
    println!(
        "M1_PROBE_RESULT reviewer={} repair={} rereview={} verdict={verdict}",
        if reviewer_pass { "PASS" } else { "FAIL" },
        if repair_nonempty && repaired_has_witness {
            "PASS"
        } else {
            "CHECK"
        },
        if residual_req1 { "FAIL" } else { "PASS" }
    );
    println!("M1_PROBE_ARTIFACTS file={}", path.display());
    Ok(())
}

fn usage_json(usage: &novel_llm::TokenUsage) -> serde_json::Value {
    serde_json::json!({
        "promptTokens": usage.prompt_tokens,
        "completionTokens": usage.completion_tokens
    })
}

#[cfg(test)]
#[path = "m1_run/tests.rs"]
mod tests;
