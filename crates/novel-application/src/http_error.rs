//! HTTP 层 - 错误映射
//! 
//! 将应用层错误映射到 HTTP 响应。

use crate::error::{AppError, AppErrorCode};
use crate::http_response::ErrorResponse;

// 注意：这个模块是概念性的设计
// 在实际使用中，需要根据具体的 Web 框架（actix-web, axum 等）进行适配

/// HTTP 状态码
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpStatusCode {
    OK = 200,
    Created = 201,
    BadRequest = 400,
    NotFound = 404,
    Conflict = 409,
    InternalServerError = 500,
}

impl HttpStatusCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpStatusCode::OK => "200",
            HttpStatusCode::Created => "201",
            HttpStatusCode::BadRequest => "400",
            HttpStatusCode::NotFound => "404",
            HttpStatusCode::Conflict => "409",
            HttpStatusCode::InternalServerError => "500",
        }
    }
}

/// HTTP 错误映射结果
#[derive(Debug, Clone)]
pub struct HttpErrorResponse {
    pub status_code: HttpStatusCode,
    pub error_response: ErrorResponse,
}

impl HttpErrorResponse {
    pub fn new(status_code: HttpStatusCode, error_response: ErrorResponse) -> Self {
        Self {
            status_code,
            error_response,
        }
    }
}

/// 将应用错误映射到 HTTP 响应
pub fn map_error_to_http(error: AppError) -> HttpErrorResponse {
    let (status_code, error_code) = match error.code {
        AppErrorCode::Validation => (HttpStatusCode::BadRequest, "VALIDATION"),
        AppErrorCode::NotFound => (HttpStatusCode::NotFound, "NOT_FOUND"),
        AppErrorCode::Conflict => (HttpStatusCode::Conflict, "CONFLICT"),
        AppErrorCode::IntegrityError => (HttpStatusCode::InternalServerError, "INTEGRITY_ERROR"),
        AppErrorCode::Cancelled => (HttpStatusCode::BadRequest, "CANCELLED"),
        AppErrorCode::ModelRateLimit => (HttpStatusCode::InternalServerError, "RATE_LIMIT"),
        AppErrorCode::StorageCorrupt => (HttpStatusCode::InternalServerError, "STORAGE_ERROR"),
    };

    let error_response = ErrorResponse::new(error_code, &error.message);
    HttpErrorResponse::new(status_code, error_response)
}

/// HTTP 响应包装
#[derive(Debug, Clone)]
pub struct HttpResponse<T> {
    pub status_code: HttpStatusCode,
    pub data: Option<T>,
    pub error: Option<ErrorResponse>,
}

impl<T> HttpResponse<T> {
    /// 成功响应 (200 OK)
    pub fn ok(data: T) -> Self {
        Self {
            status_code: HttpStatusCode::OK,
            data: Some(data),
            error: None,
        }
    }

    /// 创建响应 (201 Created)
    pub fn created(data: T) -> Self {
        Self {
            status_code: HttpStatusCode::Created,
            data: Some(data),
            error: None,
        }
    }

    /// 错误响应
    pub fn error(status_code: HttpStatusCode, error: ErrorResponse) -> Self {
        Self {
            status_code,
            data: None,
            error: Some(error),
        }
    }

    /// 从应用错误生成 HTTP 响应
    pub fn from_app_error(app_error: AppError) -> Self {
        let http_error = map_error_to_http(app_error);
        Self {
            status_code: http_error.status_code,
            data: None,
            error: Some(http_error.error_response),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_status_code_validation() {
        assert_eq!(HttpStatusCode::OK.as_str(), "200");
        assert_eq!(HttpStatusCode::Created.as_str(), "201");
        assert_eq!(HttpStatusCode::BadRequest.as_str(), "400");
        assert_eq!(HttpStatusCode::NotFound.as_str(), "404");
        assert_eq!(HttpStatusCode::Conflict.as_str(), "409");
        assert_eq!(HttpStatusCode::InternalServerError.as_str(), "500");
    }

    #[test]
    fn test_map_validation_error() {
        let app_error = AppError::validation("Invalid input");
        let http_error = map_error_to_http(app_error);
        
        assert_eq!(http_error.status_code, HttpStatusCode::BadRequest);
        assert_eq!(http_error.error_response.code, "VALIDATION");
    }

    #[test]
    fn test_map_not_found_error() {
        let app_error = AppError::not_found("Novel not found");
        let http_error = map_error_to_http(app_error);
        
        assert_eq!(http_error.status_code, HttpStatusCode::NotFound);
        assert_eq!(http_error.error_response.code, "NOT_FOUND");
    }

    #[test]
    fn test_map_conflict_error() {
        let app_error = AppError::conflict("Chapter already exists");
        let http_error = map_error_to_http(app_error);
        
        assert_eq!(http_error.status_code, HttpStatusCode::Conflict);
        assert_eq!(http_error.error_response.code, "CONFLICT");
    }

    #[test]
    fn test_map_integrity_error() {
        let app_error = AppError::integrity("Data integrity issue");
        let http_error = map_error_to_http(app_error);
        
        assert_eq!(http_error.status_code, HttpStatusCode::InternalServerError);
        assert_eq!(http_error.error_response.code, "INTEGRITY_ERROR");
    }

    #[test]
    fn test_http_response_ok() {
        #[derive(Debug, Clone, PartialEq)]
        struct TestData {
            value: String,
        }

        let data = TestData {
            value: "test".to_string(),
        };
        let response = HttpResponse::ok(data.clone());
        
        assert_eq!(response.status_code, HttpStatusCode::OK);
        assert_eq!(response.data, Some(data));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_http_response_created() {
        #[derive(Debug, Clone, PartialEq)]
        struct TestData {
            value: String,
        }

        let data = TestData {
            value: "created".to_string(),
        };
        let response = HttpResponse::created(data.clone());
        
        assert_eq!(response.status_code, HttpStatusCode::Created);
        assert_eq!(response.data, Some(data));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_http_response_from_app_error() {
        let app_error = AppError::validation("Invalid input");
        let response: HttpResponse<String> = HttpResponse::from_app_error(app_error);
        
        assert_eq!(response.status_code, HttpStatusCode::BadRequest);
        assert!(response.data.is_none());
        assert!(response.error.is_some());
    }

    #[test]
    fn test_http_error_response_structure() {
        let error_response = ErrorResponse::new("VALIDATION", "Invalid input")
            .with_details("Field: title");
        let http_error = HttpErrorResponse::new(HttpStatusCode::BadRequest, error_response);
        
        assert_eq!(http_error.status_code, HttpStatusCode::BadRequest);
        assert_eq!(http_error.error_response.code, "VALIDATION");
        assert_eq!(http_error.error_response.message, "Invalid input");
        assert_eq!(http_error.error_response.details, Some("Field: title".to_string()));
    }

    #[test]
    fn test_error_message_preservation() {
        let app_error = AppError::not_found("Novel ID: 123 not found");
        let http_error = map_error_to_http(app_error);
        
        assert_eq!(http_error.error_response.message, "Novel ID: 123 not found");
    }

    #[test]
    fn test_retryable_status_codes() {
        // 冲突错误是可重试的，但 HTTP 409 不是重试状态
        let app_error = AppError::conflict("Chapter version conflict");
        let http_error = map_error_to_http(app_error);
        
        assert_eq!(http_error.status_code, HttpStatusCode::Conflict);
        // 实际的重试逻辑应该由客户端根据业务规则决定
    }
}
