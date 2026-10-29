# P8 E2E Security Audit

## 生产配置

- `src-tauri/tauri.conf.json`
  - `withGlobalTauri: false`
  - capabilities 仅 `default`
  - 无 `wdio` / `wdio-webdriver` 权限
- `src-tauri/Cargo.toml`
  - `tauri-plugin-wdio` / `tauri-plugin-wdio-webdriver` 为 optional
  - 只在 `--features e2e` 构建启用
- `src-tauri/src/lib.rs`
  - 插件注册包裹 `#[cfg(feature = "e2e")]`
- `client/src/main.tsx`
  - `@wdio/tauri-plugin` 仅在 `VITE_TAURI_E2E=1` 的 E2E 构建导入
- 生产前端 bundle 检查：没有 `wdio`/`tauri-plugin-wdio` 字符串

## E2E 配置隔离

- 独立 `tauri.e2e.conf.json`
- 独立 productName / identifier
- 独立 `dist-e2e`
- 独立 `capabilities/e2e.json`
- 独立临时应用数据目录
- E2E model secret 只在 `feature=e2e` 启用
- E2E endpoint 只从 `BIZ_NOVEL_E2E_ENDPOINT` 注入
- Mock Provider 只绑定 `127.0.0.1` 临时端口
- 测试不会读取真实用户小说数据库
- 不包含任意命令执行生产 IPC；WDIO execute 仅测试插件提供，生产未注册

## 生产构建审计命令

```bash
cargo tauri build --target aarch64-apple-darwin --bundles app
scripts/audit-tauri-release.sh
```

审计需要确认：

- `.app` 内无 Node/Electron/sidecar
- 二进制无 `tauri_plugin_wdio` / embedded server 字符串
- 生产前端无 `wdio` 依赖
- 生产 `withGlobalTauri: false`
- 生产能力不含 `wdio:default` / `wdio-webdriver:default`

## 风险

- E2E 使用假 Key；不是真实 Provider 验收。
- E2E 插件版本与 Tauri 版本需持续锁定；升级时重新跑 E2E。
- WDIO 日志可能包含测试正文；测试目录不得提交敏感内容。
- 生产构建必须在 E2E 构建之后重新执行，避免使用带测试特性的二进制。

## 实际审计结果

```text
production withGlobalTauri = false
production capabilities = ["default"]
production binary tauri_plugin_wdio = absent
production binary tauri-plugin-wdio = absent
production binary wdio-webdriver = absent
production binary TAURI_WEBDRIVER_PORT = absent
production bundle wdio files = none
production frontend wdio refs = none
scripts/audit-tauri-release.sh = PASS
```

E2E product 使用独立 `Biz Novel Studio E2E.app` / `com.ai-novel.studio.e2e`，不与生产 App 混用。
