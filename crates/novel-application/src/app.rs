//! 应用程序集成模块
//!
//! 提供应用程序的初始化、配置和生命周期管理。

use crate::error::AppError;
use std::collections::HashMap;

// ============================================================================
// 应用配置
// ============================================================================

/// 应用程序配置
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// 服务器主机
    pub host: String,
    /// 服务器端口
    pub port: u16,
    /// 数据库 URL
    pub database_url: String,
    /// 环境名称
    pub environment: String,
    /// 日志级别
    pub log_level: String,
    /// 其他配置选项
    pub extra: HashMap<String, String>,
}

impl AppConfig {
    /// 从环境变量读取配置
    pub fn from_env() -> Result<Self, AppError> {
        Ok(Self {
            host: std::env::var("APP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: std::env::var("APP_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite::memory:".to_string()),
            environment: std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string()),
            log_level: std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            extra: HashMap::new(),
        })
    }

    /// 创建默认开发配置
    pub fn development() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            database_url: "sqlite::memory:".to_string(),
            environment: "development".to_string(),
            log_level: "debug".to_string(),
            extra: HashMap::new(),
        }
    }

    /// 创建默认测试配置
    pub fn testing() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8081,
            database_url: "sqlite::memory:".to_string(),
            environment: "testing".to_string(),
            log_level: "debug".to_string(),
            extra: HashMap::new(),
        }
    }

    /// 创建默认生产配置
    pub fn production() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            database_url: "postgres://localhost/novel".to_string(),
            environment: "production".to_string(),
            log_level: "info".to_string(),
            extra: HashMap::new(),
        }
    }

    /// 验证配置
    pub fn validate(&self) -> Result<(), AppError> {
        if self.host.is_empty() {
            return Err(AppError::validation("host cannot be empty"));
        }
        if self.port == 0 {
            return Err(AppError::validation("port must be greater than 0"));
        }
        if self.database_url.is_empty() {
            return Err(AppError::validation("database_url cannot be empty"));
        }
        Ok(())
    }

    /// 获取配置概览
    pub fn summary(&self) -> String {
        format!(
            "Config({}:{} @ {} [{}])",
            self.host, self.port, self.environment, self.log_level
        )
    }
}

// ============================================================================
// 应用上下文
// ============================================================================

/// 应用程序上下文
#[derive(Debug, Clone)]
pub struct AppContext {
    /// 应用配置
    pub config: AppConfig,
    /// 应用状态
    pub state: AppState,
    /// 应用元数据
    pub metadata: AppMetadata,
}

/// 应用状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    /// 初始化中
    Initializing,
    /// 运行中
    Running,
    /// 停止中
    Stopping,
    /// 已停止
    Stopped,
    /// 错误状态
    Error,
}

impl std::fmt::Display for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppState::Initializing => write!(f, "initializing"),
            AppState::Running => write!(f, "running"),
            AppState::Stopping => write!(f, "stopping"),
            AppState::Stopped => write!(f, "stopped"),
            AppState::Error => write!(f, "error"),
        }
    }
}

/// 应用元数据
#[derive(Debug, Clone)]
pub struct AppMetadata {
    /// 应用名称
    pub name: String,
    /// 应用版本
    pub version: String,
    /// 启动时间戳（秒）
    pub start_time: u64,
    /// 请求计数
    pub request_count: u64,
    /// 错误计数
    pub error_count: u64,
}

impl AppMetadata {
    /// 创建新的应用元数据
    pub fn new() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            name: "AI-Novel-Writing-Assistant".to_string(),
            version: "3.5".to_string(),
            start_time,
            request_count: 0,
            error_count: 0,
        }
    }

    /// 增加请求计数
    pub fn increment_request_count(&mut self) {
        self.request_count += 1;
    }

    /// 增加错误计数
    pub fn increment_error_count(&mut self) {
        self.error_count += 1;
    }

    /// 获取运行时长（秒）
    pub fn uptime_seconds(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        current_time.saturating_sub(self.start_time)
    }
}

impl AppContext {
    /// 创建新的应用上下文
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            state: AppState::Initializing,
            metadata: AppMetadata::new(),
        }
    }

    /// 从配置初始化上下文
    pub fn from_config(config: AppConfig) -> Result<Self, AppError> {
        config.validate()?;
        Ok(Self::new(config))
    }

    /// 转换为运行状态
    pub fn mark_running(&mut self) {
        self.state = AppState::Running;
    }

    /// 转换为停止状态
    pub fn mark_stopping(&mut self) {
        self.state = AppState::Stopping;
    }

    /// 转换为已停止状态
    pub fn mark_stopped(&mut self) {
        self.state = AppState::Stopped;
    }

    /// 转换为错误状态
    pub fn mark_error(&mut self) {
        self.state = AppState::Error;
    }

    /// 检查应用是否正在运行
    pub fn is_running(&self) -> bool {
        self.state == AppState::Running
    }

    /// 获取应用状态信息
    pub fn status(&self) -> String {
        format!(
            "Status: {} | {} | Uptime: {}s | Requests: {} | Errors: {}",
            self.state,
            self.config.summary(),
            self.metadata.uptime_seconds(),
            self.metadata.request_count,
            self.metadata.error_count
        )
    }
}

// ============================================================================
// 应用初始化
// ============================================================================

/// 应用程序初始化器
pub struct AppInitializer;

impl AppInitializer {
    /// 初始化应用程序
    pub async fn initialize() -> Result<AppContext, AppError> {
        // 1. 读取配置
        let config = AppConfig::from_env()?;

        // 2. 验证配置
        config.validate()?;

        // 3. 创建上下文
        let ctx = AppContext::from_config(config)?;

        // 4. 初始化完成
        Ok(ctx)
    }

    /// 初始化（带指定配置）
    pub async fn initialize_with(config: AppConfig) -> Result<AppContext, AppError> {
        config.validate()?;
        let mut ctx = AppContext::from_config(config)?;
        ctx.mark_running();
        Ok(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_config_development() {
        let config = AppConfig::development();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8080);
        assert_eq!(config.environment, "development");
    }

    #[test]
    fn test_app_config_testing() {
        let config = AppConfig::testing();
        assert_eq!(config.environment, "testing");
        assert_eq!(config.port, 8081);
    }

    #[test]
    fn test_app_config_production() {
        let config = AppConfig::production();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.environment, "production");
    }

    #[test]
    fn test_app_config_validation_empty_host() {
        let config = AppConfig {
            host: "".to_string(),
            port: 8080,
            database_url: "sqlite::memory:".to_string(),
            environment: "test".to_string(),
            log_level: "info".to_string(),
            extra: HashMap::new(),
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_app_config_validation_zero_port() {
        let config = AppConfig {
            host: "localhost".to_string(),
            port: 0,
            database_url: "sqlite::memory:".to_string(),
            environment: "test".to_string(),
            log_level: "info".to_string(),
            extra: HashMap::new(),
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_app_config_validation_valid() {
        let config = AppConfig::development();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_app_config_summary() {
        let config = AppConfig::development();
        let summary = config.summary();
        assert!(summary.contains("127.0.0.1:8080"));
        assert!(summary.contains("development"));
    }

    #[test]
    fn test_app_state_display() {
        assert_eq!(AppState::Initializing.to_string(), "initializing");
        assert_eq!(AppState::Running.to_string(), "running");
        assert_eq!(AppState::Stopped.to_string(), "stopped");
    }

    #[test]
    fn test_app_context_new() {
        let config = AppConfig::development();
        let ctx = AppContext::new(config);
        assert_eq!(ctx.state, AppState::Initializing);
        assert_eq!(ctx.metadata.request_count, 0);
    }

    #[test]
    fn test_app_context_state_transitions() {
        let config = AppConfig::development();
        let mut ctx = AppContext::new(config);

        assert_eq!(ctx.state, AppState::Initializing);
        ctx.mark_running();
        assert_eq!(ctx.state, AppState::Running);
        assert!(ctx.is_running());

        ctx.mark_stopping();
        assert_eq!(ctx.state, AppState::Stopping);

        ctx.mark_stopped();
        assert_eq!(ctx.state, AppState::Stopped);
    }

    #[test]
    fn test_app_context_error_state() {
        let config = AppConfig::development();
        let mut ctx = AppContext::new(config);
        ctx.mark_error();
        assert_eq!(ctx.state, AppState::Error);
        assert!(!ctx.is_running());
    }

    #[test]
    fn test_app_metadata_increments() {
        let mut metadata = AppMetadata::new();
        assert_eq!(metadata.request_count, 0);
        assert_eq!(metadata.error_count, 0);

        metadata.increment_request_count();
        assert_eq!(metadata.request_count, 1);

        metadata.increment_error_count();
        assert_eq!(metadata.error_count, 1);
    }

    #[test]
    fn test_app_metadata_uptime() {
        let metadata = AppMetadata::new();
        assert!(metadata.uptime_seconds() <= 1); // 应该立即完成
    }

    #[test]
    fn test_app_context_status() {
        let config = AppConfig::development();
        let mut ctx = AppContext::new(config);
        ctx.mark_running();
        let status = ctx.status();
        assert!(status.contains("running"));
        assert!(status.contains("Requests: 0"));
    }

    #[tokio::test]
    async fn test_app_initializer_with_config() {
        let config = AppConfig::testing();
        let ctx = AppInitializer::initialize_with(config).await;
        assert!(ctx.is_ok());
        let ctx = ctx.unwrap();
        assert!(ctx.is_running());
    }

    #[tokio::test]
    async fn test_app_initializer_with_invalid_config() {
        let config = AppConfig {
            host: "".to_string(),
            port: 8080,
            database_url: "sqlite::memory:".to_string(),
            environment: "test".to_string(),
            log_level: "info".to_string(),
            extra: HashMap::new(),
        };
        let ctx = AppInitializer::initialize_with(config).await;
        assert!(ctx.is_err());
    }
}
