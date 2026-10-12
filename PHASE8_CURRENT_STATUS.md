# Phase 8 当前状态（2026-10-13）

## 构建问题的根因

`server` 的 `tsc` 在默认约 2GB 堆内存下处理 1087 个 TS 文件时内存溢出，
日志为 `FATAL ERROR: Ineffective mark-compacts near heap limit ... JavaScript heap out of memory`，
进程以 `Abort trap: 6`（退出码 134）结束。

Node 版本（20.19 / 22.12 / 26.9）、Prisma、better-sqlite3 都不是根因。

## 修复

`server/package.json` 的 `build` 与 `typecheck` 改为：

```
node --max-old-space-size=6144 node_modules/typescript/lib/tsc.js -p tsconfig.json [--noEmit]
```

已验证：`pnpm run typecheck` 退出码 0；`pnpm run build` 退出码 0。

## 测试现状（Node 20.19.0，快速测试集）

测试需要一个已应用迁移的 SQLite 库。默认的 `server/dev.db` 是空库，会导致
`The table main.DirectorRun does not exist`，一次运行出现 361 个失败，均与代码无关。

使用已执行 `prisma migrate deploy` 的临时库（`DATABASE_URL=file:/tmp/ai-novel-test.db`）后：
1927 项中 1911 通过、4 失败、12 跳过。4 个失败如下：

| 用例 | 现状 |
| --- | --- |
| `automaticAttempt.test.js` | 依赖 `node:sqlite`，Node 20 没有；Node 22 加 `--experimental-sqlite` 后 4/4 通过 |
| `pipelineExecutionLease.test.js` | 同样依赖 `node:sqlite`；在 Node 22 下被 better-sqlite3 的 `NODE_MODULE_VERSION` 不匹配阻断，需在 Node 22 下重新编译原生模块后再验证 |
| `promptCacheTemplateCompatibility.test.js` | `getRegisteredPromptAsset` 返回 null，尚未调查 |
| `directorSimplificationGuard.test.js` | 前端自行推断导演状态的出现次数 210，基线 205，尚未调查 |

## 待办

1. 在 Node 22 下重新编译 better-sqlite3，复验 `pipelineExecutionLease`。
2. 调查 `promptCacheTemplateCompatibility` 与 `directorSimplificationGuard` 的失败。
3. 完整集成测试（`test:integration`）尚未运行。
