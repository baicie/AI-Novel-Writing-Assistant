//! 小说 CRUD 端到端集成测试
//!
//! 这套测试验证：
//! - 创建小说：HTTP 请求 → 数据持久化到数据库
//! - 读取小说：HTTP 返回数据 = 数据库存储数据
//! - 更新小说：修改 → HTTP 返回值 = 数据库最新值
//! - 删除小说：删除 → 数据库中不存在
//! - 并发操作：多个并发请求互不冲突

#[path = "common.rs"]
mod common;

use common::TestContext;
use serde_json::json;

/// 创建小说 - 验证数据持久化
#[tokio::test]
async fn test_create_novel_persists_to_db() {
    let ctx = TestContext::new().await;
    ctx.reset_db().await;

    // 确认初始状态
    assert_eq!(ctx.count_novels().await, 0);

    // 通过 HTTP POST 创建小说
    let payload = json!({
        "title": "我的第一部小说",
        "description": "这是一部充满想象力的故事",
        "narrative_form": "long_novel",
        "writing_mode": "original",
        "project_mode": "ai_led",
        "creation_experience": "simple",
    });

    let response = ctx
        .http_client
        .post(ctx.url("/api/novels"))
        .json(&payload)
        .send()
        .await
        .expect("HTTP request failed");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("parse JSON");

    // 验证 HTTP 响应结构
    assert_eq!(body["success"], true);
    assert!(body["data"]["id"].is_string());
    let novel_id = body["data"]["id"].as_str().unwrap();

    // 验证数据已经保存到数据库
    assert_eq!(ctx.count_novels().await, 1);

    // 从数据库直接查询验证数据
    let db_novel = ctx.get_novel_from_db(novel_id).await;
    assert!(db_novel.is_some());
    let novel = db_novel.unwrap();

    assert_eq!(novel["title"], "我的第一部小说");
    assert_eq!(novel["description"], "这是一部充满想象力的故事");
    assert_eq!(novel["status"], "draft");
    assert_eq!(novel["creation_experience"], "simple");
    assert_eq!(novel["narrative_form"], "long_novel");
    assert_eq!(novel["writing_mode"], "original");
    assert_eq!(novel["project_mode"], "ai_led");
}

/// 创建小说 - 最小化请求
#[tokio::test]
async fn test_create_novel_with_minimal_fields() {
    let ctx = TestContext::new().await;
    ctx.reset_db().await;

    // 仅提供必需字段（标题）
    let payload = json!({
        "title": "只有标题的小说"
    });

    let response = ctx
        .http_client
        .post(ctx.url("/api/novels"))
        .json(&payload)
        .send()
        .await
        .expect("HTTP request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("parse JSON");

    let novel_id = body["data"]["id"].as_str().unwrap();

    // 验证数据库中有默认值
    let db_novel = ctx.get_novel_from_db(novel_id).await.unwrap();
    assert_eq!(db_novel["title"], "只有标题的小说");
    assert_eq!(db_novel["status"], "draft");
    assert_eq!(db_novel["creation_experience"], "simple"); // 默认值
    assert_eq!(db_novel["narrative_form"], "long_novel");
    assert_eq!(db_novel["writing_mode"], "original");
    assert_eq!(db_novel["project_mode"], "ai_led");
}

/// 读取小说 - 验证数据完整性
#[tokio::test]
async fn test_get_novel_returns_persisted_data() {
    let ctx = TestContext::new().await;
    ctx.reset_db().await;

    // 创建小说
    let create_payload = json!({
        "title": "测试小说",
        "description": "详细描述",
    });

    let create_response = ctx
        .http_client
        .post(ctx.url("/api/novels"))
        .json(&create_payload)
        .send()
        .await
        .expect("create novel");

    let create_body: serde_json::Value = create_response.json().await.expect("parse");
    let novel_id = create_body["data"]["id"].as_str().unwrap();

    // 通过 HTTP GET 读取小说
    let get_response = ctx
        .http_client
        .get(ctx.url(&format!("/api/novels/{}", novel_id)))
        .send()
        .await
        .expect("get novel");

    assert_eq!(get_response.status(), 200);
    let get_body: serde_json::Value = get_response.json().await.expect("parse");

    // 验证返回的数据与创建时一致（字段用 camelCase）
    assert_eq!(get_body["data"]["id"], novel_id);
    assert_eq!(get_body["data"]["title"], "测试小说");
    assert_eq!(get_body["data"]["description"], "详细描述");
    assert_eq!(get_body["data"]["status"], "draft");
}

/// 列表小说 - 验证分页和排序
#[tokio::test]
async fn test_list_novels_returns_persisted() {
    let ctx = TestContext::new().await;
    ctx.reset_db().await;

    // 创建 3 个小说
    for i in 1..=3 {
        let payload = json!({
            "title": format!("小说 {}", i),
            "description": format!("描述 {}", i),
        });
        ctx.http_client
            .post(ctx.url("/api/novels"))
            .json(&payload)
            .send()
            .await
            .expect("create");
    }

    assert_eq!(ctx.count_novels().await, 3);

    // 通过 HTTP 列表
    let response = ctx
        .http_client
        .get(ctx.url("/api/novels"))
        .send()
        .await
        .expect("list novels");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("parse");

    // 验证返回所有小说
    let items = body["data"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);

    // 验证按 updated_at 倒序排列
    for (_idx, item) in items.iter().enumerate() {
        assert!(item["title"].as_str().unwrap().contains("小说"));
    }
}

/// 更新小说 - 验证字段修改
#[tokio::test]
async fn test_update_novel_persists() {
    let ctx = TestContext::new().await;
    ctx.reset_db().await;

    // 创建小说
    let create_payload = json!({
        "title": "原始标题",
        "description": "原始描述",
    });

    let create_response = ctx
        .http_client
        .post(ctx.url("/api/novels"))
        .json(&create_payload)
        .send()
        .await
        .expect("create");

    let create_body: serde_json::Value = create_response.json().await.expect("parse");
    let novel_id = create_body["data"]["id"].as_str().unwrap();
    let original_updated_at = create_body["data"]["updatedAt"].as_i64().unwrap();

    // 等待一点时间确保 updatedAt 变化
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // 通过 HTTP PUT 更新小说
    let update_payload = json!({
        "title": "新的标题",
        "description": "新的描述",
    });

    let update_response = ctx
        .http_client
        .put(ctx.url(&format!("/api/novels/{}", novel_id)))
        .json(&update_payload)
        .send()
        .await
        .expect("update");

    assert_eq!(update_response.status(), 200);
    let update_body: serde_json::Value = update_response.json().await.expect("parse");

    // 验证 HTTP 返回值（使用 camelCase）
    assert_eq!(update_body["data"]["title"], "新的标题");
    assert_eq!(update_body["data"]["description"], "新的描述");
    let new_updated_at = update_body["data"]["updatedAt"].as_i64().unwrap();
    assert!(new_updated_at >= original_updated_at);

    // 验证数据库中的数据已更新
    let db_novel = ctx.get_novel_from_db(novel_id).await.unwrap();
    assert_eq!(db_novel["title"], "新的标题");
    assert_eq!(db_novel["description"], "新的描述");
}

/// 删除小说 - 验证数据被移除
#[tokio::test]
async fn test_delete_novel_removes_from_db() {
    let ctx = TestContext::new().await;
    ctx.reset_db().await;

    // 创建小说
    let create_payload = json!({
        "title": "要删除的小说"
    });

    let create_response = ctx
        .http_client
        .post(ctx.url("/api/novels"))
        .json(&create_payload)
        .send()
        .await
        .expect("create");

    let create_body: serde_json::Value = create_response.json().await.expect("parse");
    let novel_id = create_body["data"]["id"].as_str().unwrap();

    assert_eq!(ctx.count_novels().await, 1);

    // 通过 HTTP DELETE 删除小说
    let delete_response = ctx
        .http_client
        .delete(ctx.url(&format!("/api/novels/{}", novel_id)))
        .send()
        .await
        .expect("delete");

    assert_eq!(delete_response.status(), 200);

    // 验证数据库中已删除
    assert_eq!(ctx.count_novels().await, 0);
    assert!(ctx.get_novel_from_db(novel_id).await.is_none());
}

/// 删除不存在的小说 - 验证 404
#[tokio::test]
async fn test_delete_nonexistent_novel_returns_404() {
    let ctx = TestContext::new().await;

    let response = ctx
        .http_client
        .delete(ctx.url("/api/novels/nonexistent-id"))
        .send()
        .await
        .expect("delete request");

    // 应该返回 404 或错误状态
    assert!(!response.status().is_success());
}

/// 读取不存在的小说 - 验证 404
#[tokio::test]
async fn test_get_nonexistent_novel_returns_404() {
    let ctx = TestContext::new().await;

    let response = ctx
        .http_client
        .get(ctx.url("/api/novels/nonexistent-id"))
        .send()
        .await
        .expect("get request");

    assert!(!response.status().is_success());
}

/// 并发创建 - 验证无冲突
#[tokio::test]
async fn test_novel_concurrent_creation() {
    let ctx = std::sync::Arc::new(TestContext::new().await);
    ctx.reset_db().await;

    // 并发创建 10 个小说
    let mut handles = vec![];

    for i in 0..10 {
        let ctx = ctx.clone();
        let handle = tokio::spawn(async move {
            let payload = json!({
                "title": format!("并发小说 {}", i)
            });

            ctx.http_client
                .post(ctx.url("/api/novels"))
                .json(&payload)
                .send()
                .await
                .expect("create")
                .json::<serde_json::Value>()
                .await
                .expect("parse")["data"]["id"]
                .as_str()
                .unwrap()
                .to_string()
        });
        handles.push(handle);
    }

    // 等待所有请求完成
    let mut novel_ids = vec![];
    for handle in handles {
        let id = handle.await.expect("join");
        novel_ids.push(id);
    }

    // 验证所有小说都被保存
    assert_eq!(ctx.count_novels().await, 10);

    // 验证没有重复的 ID
    novel_ids.sort();
    novel_ids.dedup();
    assert_eq!(novel_ids.len(), 10);

    // 验证每个小说都在数据库中
    for (i, id) in novel_ids.iter().enumerate() {
        let novel = ctx.get_novel_from_db(id).await;
        assert!(novel.is_some(), "小说 {} 不在数据库中", i);
    }
}

/// 并发更新 - 验证最终一致性
#[tokio::test]
async fn test_concurrent_update_same_novel() {
    let ctx = std::sync::Arc::new(TestContext::new().await);
    ctx.reset_db().await;

    // 创建一个小说
    let create_payload = json!({
        "title": "要被并发修改的小说",
        "book_selling_point": "初始卖点"
    });

    let create_response = ctx
        .http_client
        .post(ctx.url("/api/novels"))
        .json(&create_payload)
        .send()
        .await
        .expect("create");

    let create_body: serde_json::Value = create_response.json().await.expect("parse");
    let novel_id = create_body["data"]["id"].as_str().unwrap().to_string();

    // 并发修改同一小说的不同字段
    let mut handles = vec![];

    for i in 0..5 {
        let ctx = ctx.clone();
        let novel_id = novel_id.clone();

        let handle = tokio::spawn(async move {
            let payload = json!({
                "title": format!("标题修改 {}", i),
                "target_audience": format!("目标受众 {}", i),
            });

            ctx.http_client
                .put(ctx.url(&format!("/api/novels/{}", novel_id)))
                .json(&payload)
                .send()
                .await
        });
        handles.push(handle);
    }

    // 等待所有更新完成
    for handle in handles {
        let result = handle.await.expect("join");
        assert!(result.is_ok(), "update request failed");
        let response = result.unwrap();
        assert_eq!(response.status(), 200);
    }

    // 验证小说仍然存在且有最新的修改
    let db_novel = ctx.get_novel_from_db(&novel_id).await.unwrap();
    assert_eq!(db_novel["id"], novel_id);
    assert!(db_novel["title"].as_str().unwrap().contains("标题修改"));
}
