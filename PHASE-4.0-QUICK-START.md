# Phase 4.0 快速开始指南

**版本**: 1.0  
**发布日期**: 2026-10-20  
**适用**: 所有开发人员  
**阅读时间**: 10-15 分钟

---

## 一、30 秒快速了解

**这是什么**?  
从 2026-10-22 开始，我们进入 Phase 4.0，用 6 周时间构建企业级 Web 服务。

**我需要做什么**?  
按照每日计划实现 5 个 Task，每个 Task 包括代码、测试和文档。

**成功标志**?  
2026-12-10 发布 v1.0.0，包含 13 个端点、3 个 Repository、完整的认证、监控和部署系统。

---

## 二、项目快照

### 2.1 项目规模

```
代码行数:        ~2,440 行 (5 个 Task)
单元测试:        112+ 个
测试覆盖率:      > 85%
开发周期:        6 周 (Oct 22 - Dec 10)
团队规模:        1-2 人
```

### 2.2 5 个 Task 总览

| Task | 内容 | 时间 | 测试 |
|------|------|------|------|
| 4.1 | Web 框架 (13 个端点) | W1 (5-6 天) | 35+ |
| 4.2 | 数据库 (3 个 Repository) | W2 (4-5 天) | 28+ |
| 4.3 | 认证 (JWT + Bcrypt) | W3 (3-4 天) | 23+ |
| 4.4 | 监控 (Prometheus) | W4 (2-3 天) | 18+ |
| 4.5 | 部署 (Docker) | W5 (1-2 天) | 8+ |

---

## 三、如何开始

### 3.1 环境检查清单 (5 分钟)

```bash
# 检查 Rust 版本
rustc --version          # 应该 1.70+
cargo --version          # 应该 1.70+

# 检查 Git
git --version            # 应该 2.0+
git config user.name     # 应该有值

# 检查数据库
sqlite3 --version        # 应该有版本

# 检查 IDE
code --version          # VS Code
# 或打开 Cursor IDE
```

### 3.2 项目设置 (10 分钟)

```bash
# 1. 创建特性分支
git checkout -b phase-4.0-implementation

# 2. 构建项目
cargo build --release

# 3. 运行测试
cargo test --all

# 4. 检查代码
cargo fmt
cargo clippy -- -D warnings

# 5. 查看文档
cargo doc --open
```

### 3.3 阅读关键文档 (15 分钟)

按优先级阅读：

1. **必读** (5 分钟)
   - 📘 `PHASE-4.0-STARTUP-CHECKLIST.md` - 启动计划
   - 📙 `PHASE-4.0-FULL-IMPLEMENTATION-GUIDE.md` - 全局指南

2. **参考** (5 分钟)
   - 📕 `RUST-CODE-STYLE-GUIDE.md` - 编码规范
   - 📗 `TESTING-PLAN.md` - 测试框架

3. **按需** (5 分钟)
   - 📓 `TASK-4.X-IMPLEMENTATION-ROADMAP.md` - 具体 Task 规划

---

## 四、每日工作流程

### 4.1 工作日时间表

```
09:00-09:15   晨会
09:15-12:30   编码 (2.5h)
12:30-14:00   午休
14:00-16:00   编码/测试 (2h)
16:00-17:00   审查/提交 (1h)
```

### 4.2 每日检查清单

**编码检查**:
```bash
cargo fmt                              # 格式化
cargo clippy -- -D warnings            # 无警告
cargo test --lib                       # 单元测试
cargo tarpaulin --out Html             # 覆盖率
```

**提交检查**:
```bash
git status                             # 清空工作树
git add .                              # 添加文件
git commit -m "描述"                   # 提交
git push origin phase-4.0-implementation # 推送
```

### 4.3 编码模板

**新函数的标准流程**:

```rust
// 1. 编写函数（带文档注释）
/// 创建故事
/// 
/// # 参数
/// - `req`: 创建请求
/// 
/// # 返回
/// 创建成功时返回故事对象
pub async fn create_novel(req: CreateNovelRequest) -> Result<Novel> {
    // 实现逻辑
}

// 2. 编写单元测试
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_novel_valid() {
        // 正常场景测试
    }

    #[tokio::test]
    async fn test_create_novel_invalid() {
        // 异常场景测试
    }
}

// 3. 运行测试确保通过
// cargo test create_novel

// 4. 检查文档和覆盖率
// cargo doc
// cargo tarpaulin
```

---

## 五、常见任务速查

### 5.1 运行单个测试

```bash
# 运行单个测试
cargo test test_create_novel -- --nocapture

# 运行特定模块的所有测试
cargo test routes::novels

# 运行并显示详细输出
cargo test -- --nocapture --test-threads=1
```

### 5.2 性能分析

```bash
# 生成覆盖率报告
cargo tarpaulin --out Html > coverage.html

# 运行基准测试
cargo criterion

# 记录性能指标
cargo build --release
time ./target/release/app
```

### 5.3 代码审查

```bash
# 查看待审查的提交
git log origin/main..HEAD --oneline

# 查看本次修改
git diff HEAD~1

# 对比两个分支
git diff main..HEAD
```

---

## 六、质量标准速查

### 6.1 代码质量门槛

| 指标 | 要求 | 检查命令 |
|-----|------|--------|
| 格式化 | 通过 | `cargo fmt -- --check` |
| Clippy | 0 警告 | `cargo clippy -- -D warnings` |
| 单元测试 | 100% 通过 | `cargo test --lib` |
| 覆盖率 | > 85% | `cargo tarpaulin` |
| 文档 | 100% 完整 | `cargo doc --document-private-items` |

### 6.2 提交质量门槛

```
前置检查:
  ✓ git status 工作树干净
  ✓ 所有新文件已添加
  ✓ cargo test 全部通过
  ✓ cargo fmt 格式化完成

提交信息:
  ✓ 主语动词 + 宾语 (英文或中文)
  ✓ 第一行 < 50 字
  ✓ 包含 What/Why/How
  ✓ 关联 Issue (如果有)
```

---

## 七、问题排查指南

### 7.1 编译错误

**错误**: `error[E0425]: cannot find value`

```bash
# 原因: 变量或函数未定义
# 解决:
1. 检查拼写
2. 检查导入 (use statement)
3. 检查作用域
```

**错误**: `error[E0308]: mismatched types`

```bash
# 原因: 类型不匹配
# 解决:
1. 检查函数返回类型
2. 检查参数类型
3. 使用 .into() 进行类型转换
```

### 7.2 测试失败

**断言失败**:

```bash
# 在测试中添加详细的断言信息
assert_eq!(actual, expected, "详细的失败说明");

# 运行时查看详细输出
cargo test test_name -- --nocapture
```

**异步测试超时**:

```bash
# 在 Cargo.toml 中增加超时时间
[profile.test]
timeout = 300

# 或使用 tokio::test 的 timeout
#[tokio::test]
#async fn test_something() { ... }
```

### 7.3 性能问题

```bash
# 1. 生成覆盖率报告，找出关键路径
cargo tarpaulin --out Html

# 2. 使用基准测试
cargo criterion

# 3. 分析性能热点
cargo flamegraph  # 需要安装 flamegraph
```

---

## 八、提交到 Git

### 8.1 标准提交流程

```bash
# 1. 确保工作树干净
cargo test --all
cargo fmt
cargo clippy

# 2. 检查修改
git status
git diff

# 3. 暂存修改
git add .

# 4. 提交
git commit -m "feat: 功能描述"

# 5. 推送
git push origin phase-4.0-implementation

# 6. 创建 PR (如果需要)
gh pr create --title "功能标题"
```

### 8.2 提交信息模板

```
[类型]: [简短描述]

[详细说明]

[相关链接]

---
# 类型: feat (新功能), fix (修复), refactor (重构), docs (文档), test (测试)
# 简短描述: < 50 字
# 详细说明: 说明做了什么和为什么
# 相关链接: 关闭的 Issue, PR 等
```

---

## 九、获得帮助

### 9.1 文档导航

| 需求 | 文档 | 位置 |
|-----|------|------|
| 全局计划 | PHASE-4.0-FULL-IMPLEMENTATION-GUIDE.md | docs/plans |
| 启动清单 | PHASE-4.0-STARTUP-CHECKLIST.md | docs/plans |
| 编码规范 | RUST-CODE-STYLE-GUIDE.md | docs/plans |
| 测试框架 | TESTING-PLAN.md | docs/plans |
| Task 详情 | TASK-4.X-IMPLEMENTATION-ROADMAP.md | docs/plans |
| 架构决策 | PHASE-4.0-ARCHITECTURE-DECISIONS.md | docs/wiki |

### 9.2 命令速查

```bash
# 快速构建和测试
cargo build && cargo test

# 完整的质量检查
cargo fmt && cargo clippy && cargo test && cargo tarpaulin

# 查看文档
cargo doc --open

# 查看项目依赖
cargo tree

# 检查安全漏洞
cargo audit
```

### 9.3 常见命令

| 任务 | 命令 |
|-----|------|
| 新建项目 | `cargo new --lib project_name` |
| 添加依赖 | `cargo add dependency_name` |
| 更新依赖 | `cargo update` |
| 审查依赖 | `cargo outdated` |
| 清理构建 | `cargo clean` |
| 查看汇编 | `cargo asm` |

---

## 十、下一步

### 立即行动 (2026-10-22)

- [ ] 阅读完整的启动检查清单
- [ ] 验证开发环境
- [ ] 参加启动会议
- [ ] 开始 Task 4.1 Day 1

### 关键日期

```
2026-10-21 (明天)
  └─ 启动前最后检查

2026-10-22 (后天) 09:00 AM 🚀
  └─ Phase 4.0 正式启动!

2026-10-26 (下周六)
  └─ Task 4.1 完成

2026-12-10
  └─ v1.0.0 发布 🎉
```

---

## 总结

**这份指南告诉你**:
1. ✅ Phase 4.0 是什么
2. ✅ 如何快速上手
3. ✅ 每天应该做什么
4. ✅ 如何保证代码质量
5. ✅ 遇到问题怎么办

**记住这个流程**:
```
修改代码 → 写测试 → 验证通过 → 提交 → 继续下一个
```

**如有疑问**:
- 查看相应的规划文档
- 参考架构决策文档
- 检查快速命令参考

---

**准备好开始了吗？** 🚀

见你在 2026-10-22 09:00 AM！

