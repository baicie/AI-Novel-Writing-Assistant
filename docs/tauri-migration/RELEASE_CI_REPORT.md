# Tauri Release CI 交付报告（P0–P7）

## 一、基线与范围

- 仓库：`baicie/AI-Novel-Writing-Assistant`
- 分支：`migration/tauri-2-rust`
- 本阶段起点：`61670bc7`
- 本阶段未 Push、未建 Tag、未创建任何公开 Release（因此 GitHub Actions 侧状态一律记 NOT_RUN）

## 二、交付物

| 交付 | 路径 | 状态 |
| --- | --- | --- |
| 现有链路审计与风险清单 | `docs/tauri-migration/RELEASE_CI_AUDIT.md` | 已交付 |
| CI 流水线 | `.github/workflows/tauri-ci.yml` | 已交付（本地自检通过） |
| 正式/预发布流水线 | `.github/workflows/tauri-release.yml` | 已交付（本地自检通过） |
| Nightly 流水线 | `.github/workflows/tauri-nightly.yml` | 已交付（本地自检通过） |
| 发布脚本（版本/模式/清单/产物/自检） | `scripts/tauri-release/*` | 23 条单测通过 |
| Updater 接入 | `src-tauri/Cargo.toml`、`src-tauri/src/lib.rs`、`src-tauri/tauri.updater.conf.json` | 本地实测产出 `.sig` |
| 跨平台图标 | `src-tauri/icons/icon.ico` | 已生成（16–256 七档） |
| 工具链固定 | `rust-toolchain.toml`（1.96.0） | 已交付 |
| 操作手册 | `docs/tauri-migration/RELEASE_CI.md` | 已交付 |

## 三、本地实测证据

| 项目 | 命令 | 结果 |
| --- | --- | --- |
| 发布脚本单测 | `node --test scripts/tauri-release/*.test.mjs` | **23 passed / 0 failed** |
| Workflow 自检 | `node scripts/tauri-release/cli.mjs check-workflows` | **通过**（禁 `|| true`、禁 `continue-on-error`、禁未固定 action、release 必须 Draft + publish） |
| 版本一致性 | `cli.mjs check-versions --expected 0.1.0` | 通过；传错版本时退出码 1 |
| Tag 命名空间 | `cli.mjs parse-tag --tag v0.2.0` | 拒绝（Electron 命名空间） |
| 模式判定 | `cli.mjs resolve-mode` | 无密钥 + 正式 Tag → 失败（不降级）；预发布 Tag → unsigned-qa |
| Rust 格式 | `cargo fmt --check -p …`（迁移 9 个 crate） | 通过 |
| Clippy | `cargo clippy --workspace --exclude novel-http --lib -- -D warnings` | 通过 |
| Rust 单测 | `cargo test --workspace --exclude novel-http` | 全绿 |
| 未签名 bundle | `pnpm tauri build --target aarch64-apple-darwin --bundles app` | 产出 `Biz Novel Studio.app`（23.29 MiB） |
| Updater 签名产物 | 同上 + `--config src-tauri/tauri.updater.conf.json` + 私钥 | 产出 `*.app.tar.gz`（10.27 MiB）与 `*.app.tar.gz.sig` |
| latest.json | `cli.mjs write-latest` 指向真实产物 | 生成成功，含真实签名与下载地址 |
| SHA256 清单 | `cli.mjs write-sums` | 生成成功（`hash  文件名`） |

## 四、平台矩阵

| 平台 | CI（PR） | Release | Nightly | 真实运行 |
| --- | --- | --- | --- | --- |
| macOS arm64 | 构建 `app` | `app,dmg` | `app,dmg` | 仅本地 macOS 实测（unsigned + updater-signed） |
| macOS x64 | — | `app,dmg` | — | **NOT_RUN** |
| Windows x64 | — | `nsis` | `nsis` | **NOT_RUN** |
| Linux x64 | 构建 `deb,appimage` | `deb,appimage` | `deb,appimage` | **NOT_RUN** |

> Windows/Linux 的构建步骤、依赖清单与产物断言都已写进 workflow，但没有 GitHub runner 上的一次真实执行记录，因此不宣称通过。

## 五、三级签名模式

| 模式 | 状态 | 证据 |
| --- | --- | --- |
| A unsigned-qa | **已实现并本地验证** | 无密钥构建通过；未生成 `.sig`；workflow 默认路径 |
| B updater-signed | **已实现并本地验证** | 生成 `.app.tar.gz` + `.sig`，并据此生成 `latest.json` |
| C fully-signed | 预留（未启用） | workflow 未接 Apple/Windows 证书；文档说明扩展点 |

密钥：本机用 `tauri signer generate` 生成一份，公钥已入库到配置，私钥保存在
`~/.config/biz-novel-studio/tauri-updater.key`（600，未入库、未上传）。GitHub Secret 尚未配置（没有 Push 权限动作）。

## 六、P0–P7 逐项

| 阶段 | 结论 | 说明 |
| --- | --- | --- |
| P0 审计 | PASS | 输出风险清单 12 条与处置 |
| P1 CI 结构 | PASS | 三文件新增，既有 5 个 workflow 未改；path filter + 并发取消 + 冻结锁文件 |
| P2 跨平台构建 | PARTIAL | 矩阵与产物校验齐备；仅 macOS 有真实构建记录 |
| P3 三级签名 | PASS（A/B）/ 预留（C） | 缺密钥不降级；stable 强制模式 B |
| P4 Release 机制 | PASS（本地契约验证） | Tag→校验→门禁→Draft→校验→显式公开；GitHub 侧 NOT_RUN |
| P5 Nightly | PASS（本地契约验证） | 独立 Tag/通道/索引文件名；重复提交跳过；GitHub 侧 NOT_RUN |
| P6 安装升级验证 | NOT_RUN | 需要真实 runner 与安装环境；本阶段不虚构 |
| P7 文档交付 | PASS | 手册 + 本报告 + 审计 |

## 七、最终状态

```text
PARTIAL
```

理由：

- CI/发布/nightly 三条流水线与脚本、自检、文档、本地验证都已完成；
- 但 GitHub Actions 从未真实执行（未 Push），Windows/Linux 从未真实构建，安装与升级验证 NOT_RUN；
- 因此不能给出 `RELEASE_VERIFIED`，也不应标 `READY_FOR_RELEASE`。

推进到下一状态所需的最小动作：

1. 把本分支推到远端并观察一次 `tauri-ci` 全绿（含 Linux bundle 作业）；
2. 配置 `TAURI_SIGNING_PRIVATE_KEY` Secret；
3. 打一个 `tauri-v0.1.1-beta.1` 预发布 Tag，确认 Draft 生成、四平台资产齐全、`latest.json` 平台映射正确；
4. 需要正式发布时用 `publish=true` 重跑并把 Draft 公开。

## 八、遗留问题

| 编号 | 问题 | 影响 |
| --- | --- | --- |
| R10 | Linux/Windows 未真实构建 | 可能缺系统依赖或 path 差异，首次运行需修正 |
| R6 | `novel-http` 历史 fmt 债务 | CI 只能检查迁移 crate 子集 |
| — | 测试目标 clippy 告警未清零 | `--lib` 严格，测试目标仍宽松 |
| — | 更新检查 UI 未接入 | 模式 B 只保证产物/索引正确 |
| — | macOS 未签名未公证 | 用户首次打开需手动放行（模式 A/B 的预期行为） |
