//! HTTP 处理器集成模块
//!
//! 将 UseCase 与 HTTP 层集成，实现所有 REST 端点的处理器。
//!
//! 注意：这个模块提供处理器的框架结构。
//! 实际的处理器实现需要与具体的 Web 框架（如 actix-web）集成。

use crate::error::AppError;
use crate::http_error::map_error_to_http;
use crate::http_request::ValidateRequest;

// ============================================================================
// 处理器组织
// ============================================================================

/// 所有 HTTP 处理器的集合
pub struct HttpHandlers;

impl HttpHandlers {
    /// 获取所有处理器的列表
    pub fn list() -> Vec<&'static str> {
        vec![
            "create_novel",
            "list_novels",
            "get_novel",
            "update_novel",
            "delete_novel",
            "set_creation_experience",
            "create_chapter",
            "list_chapters",
            "get_chapter",
            "save_chapter",
            "promote_chapter",
            "start_run",
            "approve_run",
        ]
    }

    /// 获取处理器总数
    pub fn count() -> usize {
        Self::list().len()
    }

    /// 检查处理器是否存在
    pub fn has(name: &str) -> bool {
        Self::list().contains(&name)
    }

    /// 按类别统计处理器数量
    pub fn count_by_category(category: &str) -> usize {
        match category {
            "novel" => 6,   // create, list, get, update, delete, set_experience
            "chapter" => 5, // create, list, get, save, promote
            "run" => 2,     // start, approve
            _ => 0,
        }
    }
}

// ============================================================================
// 通用处理器接口
// ============================================================================

/// HTTP 处理器的通用特征
pub trait HttpHandler {
    type Request: ValidateRequest;
    type Response;

    /// 处理 HTTP 请求
    fn handle(request: Self::Request) -> Result<Self::Response, AppError> {
        // 1. 验证请求
        request.validate().map_err(|e| AppError::validation(&e))?;

        // 2. 处理逻辑（由子类型实现）
        // 3. 返回响应
        Self::process(&request)
    }

    /// 具体的处理逻辑（由具体处理器实现）
    fn process(request: &Self::Request) -> Result<Self::Response, AppError>;
}

// ============================================================================
// 错误处理工具
// ============================================================================

/// 处理错误并转换为 HTTP 响应
pub struct ErrorHandler;

impl ErrorHandler {
    /// 将应用错误转换为 HTTP 状态码
    pub fn status_code(error: &AppError) -> u16 {
        let http_error = map_error_to_http(error.clone());
        match http_error.status_code {
            crate::http_error::HttpStatusCode::OK => 200,
            crate::http_error::HttpStatusCode::Created => 201,
            crate::http_error::HttpStatusCode::BadRequest => 400,
            crate::http_error::HttpStatusCode::NotFound => 404,
            crate::http_error::HttpStatusCode::Conflict => 409,
            crate::http_error::HttpStatusCode::InternalServerError => 500,
        }
    }

    /// 获取错误响应
    pub fn response(error: AppError) -> (u16, String) {
        let http_error = map_error_to_http(error);
        let status = match http_error.status_code {
            crate::http_error::HttpStatusCode::OK => 200,
            crate::http_error::HttpStatusCode::Created => 201,
            crate::http_error::HttpStatusCode::BadRequest => 400,
            crate::http_error::HttpStatusCode::NotFound => 404,
            crate::http_error::HttpStatusCode::Conflict => 409,
            crate::http_error::HttpStatusCode::InternalServerError => 500,
        };
        let error_response = serde_json::to_string(&http_error.error_response)
            .unwrap_or_else(|_| "Internal Server Error".to_string());
        (status, error_response)
    }
}

// ============================================================================
// 处理器实现文档
// ============================================================================

// 小说处理器实现指南
//
// 当与 actix-web 集成时，处理器实现类似于：
//
// ```rust
// pub async fn create_novel_handler(
//     service: web::Data<Arc<DirectorService>>,
//     req: web::Json<CreateNovelRequest>,
// ) -> impl Responder {
//     // 1. 验证请求
//     if let Err(e) = req.validate() {
//         return HttpResponse::BadRequest()
//             .json(ErrorResponse::new("VALIDATION", e));
//     }
//
//     // 2. 创建 UseCase 输入
//     let input = CreateNovelInput {
//         title: req.title.clone(),
//         description: req.description.clone().unwrap_or_default(),
//         narrative_form: req.narrative_form.clone().unwrap_or_default(),
//         genre: req.genre.clone().unwrap_or_default(),
//         setting: req.setting.clone().unwrap_or_default(),
//     };
//
//     // 3. 调用 UseCase
//     match service.create_novel(&input).await {
//         Ok(result) => {
//             let response = NovelDetailResponse::from(result);
//             HttpResponse::Created().json(response)
//         }
//         Err(error) => {
//             let (status, body) = ErrorHandler::response(error);
//             HttpResponse::build(StatusCode::from_u16(status).unwrap())
//                 .json(serde_json::from_str::<serde_json::Value>(&body).unwrap_or_default())
//         }
//     }
// }
// ```

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_handlers_list() {
        let handlers = HttpHandlers::list();
        assert_eq!(handlers.len(), 13);
    }

    #[test]
    fn test_http_handlers_count() {
        assert_eq!(HttpHandlers::count(), 13);
    }

    #[test]
    fn test_http_handlers_has() {
        assert!(HttpHandlers::has("create_novel"));
        assert!(HttpHandlers::has("list_chapters"));
        assert!(HttpHandlers::has("approve_run"));
        assert!(!HttpHandlers::has("invalid_handler"));
    }

    #[test]
    fn test_http_handlers_count_by_category() {
        assert_eq!(HttpHandlers::count_by_category("novel"), 6);
        assert_eq!(HttpHandlers::count_by_category("chapter"), 5);
        assert_eq!(HttpHandlers::count_by_category("run"), 2);
        assert_eq!(HttpHandlers::count_by_category("unknown"), 0);
    }

    #[test]
    fn test_http_handlers_total_count() {
        let total = HttpHandlers::count_by_category("novel")
            + HttpHandlers::count_by_category("chapter")
            + HttpHandlers::count_by_category("run");
        assert_eq!(total, 13);
    }

    #[test]
    fn test_error_handler_status_code() {
        let error = AppError::validation("test");
        assert_eq!(ErrorHandler::status_code(&error), 400);

        let error = AppError::not_found("test");
        assert_eq!(ErrorHandler::status_code(&error), 404);

        let error = AppError::conflict("test");
        assert_eq!(ErrorHandler::status_code(&error), 409);
    }

    #[test]
    fn test_error_handler_response() {
        let error = AppError::validation("Invalid input");
        let (status, response) = ErrorHandler::response(error);
        assert_eq!(status, 400);
        assert!(!response.is_empty());
    }

    #[test]
    fn test_error_handler_not_found() {
        let error = AppError::not_found("Resource not found");
        let (status, _response) = ErrorHandler::response(error);
        assert_eq!(status, 404);
    }

    #[test]
    fn test_error_handler_conflict() {
        let error = AppError::conflict("Conflict");
        let (status, _response) = ErrorHandler::response(error);
        assert_eq!(status, 409);
    }

    #[test]
    fn test_error_handler_integrity() {
        let error = AppError::integrity("Integrity error");
        let (status, _response) = ErrorHandler::response(error);
        assert_eq!(status, 500);
    }

    #[test]
    fn test_all_handlers_accounted_for() {
        let handlers = HttpHandlers::list();
        let expected = vec![
            "create_novel",
            "list_novels",
            "get_novel",
            "update_novel",
            "delete_novel",
            "set_creation_experience",
            "create_chapter",
            "list_chapters",
            "get_chapter",
            "save_chapter",
            "promote_chapter",
            "start_run",
            "approve_run",
        ];
        assert_eq!(handlers.len(), expected.len());
        for handler in expected {
            assert!(handlers.contains(&handler), "Missing handler: {}", handler);
        }
    }
}
