# Rust 代码规范指南 - AI Novel Writing Assistant

**版本**: 1.0  
**日期**: 2026-10-20  
**适用范围**: Phase 4.0 及以后的所有代码

---

## 一、总体原则

### 1.1 核心价值观

- **清晰性**：代码应该像书一样易读
- **安全性**：利用 Rust 的类型系统保证内存安全
- **性能**：不牺牲清晰性的前提下追求性能
- **可维护性**：代码应该易于修改和扩展
- **一致性**：整个项目风格统一

### 1.2 设计原则

- **职责单一**: 每个函数/结构体只做一件事
- **显式优于隐式**: 优先选择明确的代码
- **错误处理**: 显式处理所有错误情况
- **文档完整**: 公有 API 必须有文档
- **测试驱动**: 关键逻辑先写测试

---

## 二、文件和模块组织

### 2.1 文件结构

```rust
// 标准文件头
//! Module documentation
//!
//! Brief description of what this module does.
//!
//! # Examples
//!
//! ```
//! // usage example
//! ```

// 导入
use crate::path::to::Item;
use std::collections::HashMap;

// 常量
const MAX_CONNECTIONS: usize = 100;

// 类型定义
pub type DbPool = SqlitePool;

// 结构体和实现
pub struct MyStruct {
    // fields
}

impl MyStruct {
    // methods
}

// 函数
pub fn my_function() {
    // body
}

// 测试
#[cfg(test)]
mod tests {
    // tests
}
```

### 2.2 模块命名

```rust
// 好: 清晰的模块名称
mod dto;
mod models;
mod services;
mod use_cases;

// 不好: 含混的模块名称
mod utils;      // 太宽泛
mod helpers;    // 太宽泛
mod common;     // 不清晰
```

### 2.3 导出规范

```rust
// lib.rs 或 mod.rs
pub mod dto;
pub mod models;
pub mod services;
pub mod use_cases;

pub use dto::*;
pub use models::*;

// 或者更精确的导出
pub use dto::NovelDto;
pub use models::Novel;
```

---

## 三、命名规范

### 3.1 类型名称

```rust
// 好: PascalCase
pub struct NovelData;
pub enum ChapterStatus;
pub trait Repository;

// 不好: snake_case
pub struct novel_data;
pub enum chapter_status;
```

### 3.2 函数和方法名称

```rust
// 好: snake_case
pub fn create_novel(title: &str) -> Result<Novel> {}
pub fn get_by_id(id: &str) -> Result<Novel> {}
pub fn update_status(id: &str, status: Status) -> Result<()> {}

// 不好: camelCase
pub fn createNovel(title: &str) -> Result<Novel> {}
pub fn getById(id: &str) -> Result<Novel> {}
```

### 3.3 常量命名

```rust
// 好: UPPER_SNAKE_CASE
const MAX_RETRIES: usize = 3;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const DB_CONNECTION_POOL_SIZE: usize = 10;

// 不好: 小写或混合
const max_retries: usize = 3;
const maxRetries: usize = 3;
```

### 3.4 变量名称

```rust
// 好: 描述性名称
let user_id = "user_123";
let connection_string = "...";
let retry_count = 0;

// 不好: 单字母或含混
let uid = "user_123";
let cs = "...";
let i = 0;
```

---

## 四、代码风格

### 4.1 缩进和空格

```rust
// 使用 4 个空格缩进（rustfmt 默认）
pub fn example() {
    if condition {
        do_something();
    }
}

// 使用 rustfmt 自动格式化
// cargo fmt

// 检查格式
// cargo fmt -- --check
```

### 4.2 行长度

```rust
// 目标: 100 字符以内，最多 120 字符
// 好: 清晰的行长度
pub fn create_novel(
    title: &str,
    description: &str,
    author_id: &str,
) -> Result<Novel> {
    // implementation
}

// 不好: 超长行
pub fn create_novel(title: &str, description: &str, author_id: &str) -> Result<Novel> { /* ... */ }
```

### 4.3 空行使用

```rust
// 模块级别的概念之间使用空行
pub struct Novel {
    id: String,
    title: String,
}

impl Novel {
    pub fn new(id: String, title: String) -> Self {
        Self { id, title }
    }

    pub fn title(&self) -> &str {
        &self.title
    }
}

// 函数之间使用空行
pub fn function_one() {
    // implementation
}

pub fn function_two() {
    // implementation
}
```

---

## 五、注释和文档

### 5.1 文档注释

```rust
/// Creates a new novel with the given title and description.
///
/// # Arguments
///
/// * `title` - The title of the novel (non-empty string)
/// * `description` - Optional description of the novel
///
/// # Returns
///
/// Returns a `Result` containing the created `Novel` or an error.
///
/// # Errors
///
/// Returns `Error::InvalidTitle` if the title is empty.
///
/// # Examples
///
/// ```
/// let novel = create_novel("My Novel", Some("A great story"))?;
/// assert_eq!(novel.title(), "My Novel");
/// ```
pub fn create_novel(title: &str, description: Option<&str>) -> Result<Novel> {
    // implementation
}
```

### 5.2 注释

```rust
// 好: 解释为什么，而不是做什么
let retry_count = 0;
// Retry up to 3 times with exponential backoff for transient failures
while retry_count < 3 {
    // implementation
}

// 不好: 重复代码
let retry_count = 0;
// Increment retry count
retry_count += 1;
```

### 5.3 TODO 和 FIXME

```rust
// TODO: Implement caching for performance
pub fn get_novel(id: &str) -> Result<Novel> {
    // implementation
}

// FIXME: This error handling is incomplete
pub fn risky_operation() -> Result<()> {
    // implementation
}

// 使用结构化格式
// TODO(username): Description
// FIXME(username, ticket-123): Description
```

---

## 六、错误处理

### 6.1 错误类型定义

```rust
/// Custom error type for the application
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid title: {0}")]
    InvalidTitle(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Internal server error")]
    InternalServerError,
}

pub type Result<T> = std::result::Result<T, Error>;
```

### 6.2 错误处理模式

```rust
// 好: 显式错误处理
pub fn create_novel(title: &str) -> Result<Novel> {
    if title.is_empty() {
        return Err(Error::InvalidTitle("Title cannot be empty".to_string()));
    }

    let novel = Novel::new(title);
    Ok(novel)
}

// 使用 ? 操作符链接结果
pub fn fetch_and_process(id: &str) -> Result<ProcessedData> {
    let data = fetch_from_db(id)?;
    let processed = process_data(data)?;
    Ok(processed)
}

// 不好: 忽略错误
pub fn create_novel(title: &str) -> Novel {
    let novel = parse_title(title).unwrap_or_default(); // 危险！
    novel
}
```

### 6.3 日志和错误

```rust
// 好: 记录错误上下文
match operation() {
    Ok(result) => {
        log::info!("Operation succeeded with result: {:?}", result);
        Ok(result)
    }
    Err(e) => {
        log::error!("Operation failed: {:?}", e);
        Err(e)
    }
}

// 使用 tracing 进行结构化日志
use tracing::{error, info};

error!(error = ?e, "Failed to create novel");
info!(novel_id = %id, "Novel created successfully");
```

---

## 七、函数设计

### 7.1 函数大小

```rust
// 好: 小而专一的函数
pub fn validate_title(title: &str) -> Result<()> {
    if title.is_empty() {
        return Err(Error::InvalidTitle("empty".into()));
    }
    if title.len() > 256 {
        return Err(Error::InvalidTitle("too long".into()));
    }
    Ok(())
}

// 不好: 函数过大，职责混杂
pub fn create_novel_and_save_and_notify(
    title: &str,
    db: &Database,
    email: &str,
) -> Result<Novel> {
    // 验证
    // 创建
    // 保存
    // 发送通知
    // 太多职责！
}
```

### 7.2 参数设计

```rust
// 好: 明确的参数类型
pub fn create_novel(
    title: &str,
    description: &str,
    author_id: &str,
) -> Result<Novel> {}

// 使用 builder 模式处理多个参数
pub struct NovelBuilder {
    title: String,
    description: Option<String>,
    author_id: String,
}

impl NovelBuilder {
    pub fn new(title: String, author_id: String) -> Self {
        Self {
            title,
            description: None,
            author_id,
        }
    }

    pub fn with_description(mut self, desc: String) -> Self {
        self.description = Some(desc);
        self
    }

    pub fn build(self) -> Result<Novel> {
        // implementation
    }
}

// 不好: 布尔参数（隐晦）
pub fn create_novel(title: &str, is_draft: bool, is_public: bool) {}
// 应该使用枚举
pub enum PublishStatus {
    Draft,
    Published,
}
```

### 7.3 返回类型

```rust
// 好: 返回 Result 处理错误
pub fn get_novel(id: &str) -> Result<Novel> {}

// 好: 返回 Option 表示可能不存在
pub fn find_novel(id: &str) -> Option<Novel> {}

// 不好: 返回 unwrap 的结果
pub fn get_novel(id: &str) -> Novel {
    find_in_db(id).unwrap() // 会 panic！
}
```

---

## 八、类型和泛型

### 8.1 结构体设计

```rust
// 好: 清晰的字段和职责
pub struct Novel {
    id: String,
    title: String,
    description: Option<String>,
    author_id: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

// 好: 使用 newtype 模式提高类型安全
pub struct NovelId(String);
pub struct AuthorId(String);

pub struct Novel {
    id: NovelId,
    author_id: AuthorId,
    // ...
}

// 不好: 所有字段都是 String
pub struct Novel {
    field1: String,
    field2: String,
    field3: String,
}
```

### 8.2 泛型使用

```rust
// 好: 使用泛型提高代码复用
pub trait Repository<T> {
    async fn create(&self, item: T) -> Result<T>;
    async fn get(&self, id: &str) -> Result<T>;
    async fn list(&self) -> Result<Vec<T>>;
}

// 特定实现
pub struct NovelRepository {
    pool: SqlitePool,
}

impl Repository<Novel> for NovelRepository {
    // implementation
}

// 不好: 重复代码
pub struct NovelRepository {
    // implementation for Novel
}
pub struct ChapterRepository {
    // implementation for Chapter (重复！)
}
```

### 8.3 特征绑定

```rust
// 好: 清晰的特征绑定
pub fn serialize<T: Serialize>(item: &T) -> Result<String> {
    Ok(serde_json::to_string(item)?)
}

// 好: 使用 where 子句提高可读性
pub fn process<T, U>(input: T) -> Result<U>
where
    T: Into<String>,
    U: From<String>,
{
    // implementation
}
```

---

## 九、测试

### 9.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_title_with_empty_string() {
        let result = validate_title("");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_title_with_valid_string() {
        let result = validate_title("Valid Title");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_title_too_long() {
        let long_title = "a".repeat(300);
        let result = validate_title(&long_title);
        assert!(result.is_err());
    }
}
```

### 9.2 集成测试

```rust
// tests/integration_tests.rs
use my_app::*;

#[tokio::test]
async fn test_create_novel_integration() {
    // 设置
    let db = setup_test_db().await;
    let repository = NovelRepository::new(db);

    // 执行
    let novel = repository
        .create(&Novel::new("Test Novel", "Test Author"))
        .await;

    // 验证
    assert!(novel.is_ok());
    let novel = novel.unwrap();
    assert_eq!(novel.title(), "Test Novel");
}
```

### 9.3 测试最佳实践

```rust
#[test]
fn test_something() {
    // AAA 模式: Arrange, Act, Assert

    // Arrange: 设置测试数据
    let input = "test input";
    let expected = "test output";

    // Act: 执行被测试的代码
    let result = function_under_test(input);

    // Assert: 验证结果
    assert_eq!(result, expected);
}

// 使用 parameterized 测试
#[test]
fn test_validation_with_various_inputs() {
    let test_cases = vec![
        ("", false),
        ("valid", true),
        ("a".repeat(256), false),
    ];

    for (input, expected) in test_cases {
        let result = validate(input).is_ok();
        assert_eq!(result, expected, "Failed for input: {}", input);
    }
}
```

---

## 十、异步编程

### 10.1 async/await 使用

```rust
// 好: 清晰的异步函数
pub async fn fetch_novel(id: &str) -> Result<Novel> {
    let novel = sqlx::query_as::<_, Novel>(
        "SELECT * FROM novels WHERE id = ?"
    )
    .bind(id)
    .fetch_one(&self.pool)
    .await?;

    Ok(novel)
}

// 好: 并发操作
pub async fn fetch_multiple(ids: &[&str]) -> Result<Vec<Novel>> {
    let futures = ids.iter().map(|id| fetch_novel(id));
    let novels = futures::future::join_all(futures).await;
    // 过滤错误...
    Ok(novels)
}

// 不好: 阻塞异步上下文
pub async fn bad_async() {
    let result = std::thread::sleep(Duration::from_secs(1)); // 会阻塞！
}

// 应该使用
pub async fn good_async() {
    tokio::time::sleep(Duration::from_secs(1)).await; // 正确
}
```

### 10.2 错误处理

```rust
// 好: 在异步上下文中处理错误
pub async fn operation() -> Result<()> {
    let result = risky_operation().await?;
    process(result).await?;
    Ok(())
}

// 处理多个异步操作
pub async fn multiple_operations() -> Result<(Data1, Data2)> {
    let future1 = fetch_data1();
    let future2 = fetch_data2();

    let (result1, result2) = tokio::join!(future1, future2);

    Ok((result1?, result2?))
}
```

---

## 十一、依赖和导入

### 11.1 导入顺序

```rust
// 标准库导入
use std::collections::HashMap;
use std::fs;

// 外部 crate 导入
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

// 本地模块导入
use crate::models::Novel;
use crate::services::NovelService;

// 当前模块导入
use super::helper;
```

### 11.2 导入最佳实践

```rust
// 好: 具体导入
use crate::models::{Novel, Chapter};
use crate::errors::Error;

// 不好: 通配符导入（隐晦）
use crate::models::*;

// 异常：当导入多个相关项时可以使用
pub use self::{
    models::*,
    services::*,
    dto::*,
};
```

---

## 十二、性能考虑

### 12.1 所有权和借用

```rust
// 好: 使用引用避免复制
pub fn process(data: &[u8]) -> Result<()> {
    // 无需复制 data
}

// 好: 使用 into() 进行零成本抽象
pub fn create(input: impl Into<String>) -> Result<Self> {
    let name = input.into();
    Ok(Self { name })
}

// 不好: 不必要的复制
pub fn process(data: Vec<u8>) -> Result<()> {
    // data 被复制
}
```

### 12.2 迭代器

```rust
// 好: 使用迭代器
pub fn filter_novels(novels: &[Novel]) -> Vec<&Novel> {
    novels
        .iter()
        .filter(|n| n.is_public())
        .collect()
}

// 好: 链式操作
let result: Vec<String> = novels
    .iter()
    .map(|n| n.title().to_string())
    .filter(|t| !t.is_empty())
    .collect();

// 避免: 多次循环
let mut result = Vec::new();
for novel in novels {
    if novel.is_public() {
        result.push(novel.title());
    }
}
```

### 12.3 内存管理

```rust
// 好: 使用 Arena 分配器处理大量小对象
// 好: 使用对象池回收对象
// 好: 使用 String 而不是 Vec<u8>

// 不好: 不必要的堆分配
let vec = vec![0; 1000]; // 如果可以用栈的话
```

---

## 十三、代码审查检查清单

审查别人的代码时检查：

### 13.1 正确性
- [ ] 逻辑是否正确？
- [ ] 错误处理是否完整？
- [ ] 是否有边界条件问题？
- [ ] 是否有竞态条件？

### 13.2 风格一致性
- [ ] 命名是否遵循规范？
- [ ] 代码格式是否一致？
- [ ] 是否通过了 clippy 检查？

### 13.3 可维护性
- [ ] 代码是否清晰易读？
- [ ] 注释是否充分？
- [ ] 是否有测试？
- [ ] 是否有文档？

### 13.4 性能
- [ ] 是否有明显的性能问题？
- [ ] 是否有不必要的复制？
- [ ] 是否有合理的缓存策略？

---

## 十四、自动化工具

### 14.1 格式化

```bash
# 自动格式化代码
cargo fmt

# 检查格式是否符合
cargo fmt -- --check
```

### 14.2 Linting

```bash
# 运行 clippy
cargo clippy

# 修复问题
cargo clippy --fix

# 强制所有警告都是错误
cargo clippy -- -D warnings
```

### 14.3 文档

```bash
# 检查文档完整性
cargo doc --no-deps

# 验证文档示例
cargo test --doc
```

### 14.4 测试

```bash
# 运行所有测试
cargo test

# 生成覆盖报告
cargo tarpaulin --out Html
```

---

## 十五、常见错误和解决方案

### 15.1 常见编译错误

```rust
// 错误: 生命周期问题
pub struct Ref<'a> {
    data: &'a str,
}

// 错误: 移动后使用
let s = String::from("hello");
println!("{}", s);
println!("{}", s); // 错误！已移动

// 解决: 使用引用
let s = String::from("hello");
println!("{}", &s);
println!("{}", &s); // 正确
```

### 15.2 常见逻辑错误

```rust
// 错误: 忘记 await
pub async fn fetch() -> Result<Data> {
    let future = async_operation();
    // 忘记了 .await!
    Ok(future)
}

// 解决
pub async fn fetch() -> Result<Data> {
    let data = async_operation().await?;
    Ok(data)
}
```

---

## 总结

遵循这些规范将确保：

✅ **代码一致性** - 整个项目看起来和感觉一致  
✅ **可维护性** - 其他开发者易于理解和修改代码  
✅ **性能** - 避免常见的性能陷阱  
✅ **安全性** - 充分利用 Rust 的安全特性  
✅ **质量** - 高质量的生产代码  

---

**规范版本**: 1.0  
**最后更新**: 2026-10-20  
**执行**: 强制执行（使用 CI/CD）

