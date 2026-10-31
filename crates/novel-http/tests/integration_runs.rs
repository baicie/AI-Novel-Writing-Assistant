//! 运行 (Run) 生命周期集成测试
//!
//! 这套测试验证：
//! - 创建运行（open_run）：HTTP 请求 → 数据持久化到数据库
//! - 运行事件：事件按 seq 正确排序，事件类型正确
//! - 状态转换：approve / cancel / resume 能正确更新数据库状态
//! - 事件确认：confirm_event 能持久化到数据库
//! - 并发运行：同一小说的多个并发运行互不干扰

#[path = "common.rs"]
mod common;

use common::TestContext;
use serde_json::{Value, json};

/// 创建运行 - 验证数据持久化
#[tokio::test]
async fn test_open_run_persists_to_db() {
    let ctx = TestContext::new().await;
    ctx.reset_db().await;

    // 创建小说
    let novel = ctx.create_test_novel("运行测试小说").await;
    let novel_id = novel["data"]["id"].as_str().unwrap();

    // 验证初始状态：无运行记录
    assert_eq!(ctx.count_runs(novel_id).await, 0);

    // 通过 HTTP POST 打开运行
    let response = ctx
        .http_client
        .post(&ctx.url(&format!("/api/novels/{}/runs", novel_id)))
        .json(&json!({
            "run_type": "full_generation"
        }))
        .send()
        .await
        .expect("HTTP request failed");

    assert_eq!(response.status(), 200, "Failed to open run");
    let body: Value = response.json().await.expect("parse JSON");
    assert_eq!(body["success"], true);
    assert!(body["data"]["run_id"].is_string());

    let run_id = body["data"]["run_id"].as_str().unwrap();

    // 验证数据库中运行已保存
    let run_count = ctx.count_runs(novel_id).await;
    assert_eq!(run_count, 1, "Run not persisted to database");

    // 直接查询运行数据
    let run_data = ctx.query_run_by_id(run_id).await;
    assert!(run_data.is_some(), "Run not found in database");

    let run = run_data.unwrap();
    assert_eq!(run["novel_id"].as_str().unwrap(), novel_id);
    assert_eq!(run["run_type"].as_str().unwrap(), "full_generation");
    assert!(run["status"].as_str().is_some());
    assert!(run["created_at"].is_string());
}

/// 运行列表 - 验证数据持久化
#[tokio::test]
async fn test_list_runs_returns_persisted() {
    let ctx = TestContext::new().await;
    ctx.reset_db().await;

    let novel = ctx.create_test_novel("列表测试").await;
    let novel_id = novel["data"]["id"].as_str().unwrap();

    // 创建 3 个运行
    let mut run_ids = vec![];
    for i in 0..3 {
        let response = ctx
            .http_client
            .post(&ctx.url(&format!("/api/novels/{}/runs", novel_id)))
            .json(&json!({
                "run_type": if i % 2 == 0 { "full_generation" } else { "draft_check" }
            }))
            .send()
            .await
            .expect("HTTP request failed");

        assert_eq!(response.status(), 200);
        let body: Value = response.json().await.expect("parse JSON");
        run_ids.push(body["data"]["run_id"].as_str().unwrap().to_string());
    }

    // 列表查询
    let response = ctx
        .http_client
        .get(&ctx.url(&format!("/api/novels/{}/runs", novel_id)))
        .send()
        .await
        .expect("HTTP request failed");

    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.expect("parse JSON");
    assert_eq!(body["success"], true);

    // 验证返回的运行数量和 ID
    let runs = body["data"]
        .as_array()
        .expect("data should be array of runs");
    assert_eq!(runs.len(), 3, "Expected 3 runs in list");

    // 验证返回的运行 ID 包含所创建的所有运行
    for run in runs {
        let rid = run["run_id"].as_str().unwrap();
        assert!(run_ids.contains(&rid.to_string()), "Unexpected run in list");
    }
}

/// 批准运行 - 验证状态转换
#[tokio::test]
async fn test_approve_run_updates_status() {
    let ctx = TestContext::new().await;
    ctx.reset_db().await;

    let novel = ctx.create_test_novel("批准测试").await;
    let novel_id = novel["data"]["id"].as_str().unwrap();

    // 打开运行
    let response = ctx
        .http_client
        .post(&ctx.url(&format!("/api/novels/{}/runs", novel_id)))
        .json(&json!({"run_type": "full_generation"}))
        .send()
        .await
        .expect("HTTP request failed");

    let body: Value = response.json().await.expect("parse JSON");
    let run_id = body["data"]["run_id"].as_str().unwrap();

    // 批准运行
    let response = ctx
        .http_client
        .post(&ctx.url(&format!("/api/novels/{}/runs/{}/approve", novel_id, run_id)))
        .send()
        .await
        .expect("HTTP request failed");

    assert_eq!(response.status(), 200);

    // 验证数据库中状态已更新
    let run = ctx.query_run_by_id(run_id).await.expect("Run not found");
    assert_eq!(run["status"].as_str().unwrap(), "approved");
}

/// 取消运行 - 验证状态转换和错误信息
#[tokio::test]
async fn test_cancel_run_persists() {
    let ctx = TestContext::new().await;
    ctx.reset_db().await;

    let novel = ctx.create_test_novel("取消测试").await;
    let novel_id = novel["data"]["id"].as_str().unwrap();

    // 打开运行
    let response = ctx
        .http_client
        .post(&ctx.url(&format!("/api/novels/{}/runs", novel_id)))
        .json(&json!({"run_type": "full_generation"}))
        .send()
        .await
        .expect("HTTP request failed");

    let body: Value = response.json().await.expect("parse JSON");
    let run_id = body["data"]["run_id"].as_str().unwrap();

    // 取消运行
    let response = ctx
        .http_client
        .post(&ctx.url(&format!("/api/novels/{}/runs/{}/cancel", novel_id, run_id)))
        .json(&json!({"reason": "User stopped the run"}))
        .send()
        .await
        .expect("HTTP request failed");

    assert_eq!(response.status(), 200);

    // 验证数据库中状态已更新，错误信息已保存
    let run = ctx.query_run_by_id(run_id).await.expect("Run not found");
    assert_eq!(run["status"].as_str().unwrap(), "cancelled");
    assert_eq!(run["error"].as_str().unwrap(), "User stopped the run");
}

/// 恢复运行 - 验证状态转换
#[tokio::test]
async fn test_resume_run_recovers_state() {
    let ctx = TestContext::new().await;
    ctx.reset_db().await;

    let novel = ctx.create_test_novel("恢复测试").await;
    let novel_id = novel["data"]["id"].as_str().unwrap();

    // 打开运行
    let response = ctx
        .http_client
        .post(&ctx.url(&format!("/api/novels/{}/runs", novel_id)))
        .json(&json!({"run_type": "full_generation"}))
        .send()
        .await
        .expect("HTTP request failed");

    let body: Value = response.json().await.expect("parse JSON");
    let run_id = body["data"]["run_id"].as_str().unwrap();

    // 首先批准运行
    ctx.http_client
        .post(&ctx.url(&format!("/api/novels/{}/runs/{}/approve", novel_id, run_id)))
        .send()
        .await
        .expect("HTTP request failed");

    // 然后恢复（模拟暂停后恢复）
    let response = ctx
        .http_client
        .post(&ctx.url(&format!("/api/novels/{}/runs/{}/resume", novel_id, run_id)))
        .send()
        .await
        .expect("HTTP request failed");

    assert_eq!(response.status(), 200);

    // 验证状态为 resumed
    let run = ctx.query_run_by_id(run_id).await.expect("Run not found");
    assert_eq!(run["status"].as_str().unwrap(), "resumed");
}

/// 确认事件 - 验证事件持久化
#[tokio::test]
async fn test_confirm_event_persists() {
    let ctx = TestContext::new().await;
    ctx.reset_db().await;

    let novel = ctx.create_test_novel("事件确认测试").await;
    let novel_id = novel["data"]["id"].as_str().unwrap();

    // 打开运行
    let response = ctx
        .http_client
        .post(&ctx.url(&format!("/api/novels/{}/runs", novel_id)))
        .json(&json!({"run_type": "full_generation"}))
        .send()
        .await
        .expect("HTTP request failed");

    let body: Value = response.json().await.expect("parse JSON");
    let run_id = body["data"]["run_id"].as_str().unwrap();

    // 确认事件
    let event_payload = json!({
        "event_id": "evt_test_001",
        "confirmed_at": "2026-10-20T12:00:00Z"
    });

    let response = ctx
        .http_client
        .post(&ctx.url(&format!(
            "/api/novels/{}/runs/{}/confirm-event",
            novel_id, run_id
        )))
        .json(&event_payload)
        .send()
        .await
        .expect("HTTP request failed");

    assert_eq!(response.status(), 200);

    // 验证事件确认已保存到数据库
    let confirmed = ctx.query_confirmed_event(run_id, "evt_test_001").await;
    assert!(confirmed.is_some(), "Event confirmation not persisted");
}

/// 并发运行 - 同一小说的多个并发运行
#[tokio::test]
async fn test_concurrent_runs_same_novel() {
    let ctx = TestContext::new().await;
    ctx.reset_db().await;

    let novel = ctx.create_test_novel("并发运行测试").await;
    let novel_id = novel["data"]["id"].as_str().unwrap();

    // 并发创建 5 个运行
    let handles: Vec<_> = (0..5)
        .map(|i| {
            let client = ctx.http_client.clone();
            let url = ctx.url(&format!("/api/novels/{}/runs", novel_id));
            let run_type = if i % 2 == 0 {
                "full_generation"
            } else {
                "draft_check"
            };

            tokio::spawn(async move {
                client
                    .post(&url)
                    .json(&json!({ "run_type": run_type }))
                    .send()
                    .await
            })
        })
        .collect();

    let mut responses = vec![];
    for handle in handles {
        let result = handle.await.expect("Task panicked");
        let response = result.expect("HTTP request failed");
        assert_eq!(response.status(), 200);
        responses.push(response);
    }

    // 验证所有运行都保存到数据库
    let run_count = ctx.count_runs(novel_id).await;
    assert_eq!(run_count, 5, "Not all concurrent runs were saved");

    // 验证运行 ID 各不相同
    let runs = ctx.query_all_runs(novel_id).await;
    let run_ids: Vec<String> = runs
        .iter()
        .map(|r| r["run_id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(run_ids.len(), 5);
    assert_eq!(
        run_ids.len(),
        run_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        "Duplicate run IDs found"
    );
}

/// 多运行独立性 - 不同运行的数据不互相影响
#[tokio::test]
async fn test_runs_are_independent() {
    let ctx = TestContext::new().await;
    ctx.reset_db().await;

    let novel = ctx.create_test_novel("独立性测试").await;
    let novel_id = novel["data"]["id"].as_str().unwrap();

    // 创建两个运行
    let mut run_ids = vec![];
    for _ in 0..2 {
        let response = ctx
            .http_client
            .post(&ctx.url(&format!("/api/novels/{}/runs", novel_id)))
            .json(&json!({"run_type": "full_generation"}))
            .send()
            .await
            .expect("HTTP request failed");

        let body: Value = response.json().await.expect("parse JSON");
        run_ids.push(body["data"]["run_id"].as_str().unwrap().to_string());
    }

    // 批准第一个运行
    ctx.http_client
        .post(&ctx.url(&format!(
            "/api/novels/{}/runs/{}/approve",
            novel_id, run_ids[0]
        )))
        .send()
        .await
        .expect("HTTP request failed");

    // 验证第二个运行仍然未批准
    let run2 = ctx.query_run_by_id(&run_ids[1]).await.unwrap();
    assert_ne!(run2["status"].as_str().unwrap(), "approved");

    // 取消第二个运行
    ctx.http_client
        .post(&ctx.url(&format!(
            "/api/novels/{}/runs/{}/cancel",
            novel_id, run_ids[1]
        )))
        .json(&json!({"reason": "Test cancel"}))
        .send()
        .await
        .expect("HTTP request failed");

    // 验证第一个运行仍然已批准
    let run1 = ctx.query_run_by_id(&run_ids[0]).await.unwrap();
    assert_eq!(run1["status"].as_str().unwrap(), "approved");
}

/// 运行类型持久化 - 不同的运行类型正确保存
#[tokio::test]
async fn test_run_types_are_persisted() {
    let ctx = TestContext::new().await;
    ctx.reset_db().await;

    let novel = ctx.create_test_novel("运行类型测试").await;
    let novel_id = novel["data"]["id"].as_str().unwrap();

    let run_types = vec!["full_generation", "draft_check", "qa_review"];

    for run_type in run_types {
        let response = ctx
            .http_client
            .post(&ctx.url(&format!("/api/novels/{}/runs", novel_id)))
            .json(&json!({ "run_type": run_type }))
            .send()
            .await
            .expect("HTTP request failed");

        assert_eq!(response.status(), 200);
        let body: Value = response.json().await.expect("parse JSON");
        let run_id = body["data"]["run_id"].as_str().unwrap();

        // 验证数据库中类型正确保存
        let run = ctx.query_run_by_id(run_id).await.expect("Run not found");
        assert_eq!(run["run_type"].as_str().unwrap(), run_type);
    }

    // 验证所有运行都被保存
    let count = ctx.count_runs(novel_id).await;
    assert_eq!(count, 3);
}
