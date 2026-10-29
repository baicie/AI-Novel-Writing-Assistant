# Tauri 2 Release CI —— P0 仓库与现有发布链路审计

审计对象：`migration/tauri-2-rust` @ `61670bc7`（工作树干净，只有未跟踪的受保护文件 `crates/novel-http/tests/integration_runs.rs`）。

## 一、现状事实

### 1. 分支与构建入口

| 项目 | 事实 |
| --- | --- |
| 当前分支 | `migration/tauri-2-rust`（Tauri 迁移分支，默认分支仍是 Electron 发布线） |
| Tauri 应用 | `src-tauri`（crate `novel-desktop`，lib `novel_desktop`） |
| Rust workspace | 11 个 crate + `migration-tools` + `src-tauri`，`version = "0.1.0"`（`version.workspace = true`） |
| 前端 | `client`（React 19 + Vite），构建产物 `client/dist` |
| 包管理 | pnpm 10.6.0（`packageManager` 已固定），`pnpm-lock.yaml` 与 `pnpm-workspace.yaml` 都在版本控制内 |
| Node 引擎 | `^20.19.0 || ^22.12.0 || >=24.0.0`；本机 v22.12.0，现有 CI 用 Node 24 |
| Rust | 本机 1.96.0，但仓库**没有** `rust-toolchain.toml`，现有 CI 用 `dtolnay/rust-toolchain@stable`（浮动） |
| Tauri CLI | 本机 `tauri-cli 2.12.2`；**仓库未把 `@tauri-apps/cli` 列为依赖** |

### 2. `src-tauri/tauri.conf.json`

- `version: "0.1.0"`（与 Cargo workspace、`client/package.json` 三处重复维护）
- `identifier: com.ai-novel.studio`
- `beforeBuildCommand`：`AI_NOVEL_CLIENT_BASE=relative pnpm --filter @ai-novel/client build`
- `bundle.targets: ["app"]` —— **只声明了 macOS `.app`**，没有 dmg / nsis / msi / deb / appimage
- `bundle.icon` 只有 png（32/64/128/256/512），**没有 `.ico`**（Windows bundle 必需）
- **没有 updater 配置**，`plugins` 段缺失

### 3. Updater 现状

- `src-tauri/Cargo.toml` 无 `tauri-plugin-updater`；`lib.rs` 无初始化；`plugins.updater` 无 endpoint/pubkey。
- 结论：当前产物**不具备自动更新能力**，Mode B（updater-signed）需要新增插件与配置，属于本阶段要落地的内容。

### 4. 现有 GitHub Actions（不删除，只新增）

| 文件 | 触发 | 作用 |
| --- | --- | --- |
| `desktop-release.yml` | `push: tags v*` / `desktop-v*` / dispatch | **Electron**（electron-builder + CSC + latest.yml），Windows only |
| `desktop-beta-release.yml` | `tags desktop-beta-v*` | Electron beta 通道 |
| `desktop-dev-release.yml` | `tags v*-dev.*` / dispatch | Electron dev 通道 |
| `site-pages.yml` | `push: main`（site/docs 路径） | 文档站 |
| `tauri-kernel-check.yml` | `pull_request`（crates/src-tauri/client 路径） / dispatch | Tauri 内核检查：fmt（**只查部分 crate**）、`cargo check`、`cargo test --workspace --exclude novel-http`、client typecheck、client 单测 |

## 二、风险清单（先列风险，再实施）

| # | 风险 | 证据 | 处置 |
| --- | --- | --- | --- |
| R1 | **Tag 命名空间冲突**：Electron 的 `desktop-release.yml` 已占用 `v*`，若 Tauri 也用 `v*` 触发，同一个 Tag 会同时跑 Electron 与 Tauri 两条发布流水线，产物、latest.yml、Release 正文互相覆盖 | `desktop-release.yml` 的 `on.push.tags` | Tauri 使用独立命名空间 `tauri-v<semver>`；文档明确 Electron 退役后再迁移 `v*` 命名空间（本次不动 Electron） |
| R2 | **跨平台 bundle 未配置**：`targets: ["app"]` 在 Windows/Linux 上无法产出安装包 | `tauri.conf.json` | 保留基础配置；CI 用 `--bundles` 按平台显式指定，并补 `.ico` 与 Windows/Linux 需要的图标集 |
| R3 | **Windows 缺 `.ico`**：Tauri Windows bundle 需要 ico | `src-tauri/icons` 只有 png | 用 Pillow 从 512 png 生成 `icon.ico`（含 16/32/48/64/128/256） |
| R4 | **版本三处重复**：Cargo workspace / client/package.json / tauri.conf.json | 三处都是 0.1.0 | 发布前用脚本强校验 Tag 与三处一致，不一致直接失败 |
| R5 | **Rust 工具链浮动**：`rust-toolchain` 缺失、CI 用 stable | 仓库无 `rust-toolchain.toml` | 新增 `rust-toolchain.toml` 固定到 1.96.0，CI/Tauri 构建共用 |
| R6 | **fmt 债务**：`crates/novel-http` 7 个文件不符合 rustfmt，现有 CI 只能查部分 crate | 现有 workflow 的 `cargo fmt --check -p ...` 子集写法 | 新 CI 沿用"子集 fmt"约定并注明债务；不顺手改 novel-http（避免与受保护文件所在的 crate 冲突） |
| R7 | **不能复用 Electron 的签名机制**：Electron 用 `CSC_LINK`/`CSC_KEY_PASSWORD` + latest.yml，且 `AI_NOVEL_ALLOW_UNSIGNED_RELEASE=true` | `desktop-release.yml` env | Tauri 流水线完全独立：不读 CSC_*，不写 latest.yml，unsigned 与 updater-signed 两态由 workflow 输入显式决定 |
| R8 | **Updater 私钥风险**：一旦提交私钥即等于把更新通道交给他人 | 无现成密钥 | 只生成一次；私钥写到家目录（600 权限），仓库只放公钥与模板；CI 从 Secrets 读取，缺失即失败（不降级） |
| R9 | **Tauri CLI 不在依赖里**：`tauri-action` 默认执行 `pnpm tauri build`，项目没有 `@tauri-apps/cli` 会直接失败 | `package.json` 无 `@tauri-apps/cli` | 增加 `@tauri-apps/cli` devDependency 并更新锁文件；若网络不可用则改用 `taiki-e/install-action` 安装 `tauri-cli`（在报告中如实标注选型与验证状态） |
| R10 | **Linux 依赖面**：webkit2gtk-4.1、libappindicator3、librsvg2、patchelf；`novel-platform` 的 `os-keychain` 在 Linux 需要 secret-service | `src-tauri/Cargo.toml`、`novel-platform` feature | ubuntu-22.04 预装官方依赖清单；Linux 构建若失败则如实标记该平台 FAIL，不用 `|| true` 掩盖 |
| R11 | **workflow_dispatch 可伪造同名正式版**：从任意分支 dispatch 也能拿到同名 Tag 的 Release | 需求 P4 | 正式版只接受 **Tag 触发**；dispatch 只在 `mode=unsigned-qa` 或 `channel=nightly` 下允许，且必须显式给 `source_sha` |
| R12 | **不完整发布**：任一矩阵失败却仍上传 latest.json | 需求 P2/P4 | `publish` job 依赖全部 build/verify，且 latest.json **最后**生成；失败时保持 draft，不触碰 stable 索引 |

## 三、本阶段决策

1. 新增三个 workflow，文件名与现有 Electron 流水线区分：`tauri-ci.yml`、`tauri-release.yml`、`tauri-nightly.yml`（不删除、不改名现有 workflow）。
2. Tag 规范：正式版 `tauri-vX.Y.Z`，预发布 `tauri-vX.Y.Z-beta.N`，nightly 不建 Tag（用 `tauri-nightly-YYYYMMDD-<sha7>` 作为 Release tag，且标记 prerelease）。
3. 三级签名模式通过 `workflow_dispatch.inputs.mode` 与 Tag 类型决定，缺密钥一律失败，不自动降级。
4. 商业证书（Apple Developer ID / Authenticode）只留占位配置与文档，不注册、不购买。
5. 本阶段不 Push、不建 Tag、不创建公开 Release，因此 **GitHub 侧所有 workflow 执行状态记为 NOT_RUN**；可本地验证的部分（脚本单测、bundle 构建、fmt/clippy/test、SHA256 与 latest.json 生成）必须实际跑并把输出留档。
