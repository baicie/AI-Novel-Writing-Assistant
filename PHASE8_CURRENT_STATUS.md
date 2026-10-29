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
| `pipelineExecutionLease.test.js` | 同样依赖 `node:sqlite`；在 Node 22 下重新编译 better-sqlite3 后 3/3 通过。验证后已还原 Node 20 的原生二进制（校验值与备份一致，Node 20 加载正常） |
| `promptCacheTemplateCompatibility.test.js` | 测试期望的提示词版本已对齐，已修复 |
| `directorSimplificationGuard.test.js` | 前端自行推断导演状态的出现次数 210，基线 205。新增来自 `a20d3cb3`（+3）与 `59086d91`（+2），均在 `NovelEdit.tsx` 等文件中。基线上调需人工批准，根治需执行计划的阶段 6 |

## 集成测试

使用临时库运行 `server/scripts/run-tests.cjs integration`：238 项中 236 通过、0 失败、2 跳过。

## 第 1 天清单实测（Node 22.12.0，2026-10-13）

| 清单项 | 结果 | 说明 |
| --- | --- | --- |
| 任务 1 类型检查 `pnpm run typecheck` | 通过，退出码 0 | 覆盖 shared、server、client、desktop |
| 任务 2 `pnpm run lint` | 通过，退出码 0 | 三个包均为 `tsc --noEmit` |
| 任务 3 格式检查 | 无法执行 | 仓库没有 `format:check` 脚本，清单该项不适用 |
| 任务 4 文档审查 | 仅完成清单校验 | `pnpm run check:docs-manifest` 通过；逐篇人工审查未做 |
| 任务 6 `pnpm install --prod` | 有意跳过 | 该命令会移除开发依赖并破坏构建环境 |
| 任务 7 `pnpm run build` | 通过，退出码 0 | 约 40 秒；有 1 条大 chunk 体积告警，不影响构建 |
| 任务 8 备份 | 有意跳过 | 清单脚本复制 `src/`，本仓库没有该目录，且 `backups/` 已被误提交；备份应改用 `git` 提交或 `db:restore` 相关流程 |

## 待办

1. `directorSimplificationGuard`：阶段 6 依赖阶段 4。阶段 4 的动作表（计划文档 S4 决策点）尚未获得人工确认，`resolve_checkpoint` 也尚未实现，因此需先确认动作表。
2. 提交 `10cd38ce` 带入了 54 个 `backups/phase8_day1_*` 文件，推送前需决定是否取消跟踪。
