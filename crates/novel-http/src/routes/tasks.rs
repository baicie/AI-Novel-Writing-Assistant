//! 任务中心 3.0a 阶段 HTTP 入口。
//!
//! 只暴露两个只读端点：
//! - `GET /api/tasks/overview` —— 顶部数字概览
//! - `GET /api/tasks/recovery-candidates` —— 待恢复任务清单
//!
//! 设计取舍：见 `docs/wiki/architecture/backend-rustization-roadmap.md`。
//! 3.0a 阶段只覆盖 auto_director run，其他业务类目的统计和恢复候选项在 3.0b~f 补全。
//! 列表 / 详情 / 重试 / 取消 / 归档 5 类写操作仍由 Node sidecar 承担，本文件不注册这些路由，
//! 它们会自动落到 `not_implemented` 兜底（501 + `migration: pending`）。

use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};

use crate::error::HttpResult;
use crate::state::KernelState;

pub async fn overview(State(state): State<KernelState>) -> HttpResult<Json<Value>> {
    let data = state.runtime.tasks.get_overview().await?;
    Ok(Json(json!({
        "success": true,
        "data": data,
        "message": "任务概览已加载。",
    })))
}

pub async fn recovery_candidates(State(state): State<KernelState>) -> HttpResult<Json<Value>> {
    let data = state.runtime.tasks.list_recovery_candidates().await?;
    Ok(Json(json!({
        "success": true,
        "data": data,
        "message": "待恢复任务已加载。",
    })))
}
