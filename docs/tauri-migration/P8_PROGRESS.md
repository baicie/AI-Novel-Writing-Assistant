# P8 自动导演最小闭环进度

## 已完成

- 新增 `crates/novel-application/tests/p8_director_loop.rs`：
  - 创建小说和章节；
  - `open_run` 初始化导演 Run；
  - Worker 走完 `story_macro → character_cast → volume_strategy → chapter_list → context → draft → consistency_review → approval → commit` 的 Rust 计划；
  - book/chapter Gate 通过 `approve` 操作；
  - MockLlm 生成候选；
  - Candidate 经 `commit` 成为正式正文；
  - 重开 SQLite 后正文和 revision 保持一致；
  - 断言最终 Run 状态为 `succeeded`。

## 验证

```bash
cargo test -p novel-application --test p8_director_loop
# 1 passed
```

相关 P7 测试：

```bash
cargo test -p novel-application --test recovery
# 6 passed
```

## 明确边界

- P8 纵向闭环目前是 **Rust Application + SQLite + Mock Provider** 的可验证后端流程，不是真实 Provider 成功。
- `story_macro`、`character_cast`、`volume_strategy`、`chapter_list`、`context`、`consistency_review` 目前是占位 artifact/固定 payload；原版 Prompt 语义和真实业务生成仍为 PARTIAL。
- 人工 Gate 在 Rust application 测试中可操作；旧 AutoDirector UI 尚未迁移，不能宣称 UI 闭环完成。
- 未迁移图像、短剧、漫画、移动端、云端协作。
- 真实付费模型未调用。
