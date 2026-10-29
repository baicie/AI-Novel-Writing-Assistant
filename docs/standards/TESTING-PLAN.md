# Phase 4.0 - 测试计划和框架

**版本**: 1.0  
**日期**: 2026-10-20  
**范围**: Phase 4.0 全部 5 个 Task

---

## 一、测试总体战略

### 1.1 测试金字塔

```
        /\
       /  \      E2E 测试 (10%)
      /    \     - 完整用户工作流
     /______\    - 真实数据库
    /        \   - 真实 HTTP 调用
   /  集成   \   集成测试 (30%)
  /测试      \   - 多模块交互
 /____________\  - 真实依赖 (部分 mock)
/            \   单元测试 (60%)
/ 单元测试   \    - 单一函数/方法
_____________\   - 模拟依赖
```

### 1.2 测试类型和覆盖目标

| 测试类型 | 数量 | 覆盖率 | 执行时间 |
|---------|------|--------|---------|
| 单元测试 | 40+ | > 90% | < 10s |
| 集成测试 | 15+ | > 80% | < 10s |
| E2E 测试 | 10+ | > 70% | < 15s |
| **总计** | **65+** | **> 85%** | **< 35s** |

### 1.3 质量门控

所有代码提交前必须：
- ✅ 所有测试通过 (100%)
- ✅ 代码覆盖率 > 85%
- ✅ 无编译警告
- ✅ clippy 检查通过
- ✅ 代码格式化

---

## 二、Task 4.1 - Web 框架测试计划

### 2.1 端点测试计算

Task 4.1 实现 13 个端点：

```
POST   /novels                 # 创建小说
GET    /novels/{id}            # 获取小说
GET    /novels                 # 列表小说
PUT    /novels/{id}            # 更新小说
DELETE /novels/{id}            # 删除小说

POST   /novels/{id}/chapters   # 创建章节
GET    /novels/{id}/chapters   # 列表章节
PUT    /chapters/{id}          # 更新章节
DELETE /chapters/{id}          # 删除章节

POST   /novels/{id}/runs       # 执行运行
GET    /novels/{id}/runs       # 列表运行
GET    /runs/{id}              # 获取运行
DELETE /runs/{id}              # 删除运行
```

### 2.2 单元测试 (15+ 个)

每个端点需要以下测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test;

    #[tokio::test]
    async fn test_create_novel_success() {
        // 成功创建
    }

    #[tokio::test]
    async fn test_create_novel_invalid_title() {
        // 标题无效
    }

    #[tokio::test]
    async fn test_create_novel_duplicate() {
        // 重复创建
    }

    #[tokio::test]
    async fn test_get_novel_success() {
        // 成功获取
    }

    #[tokio::test]
    async fn test_get_novel_not_found() {
        // 不存在
    }
}
```

### 2.3 集成测试 (8+ 个)

测试端点之间的交互：

```rust
#[cfg(test)]
mod integration_tests {
    #[tokio::test]
    async fn test_create_and_retrieve_novel() {
        // 创建 -> 获取 -> 验证
    }

    #[tokio::test]
    async fn test_create_novel_with_chapters() {
        // 创建小说 -> 添加章节 -> 验证
    }

    #[tokio::test]
    async fn test_full_workflow() {
        // 完整工作流测试
    }
}
```

### 2.4 E2E 测试 (3+ 个)

测试完整的 HTTP 工作流：

```rust
#[tokio::test]
async fn test_e2e_novel_creation() {
    // 启动服务器
    // 发送 HTTP 请求
    // 验证响应
    // 验证数据库
}

#[tokio::test]
async fn test_e2e_novel_workflow() {
    // 创建 -> 列表 -> 更新 -> 删除
}
```

### 2.5 错误处理测试 (5+ 个)

```rust
#[tokio::test]
async fn test_invalid_json_request() {
    // 格式错误的 JSON
}

#[tokio::test]
async fn test_missing_required_fields() {
    // 缺少必需字段
}

#[tokio::test]
async fn test_invalid_id_format() {
    // 无效的 ID 格式
}

#[tokio::test]
async fn test_unauthorized_access() {
    // 未授权访问
}

#[tokio::test]
async fn test_rate_limiting() {
    // 速率限制
}
```

### 2.6 中间件测试 (3+ 个)

```rust
#[tokio::test]
async fn test_logging_middleware() {
    // 验证日志记录
}

#[tokio::test]
async fn test_cors_middleware() {
    // 验证 CORS 头
}

#[tokio::test]
async fn test_error_middleware() {
    // 验证错误处理
}
```

**Task 4.1 总测试数**: 35+ 个

---

## 三、Task 4.2 - 数据库测试计划

### 3.1 Repository 测试 (20+ 个)

每个 Repository 需要以下测试：

#### NovelRepository

```rust
#[tokio::test]
async fn test_create_novel() {
    // 创建小说
}

#[tokio::test]
async fn test_get_novel_by_id() {
    // 按 ID 获取
}

#[tokio::test]
async fn test_list_novels() {
    // 列表所有
}

#[tokio::test]
async fn test_update_novel() {
    // 更新小说
}

#[tokio::test]
async fn test_delete_novel() {
    // 删除小说
}

#[tokio::test]
async fn test_find_novels_by_author() {
    // 按作者查找
}
```

#### ChapterRepository

```rust
#[tokio::test]
async fn test_create_chapter() {}

#[tokio::test]
async fn test_get_chapters_for_novel() {}

#[tokio::test]
async fn test_update_chapter_order() {}

#[tokio::test]
async fn test_delete_chapter() {}
```

#### RunRepository

```rust
#[tokio::test]
async fn test_create_run() {}

#[tokio::test]
async fn test_get_run_result() {}

#[tokio::test]
async fn test_list_runs_for_novel() {}

#[tokio::test]
async fn test_update_run_status() {}
```

### 3.2 数据库完整性测试 (5+ 个)

```rust
#[tokio::test]
async fn test_foreign_key_constraints() {
    // 验证外键约束
}

#[tokio::test]
async fn test_unique_constraints() {
    // 验证唯一约束
}

#[tokio::test]
async fn test_transaction_rollback() {
    // 验证事务回滚
}

#[tokio::test]
async fn test_index_effectiveness() {
    // 验证索引性能
}

#[tokio::test]
async fn test_concurrent_writes() {
    // 验证并发写入
}
```

### 3.3 迁移测试 (3+ 个)

```rust
#[tokio::test]
async fn test_initial_migration() {
    // 初始迁移成功
}

#[tokio::test]
async fn test_migration_idempotency() {
    // 迁移幂等性
}

#[tokio::test]
async fn test_rollback_migration() {
    // 迁移回滚
}
```

**Task 4.2 总测试数**: 28+ 个

---

## 四、Task 4.3 - 认证与授权测试计划

### 4.1 JWT 测试 (8+ 个)

```rust
#[test]
fn test_jwt_token_generation() {
    // 生成有效的 token
}

#[test]
fn test_jwt_token_expiration() {
    // 验证过期时间
}

#[test]
fn test_jwt_invalid_signature() {
    // 验证无效签名
}

#[test]
fn test_jwt_malformed_token() {
    // 验证格式错误
}

#[test]
fn test_jwt_claims_extraction() {
    // 提取 claims
}

#[test]
fn test_jwt_refresh_token() {
    // 刷新 token
}

#[test]
fn test_jwt_key_rotation() {
    // 密钥轮换
}

#[test]
fn test_jwt_algorithm_mismatch() {
    // 算法不匹配
}
```

### 4.2 认证端点测试 (6+ 个)

```rust
#[tokio::test]
async fn test_login_success() {
    // 成功登录
}

#[tokio::test]
async fn test_login_invalid_credentials() {
    // 无效凭证
}

#[tokio::test]
async fn test_login_user_not_found() {
    // 用户不存在
}

#[tokio::test]
async fn test_register_success() {
    // 成功注册
}

#[tokio::test]
async fn test_register_duplicate_email() {
    // 重复邮箱
}

#[tokio::test]
async fn test_register_invalid_password() {
    // 无效密码
}
```

### 4.3 授权测试 (6+ 个)

```rust
#[tokio::test]
async fn test_require_authentication() {
    // 验证需要认证
}

#[tokio::test]
async fn test_require_specific_role() {
    // 验证需要特定角色
}

#[tokio::test]
async fn test_token_in_header() {
    // 验证 header 中的 token
}

#[tokio::test]
async fn test_token_in_cookie() {
    // 验证 cookie 中的 token
}

#[tokio::test]
async fn test_expired_token_rejected() {
    // 验证过期 token 拒绝
}

#[tokio::test]
async fn test_invalid_token_rejected() {
    // 验证无效 token 拒绝
}
```

### 4.4 密码测试 (3+ 个)

```rust
#[test]
fn test_bcrypt_hash_and_verify() {
    // bcrypt 哈希和验证
}

#[test]
fn test_bcrypt_different_hashes() {
    // 相同密码不同哈希
}

#[test]
fn test_bcrypt_weak_password() {
    // 弱密码检测
}
```

**Task 4.3 总测试数**: 23+ 个

---

## 五、Task 4.4 - 性能与监控测试计划

### 5.1 Prometheus 指标测试 (8+ 个)

```rust
#[tokio::test]
async fn test_http_requests_total_counter() {
    // 验证请求计数
}

#[tokio::test]
async fn test_http_request_duration_histogram() {
    // 验证延迟直方图
}

#[tokio::test]
async fn test_http_request_size_histogram() {
    // 验证请求大小
}

#[tokio::test]
async fn test_http_response_size_histogram() {
    // 验证响应大小
}

#[tokio::test]
async fn test_active_connections_gauge() {
    // 验证活跃连接
}

#[tokio::test]
async fn test_database_query_duration() {
    // 验证数据库查询时间
}

#[tokio::test]
async fn test_cache_hit_rate() {
    // 验证缓存命中率
}

#[tokio::test]
async fn test_metrics_endpoint() {
    // 验证指标端点
}
```

### 5.2 缓存测试 (6+ 个)

```rust
#[tokio::test]
async fn test_cache_hit() {
    // 验证缓存命中
}

#[tokio::test]
async fn test_cache_miss() {
    // 验证缓存未命中
}

#[tokio::test]
async fn test_cache_expiration() {
    // 验证缓存过期
}

#[tokio::test]
async fn test_cache_eviction() {
    // 验证缓存驱逐
}

#[tokio::test]
async fn test_cache_concurrent_access() {
    // 验证并发访问
}

#[tokio::test]
async fn test_cache_invalidation() {
    // 验证缓存失效
}
```

### 5.3 性能基准测试 (4+ 个)

```rust
#[bench]
fn bench_create_novel(b: &mut Bencher) {
    // 创建小说基准
}

#[bench]
fn bench_get_novel(b: &mut Bencher) {
    // 获取小说基准
}

#[bench]
fn bench_list_novels(b: &mut Bencher) {
    // 列表基准
}

#[bench]
fn bench_database_query(b: &mut Bencher) {
    // 数据库查询基准
}
```

**Task 4.4 总测试数**: 18+ 个

---

## 六、Task 4.5 - 文档与部署测试计划

### 6.1 Docker 构建测试 (3+ 个)

```rust
#[test]
fn test_dockerfile_builds() {
    // Docker 镜像构建成功
}

#[test]
fn test_docker_compose_starts() {
    // docker-compose 启动成功
}

#[test]
fn test_container_health_check() {
    // 容器健康检查通过
}
```

### 6.2 CI/CD 流程测试 (3+ 个)

```
✅ 编译测试 - 代码编译成功
✅ 测试运行 - 所有测试通过
✅ 覆盖检查 - 覆盖率 > 85%
```

### 6.3 API 文档测试 (2+ 个)

```rust
#[test]
fn test_swagger_spec_valid() {
    // Swagger 规范有效
}

#[test]
fn test_api_endpoints_documented() {
    // 所有端点已文档化
}
```

**Task 4.5 总测试数**: 8+ 个

---

## 七、测试执行框架

### 7.1 单元测试模板

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct TestContext {
        // 测试数据
    }

    impl TestContext {
        fn new() -> Self {
            Self {
                // 初始化
            }
        }
    }

    #[test]
    fn test_scenario() {
        let ctx = TestContext::new();

        // Arrange: 准备
        let input = "test";

        // Act: 执行
        let result = function_under_test(input);

        // Assert: 验证
        assert!(result.is_ok());
    }
}
```

### 7.2 异步测试模板

```rust
#[tokio::test]
async fn test_async_scenario() {
    // Arrange
    let pool = setup_test_db().await;
    let repository = Repository::new(pool);

    // Act
    let result = repository.get_item("id").await;

    // Assert
    assert!(result.is_ok());
}
```

### 7.3 集成测试模板

```rust
#[tokio::test]
async fn test_integration() {
    // 初始化多个组件
    let db = setup_test_db().await;
    let repo = Repository::new(db);
    let service = Service::new(repo);

    // 测试服务级别的操作
    let result = service.complex_operation().await;

    // 验证结果
    assert!(result.is_ok());
}
```

---

## 八、测试工具和库

### 8.1 必需的 dev-dependencies

```toml
[dev-dependencies]
tokio-test = "0.4"              # 异步测试支持
mockall = "0.11"                # Mock 库
tempfile = "3.8"                # 临时文件
assert_matches = "1.5"          # 模式匹配断言
proptest = "1.2"                # 属性测试
criterion = "0.5"               # 基准测试
```

### 8.2 测试命令

```bash
# 运行所有测试
cargo test

# 仅运行单元测试
cargo test --lib

# 仅运行集成测试
cargo test --test '*'

# 运行特定测试
cargo test test_name

# 显示输出
cargo test -- --nocapture

# 单线程运行
cargo test -- --test-threads=1

# 生成覆盖报告
cargo tarpaulin --out Html

# 运行基准测试
cargo bench
```

---

## 九、测试覆盖率目标

### 9.1 按 Task 的覆盖率目标

| Task | 文件数 | 函数数 | 目标覆盖率 |
|------|--------|--------|-----------|
| 4.1 | 5 | 40+ | > 90% |
| 4.2 | 3 | 20+ | > 85% |
| 4.3 | 3 | 15+ | > 90% |
| 4.4 | 2 | 10+ | > 80% |
| 4.5 | 2 | 5+ | > 75% |
| **总计** | **15** | **90+** | **> 85%** |

### 9.2 覆盖率计算

```
覆盖率 = (已执行的代码行数 / 总代码行数) × 100%

目标:
- 总体: > 85%
- 关键路径: > 95%
- 错误处理: > 90%
```

---

## 十、持续集成检查

### 10.1 GitHub Actions 检查

```yaml
# 编译
- cargo build

# 测试
- cargo test --all

# 覆盖
- cargo tarpaulin --out Xml

# Linting
- cargo clippy -- -D warnings
- cargo fmt -- --check

# 安全审计
- cargo audit
```

### 10.2 本地验证

```bash
# 提交前检查
./scripts/pre-commit-check.sh

# 包括:
# - cargo fmt
# - cargo clippy
# - cargo test
# - cargo tarpaulin (快速)
```

---

## 十一、测试报告和指标

### 11.1 每次提交的报告

```
测试结果: 65/65 通过
覆盖率: 86.2%
执行时间: 32 秒
编译时间: 45 秒
```

### 11.2 周报告

```
本周测试统计:
- 新增测试: 20 个
- 修复的测试: 3 个
- 覆盖率趋势: ↑ 2.3%
- 平均执行时间: 31 秒
```

---

## 十二、测试失败处理

### 12.1 失败分类

- **编译失败**: 代码有问题，需要修复
- **测试失败**: 实现有问题，需要调试
- **覆盖率不足**: 需要添加更多测试
- **性能下降**: 需要优化代码

### 12.2 失败修复流程

```
1. 理解失败原因
   - 查看错误消息
   - 查看测试代码
   - 查看实现代码

2. 修复问题
   - 修改实现或测试
   - 确保逻辑正确
   - 验证边界条件

3. 验证修复
   - 运行失败的测试
   - 运行所有相关测试
   - 检查覆盖率

4. 提交修复
   - 添加测试改进
   - 更新文档
   - 提交 PR
```

---

## 十三、总体测试时间表

| 阶段 | Task | 预计测试数 | 时间 |
|------|------|-----------|------|
| Week 1 | 4.1 | 35+ | 2-3 天 |
| Week 2 | 4.2 | 28+ | 2-3 天 |
| Week 3 | 4.3 | 23+ | 2 天 |
| Week 4 | 4.4 | 18+ | 1-2 天 |
| Week 5 | 4.5 | 8+ | 0.5 天 |
| Week 6 | 集成和验证 | - | 2-3 天 |

---

## 总结

这个测试计划提供了：

✅ **完整的覆盖** - 从单元到 E2E  
✅ **明确的指标** - 65+ 测试，> 85% 覆盖  
✅ **标准化模板** - 易于遵循的测试结构  
✅ **自动化检查** - CI/CD 集成  
✅ **质量保证** - 高质量代码的保证  

**遵循这个计划，确保 Phase 4.0 的代码质量！**

---

**计划版本**: 1.0  
**最后更新**: 2026-10-20  
**执行**: Phase 4.0 实现期间
