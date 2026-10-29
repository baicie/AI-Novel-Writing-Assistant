# Task 4.3 - 认证与授权 实现路线图

**版本**: 1.0 | **状态**: 🚀 **计划中**  
**计划**: 2026-11-05 ~ 2026-11-09 | **工作量**: 3-4 天

---

## 一、核心目标

### 1.1 功能目标

- ✅ JWT 令牌生成和验证
- ✅ Bcrypt 密码加密
- ✅ 用户认证流程
- ✅ 角色和权限管理
- ✅ 令牌刷新机制

### 1.2 质量目标

| 指标 | 目标 |
|-----|------|
| **端点数** | 5 个 (auth 相关) |
| **测试用例** | 23+ 个 |
| **代码行数** | ~350 行 |
| **代码覆盖率** | > 85% |
| **安全审计** | 0 漏洞 |

---

## 二、认证系统设计

### 2.1 3 个主要组件

#### 1. JWT 令牌管理 (TokenService)
```rust
pub struct TokenService {
    secret: String,
    expiry: Duration,
}

impl TokenService {
    pub fn generate(&self, user_id: Uuid, role: UserRole) -> Result<Token> {
        // 创建 JWT 令牌
    }

    pub fn verify(&self, token: &str) -> Result<TokenClaims> {
        // 验证令牌有效性
    }

    pub fn refresh(&self, token: &str) -> Result<Token> {
        // 刷新令牌
    }
}
```

**代码行数**: ~80 行

#### 2. 密码加密服务 (PasswordService)
```rust
pub struct PasswordService;

impl PasswordService {
    pub fn hash(password: &str) -> Result<String> {
        // Bcrypt 加密
    }

    pub fn verify(password: &str, hash: &str) -> Result<bool> {
        // 验证密码
    }
}
```

**代码行数**: ~40 行

#### 3. 认证中间件 (AuthMiddleware)
```rust
pub async fn auth_middleware(
    req: Request,
    next: Next,
) -> Result<Response> {
    // 1. 从请求头提取令牌
    // 2. 验证令牌
    // 3. 注入用户信息到 request context
}
```

**代码行数**: ~60 行

### 2.2 5 个认证端点

| 端点 | 方法 | 功能 | 测试 |
|-----|------|------|------|
| /auth/register | POST | 用户注册 | 4 个 |
| /auth/login | POST | 用户登录 | 4 个 |
| /auth/refresh | POST | 刷新令牌 | 3 个 |
| /auth/verify | POST | 验证令牌 | 3 个 |
| /auth/logout | POST | 登出 | 2 个 |

---

## 三、数据库设计

### 3.1 新增表

```sql
CREATE TABLE users (
    id TEXT PRIMARY KEY,
    username TEXT UNIQUE NOT NULL,
    email TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT DEFAULT 'user',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_username (username)
);

CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    token TEXT NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id),
    INDEX idx_user (user_id),
    INDEX idx_expires (expires_at)
);
```

---

## 四、实现计划

### Day 1 (2026-11-05)

**目标**: TokenService + PasswordService

- [ ] TokenService 实现 (JWT 生成/验证/刷新) - 1.5 小时
- [ ] PasswordService 实现 (Bcrypt) - 1 小时
- [ ] 单元测试 (8 个) - 1.5 小时
- [ ] 代码审查和优化 - 1 小时

**代码行数**: ~120 行 | **测试**: 8 个 ✓

---

### Day 2 (2026-11-06)

**目标**: 认证端点实现

- [ ] AuthMiddleware 实现 - 1 小时
- [ ] 5 个认证端点实现 - 2.5 小时
- [ ] 单元测试 (15 个) - 1.5 小时

**代码行数**: ~230 行 | **测试**: 15 个 ✓

---

### Day 3 (2026-11-07)

**目标**: 集成测试和安全审计

- [ ] 集成测试 (多端点流程) - 1.5 小时
- [ ] 安全审计 (常见漏洞检查) - 1 小时
- [ ] 性能验证 - 1 小时
- [ ] 文档完成 - 1 小时

**总计**: 23+ 个测试通过 ✓

---

## 五、安全检查清单

```
□ SQL 注入防护 (参数化查询)
□ 暴力破解防护 (速率限制)
□ CSRF 防护 (令牌验证)
□ 密码策略 (最小长度、复杂性)
□ 令牌过期管理
□ HTTPS 强制
□ 安全头部设置
```

---

## 完成标准

```
✅ 5 个认证端点全部实现
✅ 23+ 个测试全部通过
✅ 代码覆盖率 > 85%
✅ 安全审计 0 漏洞
✅ 密码加密正确使用
✅ JWT 令牌验证完整
```

**启动**: 2026-11-05 | **完成**: 2026-11-09

