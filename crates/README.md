# 本地创作内核

这组 Rust crate 是桌面端的本地创作内核。它和现有 Electron 应用并排，不替换现有 Node 服务，也不改旧的 Prisma SQLite。

许可证继承仓库的 AGPL-3.0-only。用 Rust 重写业务规则不是更换许可证的理由。

## 依赖方向

```text
src-tauri / migration-tools
        │
        ▼
novel-application ── ports
        │
        ├── novel-domain
        ├── novel-workflow
        ├── novel-llm
        ├── novel-context
        └── novel-prompt

novel-storage-sqlite 实现 application 的仓储端口
novel-platform 提供密钥、日志、导出和系统时钟读取
```

- `novel-domain` 只放小说、章节保护和运行状态规则。它不能引用 Tauri、SQLx、HTTP 或系统时间。
- `novel-workflow` 只根据已经读出的产物和尝试记录决定下一步或恢复动作。它不领取任务，也不调用模型。
- `novel-application` 持有用例、事务边界和端口。后台推进循环也放在这里，避免工作流 crate 反过来依赖端口。
- `novel-storage-sqlite` 实现端口。迁移脚本在仓库根目录的 `migrations/`。
- 页面不直接调用 `invoke()`。桌面命令只从 `client/src/platform/desktopTransport.ts` 进入。

## 还没有度量的门槛

安装包体积、内存、冷启动、签名公证和召回基线都还没有实测。这些数字不能从当前代码推断。
