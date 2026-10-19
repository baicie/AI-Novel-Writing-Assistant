# Task 4.3 - 认证与授权详细设计

**日期**: 2026-10-20  
**阶段**: Phase 4.0 - 实现与集成  
**任务**: Task 4.3 - 认证与授权  
**目标**: 实现用户认证系统和权限控制

---

## 任务概述

### 目标

添加用户身份认证和权限控制，保护 API 端点，支持多用户场景。

### 认证方案选择

| 方案 | 安全性 | 性能 | 易用性 | 扩展性 |
|-----|-------|------|-------|--------|
| **JWT** | ✅ 高 | ✅ 高 | ✅ 中 | ✅ 高 |
| Session | ✅ 高 | ⚠️ 中 | ✅ 高 | ❌ 低 |
| OAuth 2.0 | ⭐⭐ | ⚠️ 中 | ❌ 低 | ⭐⭐ |
| API Key | ⚠️ 中 | ✅ 高 | ✅ 高 | ❌ 低 |

**选择理由**: JWT 无状态、性能优、适合微服务、与异步框架配合好

---

## Task 4.3 分解

### Part 1: JWT 实现 (1-2 天)

#### 1.1 JWT 依赖

```toml
[dependencies]
jsonwebtoken = "9.2"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.6", features = ["v4", "serde"] }
```

#### 1.2 JWT 配置

**文件**: `src/auth/jwt_config.rs`

```rust
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use chrono::{Duration, Utc};
use std::env;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,              // 用户 ID
    pub email: String,            // 用户邮箱
    pub exp: i64,                 // 过期时间
    pub iat: i64,                 // 签发时间
    pub role: String,             // 用户角色
}

pub struct JwtConfig {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    token_expiry: Duration,
}

impl JwtConfig {
    pub fn new() -> Result<Self, String> {
        let secret = env::var("JWT_SECRET")
            .unwrap_or_else(|_| "your-secret-key".to_string());
        
        Ok(JwtConfig {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            token_expiry: Duration::hours(24),
        })
    }

    pub fn generate_token(&self, user_id: &str, email: &str, role: &str) -> Result<String, String> {
        let now = Utc::now();
        let claims = Claims {
            sub: user_id.to_string(),
            email: email.to_string(),
            exp: (now + self.token_expiry).timestamp(),
            iat: now.timestamp(),
            role: role.to_string(),
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| format!("Token generation failed: {}", e))
    }

    pub fn verify_token(&self, token: &str) -> Result<Claims, String> {
        decode::<Claims>(token, &self.decoding_key, &Validation::default())
            .map(|d| d.claims)
            .map_err(|e| format!("Token verification failed: {}", e))
    }
}
```

**预期**: ~80 行代码

#### 1.3 Token 端点

**文件**: `src/auth/handlers.rs`

```rust
use actix_web::{web, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use crate::auth::JwtConfig;
use crate::repository::UserRepository;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub expires_in: i64,
}

pub async fn login(
    req: web::Json<LoginRequest>,
    jwt: web::Data<JwtConfig>,
    user_repo: web::Data<UserRepository>,
) -> Result<HttpResponse> {
    // 验证用户邮箱和密码
    match user_repo.find_by_email(&req.email).await {
        Ok(Some(user)) => {
            // 验证密码（实际应使用 bcrypt）
            if verify_password(&req.password, &user.password_hash)? {
                // 生成 JWT
                match jwt.generate_token(&user.id, &user.email, &user.role) {
                    Ok(token) => {
                        Ok(HttpResponse::Ok().json(LoginResponse {
                            token,
                            expires_in: 86400, // 24 小时
                        }))
                    }
                    Err(e) => {
                        Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                            "error": e
                        })))
                    }
                }
            } else {
                Ok(HttpResponse::Unauthorized().json(serde_json::json!({
                    "error": "Invalid credentials"
                })))
            }
        }
        Ok(None) => {
            Ok(HttpResponse::Unauthorized().json(serde_json::json!({
                "error": "User not found"
            })))
        }
        Err(e) => {
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string()
            })))
        }
    }
}

fn verify_password(password: &str, hash: &str) -> Result<bool> {
    // 使用 bcrypt 验证密码
    // bcrypt::verify(password, hash).map_err(|_| ...)
    Ok(true) // 临时实现
}
```

**预期**: ~70 行代码

---

### Part 2: 用户管理 (0.5-1 天)

#### 2.1 用户模型

**文件**: `src/models/user.rs`

```rust
use sqlx::FromRow;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: String,
    pub password_hash: String,
    pub role: String,                // admin, user, guest
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub name: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}
```

**预期**: ~40 行代码

#### 2.2 用户 Repository

```rust
pub async fn find_by_email(&self, email: &str) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE email = ?"
    )
    .bind(email)
    .fetch_optional(&self.pool)
    .await
}

pub async fn create(&self, user: &User) -> Result<User, sqlx::Error> {
    sqlx::query_as::<_, User>(
        "INSERT INTO users (id, email, name, password_hash, role, is_active, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         RETURNING *"
    )
    .bind(&user.id)
    .bind(&user.email)
    .bind(&user.name)
    .bind(&user.password_hash)
    .bind(&user.role)
    .bind(user.is_active)
    .bind(chrono::Utc::now())
    .bind(chrono::Utc::now())
    .fetch_one(&self.pool)
    .await
}
```

**预期**: ~50 行代码

---

### Part 3: 权限检查中间件 (1-2 天)

#### 3.1 认证中间件

**文件**: `src/auth/middleware.rs`

```rust
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage,
};
use futures::future::LocalBoxFuture;
use crate::auth::JwtConfig;

pub struct AuthMiddleware {
    jwt_config: JwtConfig,
}

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = AuthMiddlewareService<S>;
    type InitError = ();
    type Future = std::future::Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        std::future::ready(Ok(AuthMiddlewareService {
            service,
            jwt_config: self.jwt_config.clone(),
        }))
    }
}

pub struct AuthMiddlewareService<S> {
    service: S,
    jwt_config: JwtConfig,
}

impl<S, B> Service<ServiceRequest> for AuthMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // 从 Authorization 头提取 token
        let token = req
            .headers()
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .map(|s| s.to_string());

        match token {
            Some(token) => {
                match self.jwt_config.verify_token(&token) {
                    Ok(claims) => {
                        // 将 claims 存储在请求扩展中
                        req.extensions_mut().insert(claims);
                        
                        let fut = self.service.call(req);
                        Box::pin(async move {
                            let res = fut.await?;
                            Ok(res)
                        })
                    }
                    Err(_) => {
                        Box::pin(async move {
                            Err(actix_web::error::ErrorUnauthorized("Invalid token"))
                        })
                    }
                }
            }
            None => {
                Box::pin(async move {
                    Err(actix_web::error::ErrorUnauthorized("Missing token"))
                })
            }
        }
    }
}
```

**预期**: ~90 行代码

#### 3.2 角色检查

```rust
pub fn require_role(required_role: &str) -> impl Fn(HttpRequest) -> Result<(), Error> {
    let required = required_role.to_string();
    move |req: HttpRequest| {
        if let Some(claims) = req.extensions().get::<Claims>() {
            if claims.role == required || claims.role == "admin" {
                Ok(())
            } else {
                Err(actix_web::error::ErrorForbidden("Insufficient permissions"))
            }
        } else {
            Err(actix_web::error::ErrorUnauthorized("No credentials"))
        }
    }
}
```

**预期**: ~20 行代码

---

### Part 4: 受保护端点 (1-2 天)

#### 4.1 修改现有端点

原有的端点需要添加认证检查：

```rust
pub async fn create_novel(
    req: web::Json<CreateNovelRequest>,
    http_req: HttpRequest,
) -> Result<HttpResponse> {
    // 验证认证
    let claims = http_req
        .extensions()
        .get::<Claims>()
        .ok_or_else(|| actix_web::error::ErrorUnauthorized("Not authenticated"))?
        .clone();

    // 后续处理...
}
```

**预期**: 修改所有 13 个端点

#### 4.2 用户端点

```rust
// 登录
pub async fn login(req: web::Json<LoginRequest>) -> Result<HttpResponse> {
    // JWT 令牌生成
}

// 注册
pub async fn register(req: web::Json<RegisterRequest>) -> Result<HttpResponse> {
    // 用户创建 + 密码哈希
}

// 获取当前用户
pub async fn get_current_user(http_req: HttpRequest) -> Result<HttpResponse> {
    // 返回当前用户信息
}

// 刷新 token
pub async fn refresh_token(http_req: HttpRequest) -> Result<HttpResponse> {
    // 验证当前 token，生成新 token
}
```

**预期**: ~60 行代码

---

### Part 5: 会话管理 (0.5-1 天)

#### 5.1 会话表

```sql
CREATE TABLE sessions (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  token TEXT NOT NULL,
  expires_at TIMESTAMP NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);
```

#### 5.2 会话 Repository

```rust
pub async fn create_session(&self, user_id: &str, token: &str, expires_at: DateTime<Utc>) -> Result<Session, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query_as::<_, Session>(
        "INSERT INTO sessions (id, user_id, token, expires_at)
         VALUES (?, ?, ?, ?)
         RETURNING *"
    )
    .bind(id)
    .bind(user_id)
    .bind(token)
    .bind(expires_at)
    .fetch_one(&self.pool)
    .await
}

pub async fn revoke_session(&self, token: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM sessions WHERE token = ?")
        .bind(token)
        .execute(&self.pool)
        .await?;
    Ok(())
}

pub async fn cleanup_expired(&self) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM sessions WHERE expires_at < CURRENT_TIMESTAMP")
        .execute(&self.pool)
        .await?;
    Ok(())
}
```

**预期**: ~40 行代码

---

### Part 6: 测试 (1-2 天)

#### 6.1 JWT 测试

```rust
#[test]
fn test_generate_token() {
    let config = JwtConfig::new().unwrap();
    let token = config.generate_token("user-1", "user@example.com", "user").unwrap();
    assert!(!token.is_empty());
}

#[test]
fn test_verify_token() {
    let config = JwtConfig::new().unwrap();
    let token = config.generate_token("user-1", "user@example.com", "user").unwrap();
    let claims = config.verify_token(&token).unwrap();
    assert_eq!(claims.sub, "user-1");
    assert_eq!(claims.email, "user@example.com");
}

#[test]
fn test_invalid_token() {
    let config = JwtConfig::new().unwrap();
    let result = config.verify_token("invalid.token.here");
    assert!(result.is_err());
}
```

**预期**: 10+ JWT 测试

#### 6.2 认证端点测试

```rust
#[actix_web::test]
async fn test_login_success() {
    // 创建用户
    // 登录
    // 验证返回 token
}

#[actix_web::test]
async fn test_login_invalid_credentials() {
    // 验证错误凭证返回 401
}

#[actix_web::test]
async fn test_protected_endpoint_without_token() {
    // 验证无 token 返回 401
}

#[actix_web::test]
async fn test_protected_endpoint_with_token() {
    // 验证有 token 可访问
}
```

**预期**: 10+ 认证测试

---

### Part 7: 文档 (0.5 天)

- [ ] JWT 使用说明
- [ ] 认证流程文档
- [ ] 端点保护说明
- [ ] 错误代码文档

**预期**: 2-3 页文档

---

## 完整时间表

```
Day 1-2: JWT 实现 + 用户管理
  • JWT 配置
  • Token 生成和验证
  • 用户模型和 Repository
  
Day 3-4: 权限检查
  • 认证中间件
  • 角色检查
  • 受保护端点修改

Day 5: 会话管理
  • 会话表设计
  • 会话 Repository
  • 清理任务

Day 6-7: 测试和文档
  • JWT 测试
  • 认证端点测试
  • 文档编写
```

**总计**: 4-6 工作天

---

## 成功标准

- [ ] JWT 实现完整
- [ ] 登录端点工作
- [ ] 所有端点受保护
- [ ] 权限检查工作
- [ ] 20+ 认证测试通过
- [ ] 文档完整

---

**预计开始**: 2026-11-12  
**预计完成**: 2026-11-18
