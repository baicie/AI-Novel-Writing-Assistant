# Tauri Release CI 操作手册

## 1. 结构

| 文件 | 触发 | 作用 |
| --- | --- | --- |
| `.github/workflows/tauri-ci.yml` | PR / 推送到 `migration/tauri-2-rust`、`main`（仅 Tauri 路径）/ 可复用调用 | 质量门禁 + 双平台真实 bundle 检查 |
| `.github/workflows/tauri-release.yml` | Tag `tauri-v*` / 手动（受控） | 正式与预发布：Draft → 校验 → 显式公开 |
| `.github/workflows/tauri-nightly.yml` | 每日 18:00 UTC / 手动 | 夜间 Prerelease，独立通道 |

既有的 `desktop-release.yml`、`desktop-beta-release.yml`、`desktop-dev-release.yml`（Electron）、`site-pages.yml`、
`tauri-kernel-check.yml` **均未改动**。Tauri 与 Electron 分属不同 Tag 命名空间，互不覆盖。

## 2. Tag 规范

| 类型 | 形式 | 通道 | 是否可自动更新 |
| --- | --- | --- | --- |
| 正式版 | `tauri-v1.2.3` | stable | 仅 updater-signed 且显式 `publish=true` |
| 预发布 | `tauri-v1.2.3-beta.1` | prerelease | 可选 |
| 夜间版 | `tauri-nightly-YYYYMMDD-<sha7>` | nightly | 独立索引文件 |
| Electron（既有） | `v1.2.3` / `desktop-v*` | release | 与本流水线无关 |

> `v*` 仍归 Electron。等 Electron 发布线退役后，再单独迁移命名空间。

## 3. 三级签名模式

| 模式 | 启用条件 | 产物 | 商业证书 |
| --- | --- | --- | --- |
| **A unsigned-qa**（默认） | 无任何密钥 | 普通安装包，**不生成** `.sig` 与 `latest.json` | 不需要 |
| **B updater-signed** | 配置 `TAURI_SIGNING_PRIVATE_KEY` | 安装包 + `*.app.tar.gz/.nsis.zip/.AppImage.tar.gz` + `.sig` + `latest.json` | 不需要 |
| **C fully-signed**（预留） | Apple Developer ID / Windows Authenticode | 同上 + 系统级签名/公证 | 需要（本阶段不购买） |

规则：

- 模式 A 不上传 stable 索引，也不宣称 Gatekeeper / SmartScreen 信任。
- 模式 B 缺私钥 **立即失败**，不会自动降级成 unsigned。
- 正式版（`tauri-vX.Y.Z`）只允许模式 B；模式 A 只能出预发布/QA。
- 模式 C 只需往 `tauri-release.yml` 的 build 步骤补签名 env 与 keychain 导入，不需要改其它逻辑。

## 4. GitHub Secrets 清单

| Secret | 必需性 | 说明 |
| --- | --- | --- |
| `GITHUB_TOKEN` | 自动 | 创建 Draft/Prerelease 与上传资产（`contents: write`） |
| `TAURI_SIGNING_PRIVATE_KEY` | 模式 B 必需 | 私钥文件内容（或 `signer generate` 输出的整段文本） |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 私钥有密码时必需 | 生成时留空则不需要 |

本阶段**不配置**：`APPLE_CERTIFICATE`、`APPLE_ID`、`APPLE_TEAM_ID`、`WINDOWS_CERTIFICATE` 等商业证书相关 Secrets。

## 5. 生成 Updater 签名密钥（免费）

```bash
pnpm tauri signer generate --ci -p "" -w ~/.config/biz-novel-studio/tauri-updater.key
```

- 私钥：`~/.config/biz-novel-studio/tauri-updater.key`（权限 600，**永不入库**）
- 公钥：`~/.config/biz-novel-studio/tauri-updater.key.pub` → 已写入 `src-tauri/tauri.updater.conf.json` 的 `plugins.updater.pubkey`
- 把私钥整段内容填到 GitHub Secret `TAURI_SIGNING_PRIVATE_KEY`；如果生成时设了密码，再填 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
- 轮换：重新生成后同时更新 `pubkey` 与 Secret；已发布的旧版本会因公钥不匹配拒绝该更新，必须发一次新版本让用户过渡

> 私钥丢失意味着无法再为已安装用户签名更新，只能让他们手动重装。

## 6. 正式发布操作

1. 确认三处版本一致：`Cargo.toml` 的 `[workspace.package] version`、`client/package.json`、`src-tauri/tauri.conf.json`
   （`pnpm run tauri:release:check-versions` 会校验）
2. 打 Tag 并推送：`git tag tauri-v1.2.3 && git push origin tauri-v1.2.3`
3. workflow 自动：校验 Tag/版本/模式 → 跑质量门禁 → 四平台并行构建 → 汇总 `SHA256SUMS.txt`（模式 B 额外生成 `latest.json`）→ **创建 Draft Release** 并上传
4. 人工检查 Draft：资产数量、每个平台都有安装包、`SHA256SUMS.txt` 可核对、（模式 B）`latest.json` 的平台映射与签名
5. 公开：
   - 方式一：GitHub 页面把 Draft 转为正式发布
   - 方式二：`workflow_dispatch` 传 `publish=true` 重跑（仅 `channel=stable` 且模式 B 生效）

失败时 Draft 保持不动，stable 索引不会被更新。

## 7. Nightly 操作

- 定时：每天 18:00 UTC；也可手动触发（`force=true` 可强制重建同一提交）
- 版本号：`<base>-nightly.YYYYMMDD`，只写进本次构建的 `src-tauri/tauri.nightly.conf.json`，**不改源码版本**
- 输出：Prerelease + `SHA256SUMS.txt`；配置了私钥时额外产出 `nightly-latest.json`（独立文件名）
- 断言：nightly 资产里不允许出现 `latest.json`

## 8. 本地等价验证

```bash
pnpm run test:release-scripts        # 23 条脚本单测
pnpm run tauri:ci:check-workflows    # workflow 自检（禁止 || true / continue-on-error）
pnpm run tauri:release:check-versions
cargo clippy --workspace --exclude novel-http --lib -- -D warnings
cargo test --workspace --exclude novel-http
pnpm tauri build --target aarch64-apple-darwin --bundles app
TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.config/biz-novel-studio/tauri-updater.key)" \
  pnpm tauri build --target aarch64-apple-darwin --bundles app --config src-tauri/tauri.updater.conf.json
```

## 9. 已知缺口（按风险编号）

见 `docs/tauri-migration/RELEASE_CI_AUDIT.md`。当前仍未覆盖：

- **R10 已关闭**：Linux/Windows 已在 2026-10-29 的预发布干跑中真实构建成功（见交付报告第十节）
- **R6** `crates/novel-http` 的历史 rustfmt 债务未清理，CI 只检查迁移 crate 子集
- 测试目标的 clippy 债务未清零（`--lib` 已严格，测试目标仍有历史告警）
- macOS 未做 Developer ID 签名与公证（模式 C），用户首次打开仍需手动放行
- 更新检查 UI 尚未接入：模式 B 只保证产物与索引正确，客户端还没有"检查更新"入口
