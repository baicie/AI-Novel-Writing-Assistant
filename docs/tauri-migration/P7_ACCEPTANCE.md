# P7 验收结论

## Gate 判定

**P7 Gate：PASS（自动化与核心 IPC）；GUI/真实 Provider 验收 BLOCKED。最终终态订阅修复提交：`2ac0fbce`。**

通过条件：

- 已确认的 P7 代码缺陷（SSE 截断成功、取消不能中断读取、无界 Token 队列、Lag 事件丢失、分页 total 过滤、World/Character unsupported 误报、系统凭据错误吞没、任务类型空成功）已修复并有回归测试。
- 数据持久化、正文保护、导入回滚/只读/备份/关系/hash/旧 Run 不重放测试通过。
- Workflow Lease/CAS/Approval/恢复测试通过。
- 真实 Tauri WebView Channel 基础消息通过。
- Tauri Release build、bundle audit、route/save/adapter smoke 通过。
- 未把 Web smoke 冒充 GUI 输入法/自动保存/撤销/粘贴验收。

未通过/阻塞但已显式列出：

- `cargo test --workspace --all-targets` 被用户未跟踪旧版 `integration_runs.rs` 阻塞；不修改该文件。
- 真实 GUI 输入法、Plate 自动保存、切换章节、撤销/重做、粘贴、关闭重开持久化未执行真实人工操作，状态 BLOCKED。
- 真实付费模型、真实 429/usage/Provider 差异未执行，状态 BLOCKED。
- 强制 broadcast Lag 故障注入、页面重载/Channel 释放未自动化，状态 NEEDS_MANUAL_VERIFICATION。

## 复现命令

```bash
cargo test --workspace --exclude novel-http
# 212 passed

cargo test -p novel-http \
  --test common_setup --test endpoint_tests --test integration \
  --test integration_chapters --test integration_novels
# 57 passed

pnpm typecheck
pnpm build
pnpm --filter @ai-novel/client test
cargo fmt --check -p novel-domain -p novel-workflow -p novel-llm \
  -p novel-storage-sqlite -p novel-application -p novel-desktop

scripts/smoke-tauri-route.sh
scripts/smoke-tauri-save.sh
scripts/smoke-tauri-adapter.sh
scripts/smoke-tauri-channel.sh
scripts/audit-tauri-release.sh
```

用户未跟踪文件保持原样：

```bash
git status --short
# ?? crates/novel-http/tests/integration_runs.rs
```

## 结论边界

P7 的自动化代码 Gate 通过，但不是完整产品验收。真实 macOS GUI 与真实模型环境仍是发布前阻塞。
