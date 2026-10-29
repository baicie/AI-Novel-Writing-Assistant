use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::json;

/// 所有未在 Rust 内核里实现的 `/api/*` 路径都走这个 handler，返回 501 + 当前已实现路径清单。
///
/// 注意：fallback 不能同时使用 `State` 抽取器和 `Request`，否则会冲突。所以这个 handler
/// 拿不到 `KernelState`，改成直接返回 501 + 静态 JSON 响应，里面只放提示文案。
/// supported paths 列表由 `GET /api/health` 返回，前端可以靠这个了解内核当前能力。
pub async fn handler(request: axum::http::Request<axum::body::Body>) -> impl IntoResponse {
    let path = request.uri().path().to_string();
    let method = request.method().as_str().to_string();
    tracing::warn!(method = %method, path = %path, "请求命中 501 兜底路由");
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "success": false,
            "error": format!("本地创作服务还没有实现 {method} {path}，暂时无法处理。"),
            "migration": "pending",
            "hint": "调用 GET /api/health 查看当前 Rust 内核已经实现的接口。",
        })),
    )
}
