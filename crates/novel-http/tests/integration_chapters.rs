//! 章节工作流端到端集成测试
//!
//! 这套测试验证：
//! - 创建章节：HTTP 请求 → 数据库持久化
//! - 保存内容：章节版本管理和冲突检测
//! - 晋升候选：候选正文晋升为官方版本
//! - 保护状态转换：none → user_edited 等
//! - 并发编辑：版本冲突检测

#[path = "common.rs"]
mod common;

use common::TestContext;
use serde_json::json;

/// 创建章节 - 验证基本持久化
#[tokio::test]
async fn test_create_chapter_persists() {
    let ctx = TestContext::new().await;
    ctx.reset_db().await;

    // 先创建小说
    let novel_payload = json!({
        "title": "测试小说"
    });

    let novel_response = ctx
        .http_client
        .post(ctx.url("/api/novels"))
        .json(&novel_payload)
        .send()
        .await
        .expect("create novel");

    let novel_body: serde_json::Value = novel_response.json().await.expect("parse");
    let novel_id = novel_body["data"]["id"].as_str().unwrap();

    // 创建章节
    let chapter_payload = json!({
        "title": "第一章",
        "sortOrder": 1
    });

    let chapter_response = ctx
        .http_client
        .post(ctx.url(&format!("/api/novels/{}/chapters", novel_id)))
        .json(&chapter_payload)
        .send()
        .await
        .expect("create chapter");

    assert_eq!(chapter_response.status(), 200);
    let chapter_body: serde_json::Value = chapter_response.json().await.expect("parse");

    // 验证 HTTP 响应（data 是数组）
    assert_eq!(chapter_body["success"], true);
    assert!(chapter_body["data"].is_array());
    let chapter_array = chapter_body["data"].as_array().unwrap();
    assert_eq!(chapter_array.len(), 1);
    let chapter_id = chapter_array[0]["id"].as_str().unwrap();

    // 验证数据库中有章节
    assert_eq!(ctx.count_chapters(novel_id).await, 1);

    // 从数据库验证章节数据
    let db_chapter = ctx.get_chapter_from_db(novel_id, chapter_id).await;
    assert!(db_chapter.is_some());
    let chapter = db_chapter.unwrap();
    assert_eq!(chapter["title"], "第一章");
    assert_eq!(chapter["sort_order"], 1);
    assert_eq!(chapter["protection"], "none");
}

/// 创建多个章节 - 验证排序
#[tokio::test]
async fn test_create_multiple_chapters_preserve_order() {
    let ctx = TestContext::new().await;
    ctx.reset_db().await;

    // 创建小说
    let novel_payload = json!({"title": "测试小说"});
    let novel_response = ctx
        .http_client
        .post(ctx.url("/api/novels"))
        .json(&novel_payload)
        .send()
        .await
        .expect("create");
    let novel_body: serde_json::Value = novel_response.json().await.expect("parse");
    let novel_id = novel_body["data"]["id"].as_str().unwrap();

    // 创建 3 个章节
    let mut chapter_ids = vec![];
    for i in 1..=3 {
        let payload = json!({
            "title": format!("第{}章", i),
            "sortOrder": i
        });

        let response = ctx
            .http_client
            .post(ctx.url(&format!("/api/novels/{}/chapters", novel_id)))
            .json(&payload)
            .send()
            .await
            .expect("create");

        let body: serde_json::Value = response.json().await.expect("parse");
        let chapter_array = body["data"].as_array().unwrap();
        chapter_ids.push(chapter_array[0]["id"].as_str().unwrap().to_string());
    }

    // 验证全部创建
    assert_eq!(ctx.count_chapters(novel_id).await, 3);

    // 验证每个章节的 sort_order
    for (idx, id) in chapter_ids.iter().enumerate() {
        let chapter = ctx.get_chapter_from_db(novel_id, id).await.unwrap();
        assert_eq!(chapter["sort_order"], (idx + 1) as i32);
    }
}

/// 保存章节内容 - 验证版本管理
#[tokio::test]
async fn test_save_chapter_content_persists() {
    let ctx = TestContext::new().await;
    ctx.reset_db().await;

    // 创建小说和章节
    let novel_id = create_test_novel(&ctx).await;
    let chapter_id = create_test_chapter(&ctx, &novel_id, "第一章", 1).await;

    // 获取章节以获得初始版本
    let get_response = ctx
        .http_client
        .get(ctx.url(&format!("/api/novels/{}/chapters/{}", novel_id, chapter_id)))
        .send()
        .await
        .expect("get chapter");

    let get_body: serde_json::Value = get_response.json().await.expect("parse");
    // detail 返回数组
    let chapter_array = get_body["data"].as_array().unwrap();
    let initial_version = chapter_array[0]["officialVersion"].as_i64().unwrap();

    // 保存章节内容
    let content = "这是第一章的内容。\n包含多个段落。";
    let save_payload = json!({
        "title": "第一章 (修订)",
        "content": content,
        "expectedVersion": initial_version
    });

    let save_response = ctx
        .http_client
        .put(ctx.url(&format!("/api/novels/{}/chapters/{}", novel_id, chapter_id)))
        .json(&save_payload)
        .send()
        .await
        .expect("save chapter");

    assert_eq!(save_response.status(), 200);
    let save_body: serde_json::Value = save_response.json().await.expect("parse");

    // 验证版本号递增（data 也是数组）
    let save_array = save_body["data"].as_array().unwrap();
    let new_version = save_array[0]["officialVersion"].as_i64().unwrap();
    assert_eq!(new_version, initial_version + 1);

    // 验证内容已保存
    assert_eq!(save_array[0]["content"], content);

    // 从数据库验证
    let db_chapter = ctx
        .get_chapter_from_db(&novel_id, &chapter_id)
        .await
        .unwrap();
    assert_eq!(db_chapter["content"], content);
    assert_eq!(db_chapter["official_version"], new_version);
}

/// 连续保存章节 - 版本号持续递增
#[tokio::test]
async fn test_save_chapter_version_increment() {
    let ctx = TestContext::new().await;
    ctx.reset_db().await;

    let novel_id = create_test_novel(&ctx).await;
    let chapter_id = create_test_chapter(&ctx, &novel_id, "第一章", 1).await;

    // 保存 3 次
    for round in 0..3 {
        let get_response = ctx
            .http_client
            .get(ctx.url(&format!("/api/novels/{}/chapters/{}", novel_id, chapter_id)))
            .send()
            .await
            .expect("get");

        let get_body: serde_json::Value = get_response.json().await.expect("parse");
        let chapter_array = get_body["data"].as_array().unwrap();
        let current_version = chapter_array[0]["officialVersion"].as_i64().unwrap();

        let save_payload = json!({
            "title": format!("第一章 v{}", round + 1),
            "content": format!("内容版本 {}", round + 1),
            "expectedVersion": current_version
        });

        let save_response = ctx
            .http_client
            .put(ctx.url(&format!("/api/novels/{}/chapters/{}", novel_id, chapter_id)))
            .json(&save_payload)
            .send()
            .await
            .expect("save");

        assert_eq!(save_response.status(), 200);
        let save_body: serde_json::Value = save_response.json().await.expect("parse");
        let save_array = save_body["data"].as_array().unwrap();
        let new_version = save_array[0]["officialVersion"].as_i64().unwrap();
        assert_eq!(new_version, current_version + 1);
    }

    // 最终版本应该是 3
    let final_chapter = ctx
        .get_chapter_from_db(&novel_id, &chapter_id)
        .await
        .unwrap();
    assert_eq!(final_chapter["official_version"], 3);
}

/// 版本冲突检测 - expectedVersion 不匹配
#[tokio::test]
async fn test_chapter_version_conflict_detection() {
    let ctx = TestContext::new().await;
    ctx.reset_db().await;

    let novel_id = create_test_novel(&ctx).await;
    let chapter_id = create_test_chapter(&ctx, &novel_id, "第一章", 1).await;

    // 先保存一次
    let get1 = ctx
        .http_client
        .get(ctx.url(&format!("/api/novels/{}/chapters/{}", novel_id, chapter_id)))
        .send()
        .await
        .expect("get");
    let body1: serde_json::Value = get1.json().await.expect("parse");
    let chapter_array = body1["data"].as_array().unwrap();
    let v1 = chapter_array[0]["officialVersion"].as_i64().unwrap();

    let save1 = json!({
        "title": "第一次修改",
        "content": "第一次内容",
        "expectedVersion": v1
    });

    ctx.http_client
        .put(ctx.url(&format!("/api/novels/{}/chapters/{}", novel_id, chapter_id)))
        .json(&save1)
        .send()
        .await
        .expect("save 1");

    // 现在试图用过时的版本号保存
    let conflicting_save = json!({
        "title": "冲突修改",
        "content": "冲突内容",
        "expectedVersion": v1  // 仍然用旧版本号
    });

    let conflict_response = ctx
        .http_client
        .put(ctx.url(&format!("/api/novels/{}/chapters/{}", novel_id, chapter_id)))
        .json(&conflicting_save)
        .send()
        .await
        .expect("save conflict");

    // 应该返回冲突错误
    assert!(!conflict_response.status().is_success());
}

/// 并发编辑同一章节 - 验证最后一个请求获胜
#[tokio::test]
async fn test_concurrent_chapter_edits() {
    let ctx = std::sync::Arc::new(TestContext::new().await);
    ctx.reset_db().await;

    let novel_id = create_test_novel(&ctx).await;
    let chapter_id = create_test_chapter(&ctx, &novel_id, "第一章", 1).await;

    // 并发发送 5 个保存请求
    let mut handles = vec![];

    for i in 0..5 {
        let ctx = ctx.clone();
        let novel_id = novel_id.clone();
        let chapter_id = chapter_id.clone();

        let handle = tokio::spawn(async move {
            // 每个并发任务都先获取当前版本
            let get_response = ctx
                .http_client
                .get(ctx.url(&format!("/api/novels/{}/chapters/{}", novel_id, chapter_id)))
                .send()
                .await
                .expect("get");

            let get_body: serde_json::Value = get_response.json().await.expect("parse");
            let chapter_array = get_body["data"].as_array().unwrap();
            let current_version = chapter_array[0]["officialVersion"].as_i64().unwrap();

            // 然后尝试保存
            let save_payload = json!({
                "title": format!("并发编辑 {}", i),
                "content": format!("并发内容 {}", i),
                "expectedVersion": current_version
            });

            ctx.http_client
                .put(ctx.url(&format!("/api/novels/{}/chapters/{}", novel_id, chapter_id)))
                .json(&save_payload)
                .send()
                .await
        });

        handles.push(handle);
    }

    // 等待所有请求完成
    let _success_count = 0;
    let _conflict_count = 0;

    // 等待所有请求完成
    let mut success_count = 0;
    let mut service_unavailable_count = 0;
    let mut conflict_count = 0;

    for handle in handles {
        let result = handle.await.expect("join");
        if let Ok(response) = result {
            let status = response.status();
            if status.is_success() {
                success_count += 1;
            } else if status.as_u16() == 409 {
                conflict_count += 1;
            } else if status.as_u16() == 503 {
                service_unavailable_count += 1;
            }
        }
    }

    // 验证：至少有 1 个成功
    assert!(success_count >= 1);
    // 其余可能是冲突或服务暂时不可用（并发限制）
    assert_eq!(
        success_count + conflict_count + service_unavailable_count,
        5
    );
}

/// 获取完整章节详情
#[tokio::test]
async fn test_get_chapter_full_detail() {
    let ctx = TestContext::new().await;
    ctx.reset_db().await;

    let novel_id = create_test_novel(&ctx).await;
    let chapter_id = create_test_chapter(&ctx, &novel_id, "第一章", 1).await;

    // 获取完整章节信息
    let response = ctx
        .http_client
        .get(ctx.url(&format!(
            "/api/novels/{}/chapters/{}/full",
            novel_id, chapter_id
        )))
        .send()
        .await
        .expect("get full");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("parse");

    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["id"], chapter_id);
    assert_eq!(body["data"]["title"], "第一章");
}

// ============ 辅助函数 ============

async fn create_test_novel(ctx: &TestContext) -> String {
    let payload = json!({"title": "测试小说"});
    let response = ctx
        .http_client
        .post(ctx.url("/api/novels"))
        .json(&payload)
        .send()
        .await
        .expect("create novel");

    let body: serde_json::Value = response.json().await.expect("parse");
    body["data"]["id"].as_str().unwrap().to_string()
}

async fn create_test_chapter(
    ctx: &TestContext,
    novel_id: &str,
    title: &str,
    sort_order: i32,
) -> String {
    let payload = json!({
        "title": title,
        "sortOrder": sort_order
    });

    let response = ctx
        .http_client
        .post(ctx.url(&format!("/api/novels/{}/chapters", novel_id)))
        .json(&payload)
        .send()
        .await
        .expect("create chapter");

    let body: serde_json::Value = response.json().await.expect("parse");
    // create 返回数组
    let chapter_array = body["data"].as_array().unwrap();
    chapter_array[0]["id"].as_str().unwrap().to_string()
}
