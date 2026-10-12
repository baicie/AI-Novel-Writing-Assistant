# macOS 桌面打包与分芯片产物规则

## Background

桌面端最初只产出 Windows x64 安装包。macOS 版需要同时覆盖 Apple 芯片（arm64）和 Intel 芯片（x64），并且要和 Windows 共用同一个 `vX.Y.Z` GitHub Release 与 electron-updater 更新通道。macOS 在签名、自动更新和窗口生命周期上与 Windows 差异很大，照搬 Windows 流程会产出"能打包但用户打不开、更新装不上、关窗后再也打不开"的安装包。

## Current Rule

### 产物与命名

- 每个芯片单独产出 `dmg`（给用户安装）和 `zip`（给 electron-updater 增量更新）：`Biz Novel Studio-X.Y.Z-mac-arm64.dmg/zip`、`Biz Novel Studio-X.Y.Z-mac-x64.dmg/zip`。
- 文件名中的 `arm64` 标记是 electron-updater 在 Apple 芯片机器上挑选安装包的依据，Intel 包的文件名里不能出现 `arm64`。不要改成 `universal` 或去掉 `${arch}`。
- 不产出 universal 包：两份单芯片包体积更小，原生模块也更容易逐个校验。

### 同一个 job 打两种芯片

- `desktop-release.yml` / `desktop-beta-release.yml` 中的 macOS job 在一台 Apple 芯片 runner 上用 `--mac --arm64 --x64` 一次打出两种芯片。
- 原因：electron-builder 每次发布都会上传一份 `latest-mac.yml`。如果拆成 arm64、x64 两个并行 job，后上传的那份会覆盖前一份，导致某一种芯片的用户永远检查不到更新，或者 Apple 芯片用户被更新成 Intel 包。
- macOS job `needs` Windows job，避免两个 job 同时创建同一个 GitHub Release。代价是 Windows 打包失败时 macOS 也不会发布，这是有意保持两个平台版本一致。
- 跨芯片构建依赖 `npmRebuild` 按架构重建 `better-sqlite3`。`verify-desktop-package.cjs --platform mac` 会用 `lipo -archs` 检查每个 `.app` 的主程序和 `better_sqlite3.node` 是否与目标芯片一致，这是防止"Intel 包里塞了 arm64 原生模块"的唯一门禁，不能删除。

### 签名、公证与更新

- 有 `CSC_LINK`（Developer ID 证书，CI 中来自 `MAC_CSC_LINK`）或 `CSC_NAME` 时：正式签名 + hardened runtime；若同时配置 `APPLE_ID` / `APPLE_APP_SPECIFIC_PASSWORD` / `APPLE_TEAM_ID`（或 API Key 三件套）则自动公证。
- 没有证书时：使用 ad-hoc 签名（`identity: "-"`），关闭 hardened runtime 和公证。Apple 芯片会拒绝运行完全未签名的程序，所以不能把 identity 设成 `null`。
- `run-electron-builder.cjs` 按目标平台读取签名变量：macOS 只认 `CSC_LINK` / `MAC_CSC_LINK` / `CSC_NAME`，不会把 Windows 证书（`WIN_CSC_LINK`）带进 macOS 签名。
- Squirrel.Mac 只接受与当前应用同一 Developer ID 签名的更新包。ad-hoc 包无法应用内安装更新，因此打包时把 `aiNovelDesktop.macAutoUpdate` 写进 app 的 `package.json`，运行时 `updater.ts` 读到 `false` 就把更新状态置为不可用，并提示用户到发布页下载对应芯片的新版替换。不要为 ad-hoc 包重新打开应用内下载安装，否则用户会在下载完成后遇到签名校验失败。
- 未签名的 macOS 包首次打开会被 Gatekeeper 拦截，需要用户在"系统设置 → 隐私与安全性"中允许打开。拿到 Developer ID 证书并配好 CI secrets 后，这一步和手动更新都会自动消失。

### 窗口生命周期

- macOS 上点击主窗口关闭按钮只隐藏窗口，本地服务继续运行；点击 Dock 图标（`activate`）重新显示窗口；`Cmd+Q` 走 `before-quit` 真正退出并停止服务。
- 应用内"重启安装更新"必须先解除隐藏保护再调用 `quitAndInstall`：Squirrel.Mac 会先关闭所有窗口再发出 `before-quit`，否则关窗被拦截，更新永远装不上。

## Failure Modes

- `ENOTEMPTY: rename .../Electron.app -> .../Biz Novel Studio.app`：上一轮打包失败时另一个架构仍在后台继续写入 `build/dist`。确认没有残留的 electron-builder 进程后删除 `desktop/build/dist` 再重跑 `pnpm verify:desktop-package:mac:reuse-stage`。
- 校验报 `better-sqlite3 native module is built for [arm64], expected x86_64`：跨芯片重建失败，优先检查 electron-builder 日志中 `@electron/rebuild arch=x64` 是否下载到了 prebuild，而不是去改校验脚本。
- 本地用 Node 26 构建时 `better-sqlite3` 编译失败、服务端 `tsc` 内存溢出：本地打包请使用与 CI 一致的 Node 24，并设置 `NODE_OPTIONS=--max-old-space-size=8192`。GitHub macOS runner 上的服务端构建同样会超出默认堆内存，macOS job 的构建步骤必须带上这个 `NODE_OPTIONS`。
- 开发包的更新清单必须是 `dev.yml` / `dev-mac.yml`。如果 Release 里出现 `latest.yml`，说明 publish channel 没有设成 `dev`，开发版客户端会找不到更新，也不能让这份清单冒充正式通道。
- 用户反馈"更新检查不可用"：如果是 ad-hoc 包，这是预期行为；如果已签名仍不可用，检查 app `package.json` 的 `aiNovelDesktop.macAutoUpdate` 是否为 `true`。

## Related Modules

- `desktop/electron-builder.config.cjs`：macOS 目标、产物命名、签名 / 公证开关与 `macAutoUpdate` 元数据。
- `desktop/builder/entitlements.mac.plist`：hardened runtime 下 Electron JIT 与加载原生模块所需权限。
- `desktop/scripts/run-electron-builder.cjs`：按平台归一化签名环境变量和发布门禁。
- `desktop/scripts/verify-desktop-package.cjs`：`--platform mac` 时逐芯片校验包结构与 Mach-O 架构。
- `desktop/src/runtime/updater.ts`、`desktop/src/main.ts`：macOS 手动更新判定与窗口生命周期。
- `.github/workflows/desktop-release.yml`、`.github/workflows/desktop-beta-release.yml`：macOS 发布 job。
- `.github/workflows/desktop-dev-release.yml`：`vX.Y.Z-dev.N` 开发包。macOS 仍在同一个 job 里同时打 arm64 和 x64，更新清单是 `dev-mac.yml`，不覆盖正式版的 `latest-mac.yml`。

## Source Documents

- [桌面版本号与发布标识规则](./desktop-release-versioning.md)
