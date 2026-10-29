# Tauri 桌面壳与 Node 本地服务 sidecar

## 背景

产品的全部业务能力都在 `server/` 的 Node 服务（Express + Prisma）里，包括自动导演、章节生产、Prompt Registry、RAG 和任务中心。`crates/` 下的 Rust 本地内核只覆盖最小的单章生成闭环，与 Node 服务并排开发，并没有替代它。

只打包前端和 Rust 内核的 Tauri 安装包里没有任何 HTTP 服务。前端按桌面模式请求本机接口会全部失败；启动页又要等桌面桥接上报状态，所以安装后会一直卡在启动页。如果为了"不卡启动页"，在没有确认服务可用时直接放行，用户只是从卡在启动页变成进入工作台后满屏请求失败，问题并没有解决。

## 决策

过渡期内，Tauri 安装包内置 Node 运行时和编译后的 server，作为 sidecar 运行。Rust 壳层负责启动、健康检查、日志和退出清理；Rust 内核继续并排开发，迁移完成前不承担产品主链路。

原因：

- 桌面版与 Web 版、Electron 版保持完全相同的业务能力，不在桌面端再维护一套业务实现。
- Rust 迁移可以按模块逐个替换对应的 HTTP 接口，不需要一次性重写。
- 对新手用户来说，安装后能完整走完写书流程，比安装包体积更重要。

## 当前规则

### 启动模式与端口

- 打包版（release 构建）每次启动时在 `127.0.0.1` 上挑一个空闲端口，运行 `Contents/MacOS/node`，入口是 `Contents/Resources/server/dist/app/desktop/sidecarEntry.js`。
- 开发版（debug 构建）连接 `pnpm dev:server` 提供的 3000 端口，不自己拉起服务。
- 环境变量 `AI_NOVEL_DESKTOP_SERVER_MODE=external|bundled` 可以覆盖默认模式。
- 环境变量 `AI_NOVEL_SERVER_PORT` 用于固定端口，只在调试和验收时使用。
- 只有 `GET /api/health` 返回 200 才算服务可用。打包版等待 60 秒，开发版等待 120 秒。

### 进程生命周期

- sidecar 的 stdin 是控制管道：壳层一直持有写端，server 读到输入结束（EOF）就自行优雅退出，5 秒后强制退出。
- 正常退出、应用内重启、壳层崩溃都会关闭这个管道，所以 server 不会脱离桌面应用独自运行。不要改成依赖信号或端口探测来判断壳层是否还活着。
- 壳层退出时先关闭管道，等待 5 秒，仍未退出再强制结束进程。
- 环境变量与 Electron 版保持一致：
  - `NODE_ENV=production`、`AI_NOVEL_RUNTIME=desktop`、`AI_NOVEL_DESKTOP_PACKAGED=true`
  - `AI_NOVEL_DATABASE_MODE=sqlite`、`DATABASE_URL=file:./dev.db`
  - `HOST=127.0.0.1`、`ALLOW_LAN=false`
  - `DIRECTOR_NEXT_ENABLED` 未设置时默认为 `true`
- Tauri 版额外设置 `CORS_ORIGIN`，内容是 Tauri 的页面来源 `tauri://localhost`、`http://tauri.localhost` 和 `https://tauri.localhost`。同时移除 `NODE_OPTIONS` 和 `ACCESS_PASSWORD`，避免用户终端里的配置干扰打包版。

### 数据目录

- 数据根目录默认是系统应用数据目录下的 `Biz Novel Studio`（macOS 为 `~/Library/Application Support/Biz Novel Studio`），可以用 `BIZ_NOVEL_DATA_DIR` 覆盖。
- Node 服务使用其中的 `data/dev.db`、`storage/` 和 `logs/`；Rust 内核使用 `novel.sqlite`。
- Tauri 版与 Electron 版的数据目录（`AI-Novel-Writing-Assistant-v2`）分开，两个桌面壳不会同时打开同一个 SQLite 库。把 Electron 版数据导入 Tauri 版目前还没有入口。
- 壳层把 sidecar 输出写入 `logs/desktop-server-<UTC 时间>.log`，每次启动一个文件，按服务端的日志保留规则清理，见 [日志保留](../debugging/log-retention.md)。

### 前端运行时配置与启动页

- 主窗口在 `tauri.conf.json` 中设为 `create: false`，由 Rust 在 setup 阶段创建。创建时通过 initialization script，在页面脚本运行前写入 `window.__AI_NOVEL_RUNTIME__`，内容包括 `mode=desktop`、指向本次端口的 `apiBaseUrl`、`isPackaged` 和 `appVersion`。
- 端口每次启动都会变化，不能写死在前端构建产物或 `index.html` 里。
- 启动页状态来自 Rust 命令 `get_desktop_bootstrap_snapshot`，依次经过 `launching`、`starting-server`，最后到 `ready` 或 `error`。前端的 Tauri 桥接在启动阶段每 400ms 轮询一次，进入 `ready` 后改为每 3 秒一次。
- 本地服务意外退出时，快照会变成 `error`，启动页重新出现并提供"重新启动"。
- 启动页只能在健康检查通过后放行。禁止在没有确认服务可用时直接把状态设为 `ready`。
- Electron 版需要在界面挂载后发送 app-shell-ready，才会从 `loading-ui` 进入 `ready`。修改 `DesktopBootstrapBoundary` 时，必须保证 `loading-ui` 状态下仍会渲染界面，否则 Electron 版会永远停在启动页。

### CSP 与跨域

- CSP 的 `connect-src` 和 `img-src` 必须允许 `http://127.0.0.1:*` 和 `http://localhost:*`，以便访问接口、SSE 流和生成图片；`connect-src` 还要允许 Tauri IPC 用的 `ipc:` 和 `http://ipc.localhost`。
- Tauri 只会给 CSP 追加脚本和样式的 hash，不会自动放行这些来源。
- server 的 CORS 默认只放行 localhost 页面来源，Tauri 页面来源必须通过 `CORS_ORIGIN` 显式传入。

### 打包配置

- 基础配置 `src-tauri/tauri.conf.json` 不包含 sidecar，`cargo check` 和 `cargo tauri dev` 因此不依赖暂存产物。
- 安装包必须叠加 `src-tauri/tauri.bundle.conf.json`，`pnpm build:tauri:m1`、`build:tauri:x64` 和 `build:tauri:all` 都已带上。叠加配置负责构建 server、暂存 sidecar，并声明 `externalBin` 和 `resources`。

### 暂存脚本

`src-tauri/scripts/stage-node-sidecar.cjs`（即 `pnpm stage:tauri-sidecar`）做以下几件事：

- 用 `pnpm deploy --prod --config.node-linker=hoisted` 生成扁平的 `node_modules`。Tauri 复制资源时不跟随符号链接，pnpm 默认的符号链接布局会让依赖在安装包里悄无声息地消失。因此脚本会删除 `.bin` 目录，暂存目录里仍有符号链接时直接失败。
- 复制与 `@prisma/client` 版本一致的 `.prisma` 生成产物，并校验 provider 是 `sqlite`。
- 使用执行脚本的 Node 作为运行时，也可以用 `AI_NOVEL_TAURI_NODE_BINARY` 指定，并校验它的平台和架构与打包目标一致。
- 原生模块（better-sqlite3、sharp）来自本机安装，所以只能在与目标同平台、同架构的机器上打包。
- 暂存完成后，用打包进去的 Node 依次加载 better-sqlite3、sharp 和 `@prisma/client` 做冒烟测试。

## 示例

- 推荐：在 Apple 芯片机器上执行 `pnpm build:tauri:m1` 打 arm64 安装包。
- 推荐：验收打包版后端时，用临时目录和固定端口启动 `.app` 里的主程序，再请求健康检查，不碰用户的真实数据：

  ```bash
  BIZ_NOVEL_DATA_DIR=<临时目录> AI_NOVEL_SERVER_PORT=<端口> "<App>.app/Contents/MacOS/novel-desktop"
  curl http://127.0.0.1:<端口>/api/health
  ```

- 禁止：直接执行 `cargo tauri build`、不叠加 bundle 配置，然后把产物发给用户。这样的安装包里没有本地服务，启动页会显示"安装包缺少本地创作服务"。
- 禁止：在 Apple 芯片机器上用 `build:tauri:x64` 打 Intel 包，脚本会拒绝。x64 包需要在 Intel 机器或 x64 CI 上打。

## 失败模式

- 启动页显示"安装包缺少本地创作服务"：安装包里没有 `Contents/Resources/server/dist/app/desktop/sidecarEntry.js` 或 `Contents/MacOS/node`，说明打包时没有叠加 bundle 配置。
- 启动页显示"本地创作服务启动失败"或"启动超时"：打开日志目录查看 `desktop-server-*.log`。常见原因有三种：
  - 原生模块与打包的 Node ABI 不一致，日志里会出现 `NODE_MODULE_VERSION`；
  - 固定端口被占用；
  - 数据库迁移失败。
- 进入工作台后请求全部失败：先检查 CSP，再检查 CORS。WebView 控制台出现 `Refused to connect`，说明是 CSP 的问题；接口返回跨域错误，说明是 `CORS_ORIGIN` 的问题。
- 打包后出现 `Cannot find module`：检查暂存目录里是否还有符号链接，或者这个依赖是否只写在了 devDependencies 里。
- 正式分发前，`Contents/Resources/server` 里的 `.node` 和 `.dylib` 也需要随应用一起签名和公证。这一步目前还没有接入。

## 相关模块

- `src-tauri/src/node_backend/`：启动模式、端口、进程、健康检查、日志和启动快照。
- `src-tauri/src/lib.rs`：主窗口创建、壳层命令和退出清理。
- `src-tauri/tauri.conf.json`、`src-tauri/tauri.bundle.conf.json`、`src-tauri/capabilities/default.json`
- `src-tauri/scripts/stage-node-sidecar.cjs`
- `server/src/app/desktop/sidecarEntry.ts`
- `client/src/lib/desktop.ts`、`client/src/platform/desktopTransport.ts`、`client/src/components/layout/DesktopBootstrapBoundary.tsx`

## 来源文档

- [本地 Rust 创作内核](./rust-native-kernel.md)
- [桌面本地创作内核架构](./desktop-local-kernel.md)
- [macOS 桌面打包与分芯片产物规则](../workflows/desktop-macos-packaging.md)（Electron 版）
