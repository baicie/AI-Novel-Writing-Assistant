# 桌面发布流水线：命名空间、签名模式与门禁

## 背景

仓库同时存在两条桌面发布线：Electron（`desktop-*.yml`，占用 `v*` 与 `desktop-*` Tag）与 Tauri（`tauri-*.yml`）。
两条线共用 GitHub Release 能力，一旦 Tag 命名空间重叠，同一次推送会触发两套流水线，产物与更新索引互相覆盖。

## 决策

1. **Tag 命名空间分离**：Tauri 只用 `tauri-v<semver>`（正式）、`tauri-v<semver>-<pre>`（预发布）、
   `tauri-nightly-YYYYMMDD-<sha7>`（夜间）。`v*` 在 Electron 退役前保持归 Electron。
2. **官方 Action 做底座**：打包用 `pnpm tauri build`（CLI 已入库为 devDependency），发布用 `gh` CLI，
   保证 Draft → 校验 → 公开每一步都可审计、可中断。
3. **三级签名**：`unsigned-qa`（默认，无密钥）、`updater-signed`（Tauri 免费密钥）、`fully-signed`（商业证书，预留）。
   缺密钥一律失败，不自动降级；稳定通道强制要求 `updater-signed`。
4. **门禁复用**：`tauri-release.yml` 与 `tauri-nightly.yml` 通过 `workflow_call` 复用 `tauri-ci.yml`，
   避免"发布流水线自己有一套更宽松的检查"。
5. **不完整版本永不出门**：任一平台失败 → Draft 保留、stable 索引不写；`publish` 必须显式 `true`。
6. **自检代替口头约定**：`scripts/tauri-release/check-workflows.mjs` 静态检查 workflow 是否出现
   `|| true`、`continue-on-error`、未固定版本的 action、release 是否先建 Draft。

## 当前规则

- 版本三处必须一致：`Cargo.toml` 的 `[workspace.package] version`、`client/package.json`、`src-tauri/tauri.conf.json`。
- 更新索引文件名区分通道：stable 用 `latest.json`，nightly 用 `nightly-latest.json`。
- Updater 公钥入库、私钥只放本地与 GitHub Secrets。
- 原生 E2E 不做常规 PR 门禁，靠 `run-native-e2e` 标签或手动触发。

## 失败模式

- **同 Tag 双流水线**：说明有人给 Tauri 用了 `v*`，改回 `tauri-v*`。
- **Draft 已建但缺平台资产**：先看矩阵里哪个平台失败；不要手动解除 Draft。
- **更新包没有 `.sig`**：`createUpdaterArtifacts` 未生效或私钥缺失；模式 B 下这是硬失败。
- **`latest.json` 平台缺失**：更新包文件名没有带 arch，映射不到平台；构建后重命名步骤被跳过。

## 相关模块

- `.github/workflows/tauri-ci.yml` / `tauri-release.yml` / `tauri-nightly.yml`
- `scripts/tauri-release/`（版本、模式、清单、workflow 自检）
- `src-tauri/tauri.updater.conf.json`（更新源与公钥）
- `docs/tauri-migration/RELEASE_CI.md`（操作手册）
