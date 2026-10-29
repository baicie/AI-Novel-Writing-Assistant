# 服务端 tsc 堆内存溢出与测试数据库前置条件

## 背景

`server` 目前有 1000 多个 TypeScript 文件。`tsc` 在 Node 默认堆上限（约 2GB）下编译时，会以 `JavaScript heap out of memory` 终止，进程退出码为 134（macOS 上表现为 `Abort trap: 6`）。这个现象容易被误判为 Node 版本、Prisma 生成物或 better-sqlite3 原生模块的问题，但这些都不是根因。

另一个常见误判发生在测试阶段：仓库里的默认开发库可能是没有执行过迁移的空库，直接运行服务端测试会出现几百个失败，原因是缺表，不是业务回归。

## 当前规则

- `server/package.json` 中的 `build` 和 `typecheck` 通过 `node --max-old-space-size=6144 node_modules/typescript/lib/tsc.js ...` 调用 TypeScript，不直接调用 `tsc` 可执行文件。文件数继续增长时先调整这个上限，同时按架构规则拆分大模块。
- 不用 esbuild 替代 `tsc` 作为构建步骤。esbuild 不做类型检查，会让构建通过但类型错误进入产物。
- 运行服务端测试前，使用临时库并先执行迁移：设置 `DATABASE_URL=file:/tmp/<name>.db`，再执行 `prisma migrate deploy`。禁止为了让测试通过去迁移、重置或清空开发库。
- 构建通过只说明编译产物可生成，不能当作测试通过。报告验证结果时需要写明实际运行的命令、通过数、失败数和跳过数。

## 诊断路径

1. 构建以 134 退出且输出包含 `heap out of memory` 时，先确认堆上限，再考虑其他原因。
2. 测试大面积报缺表或缺列时，先确认 `DATABASE_URL` 指向的库已执行迁移。
3. 个别测试依赖 `node:sqlite`：Node 20 没有该模块，需要 Node 22 并带 `--experimental-sqlite`。在 Node 22 下，原生模块 better-sqlite3 需要针对该 Node 版本重新编译，否则会因 `NODE_MODULE_VERSION` 不匹配而加载失败。重新编译会替换仓库共用的原生二进制文件，切换前先确认不会影响其他 Node 版本下的运行。
4. 前端状态推断数量守卫（`directorSimplificationGuard`）失败时，先用 `git log -S` 与按提交统计定位新增匹配的来源，不要通过上调基线来让测试通过。基线上调需要人工明确批准。

## 相关模块

- `server/package.json` 构建与类型检查脚本
- `server/scripts/run-tests.cjs` 测试分组与进程隔离，参见 `server-test-isolation.md`
- `server/tests/directorSimplificationGuard.test.js` 与 `server/tests/fixtures/directorSimplification.baseline.json`
