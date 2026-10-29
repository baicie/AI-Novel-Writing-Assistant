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
## 九、发布说明与 wiki

- 本阶段改动全部是工程侧（workflow、发布脚本、构建配置、图标、lint 基线），**没有用户可见的产品行为变化**，
  因此按仓库规则**不更新** `docs/releases/release-notes.md` 与 README 最新更新。
- 稳定知识（Tag 命名空间分离、签名三级模式、门禁复用与自检）已写入
  `docs/wiki/architecture/desktop-release-pipeline.md`。
## 十、预发布干跑实测（2026-10-29，真实 GitHub Actions）

把 `beta` 的版本升到 `0.1.1-beta.1` 并打 Tag `tauri-v0.1.1-beta.1`（指向 `c384726`），完整跑了一遍
`tauri-release.yml`。为了排除真实缺陷，一共跑了 5 次（前 4 次各自暴露一个问题，均已修复）：

| 轮次 | 结果 | 暴露的问题 | 修复 |
| --- | --- | --- | --- |
| 1 | Windows / macOS x64 失败 | Windows runner 默认 shell 是 pwsh，bash 语法解析失败；macOS x64 用的 macos-13 排队过久 | 构建步骤显式 `shell: bash`；macOS x64 改用 macos-14 交叉编译 |
| 2 | Windows / macOS x64 失败 | `beforeBuildCommand` 用 bash 环境变量前缀，Windows cmd 不认；交叉编译目标没装到 rust-toolchain.toml 固定的 1.95/1.96 工具链上 | 新增 `scripts/build-client-relative.mjs` 统一设置环境变量；补 `rustup target add` |
| 3 | 4 平台构建全绿，发布失败 | 上传用了全量文件，AppImage 的 AppDir 内部文件触发 `HTTP 422`，Draft 里塞进 86 个无关文件 | 上传与校验和按发布产物后缀过滤 |
| 4 | 全绿，但 Draft 含 `control.tar.gz` / `data.tar.gz` | `.tar.gz` 规则过宽，误收 deb 控制包 | 改为精确后缀（`.app.tar.gz` / `.AppImage.tar.gz` / `.nsis.zip` 等），并新增 `list-release-assets` 作为唯一判定 |
| 5 | **全部 success** | — | — |

第 5 轮 run `40828347751`（commit `c384726`）：

```text
校验 Tag、版本与签名模式        success
质量门禁 / 发布脚本单测          success
质量门禁 / 前端类型检查与单测     success
质量门禁 / Rust 格式 / Clippy / 单测 success
质量门禁 / Bundle 构建检查（macOS arm64）success
质量门禁 / Bundle 构建检查（Linux x64） success
构建 macOS arm64              success
构建 macOS x64                success
构建 Windows x64              success
构建 Linux x64                success
汇总产物并创建 Draft Release    success
```

生成的 Draft Release `tauri-v0.1.1-beta.1`（`draft=true`、`prerelease=true`、**未公开**，共 6 个资产）：

| 资产 | 大小 |
| --- | --- |
| Biz.Novel.Studio_0.1.1-beta.1_aarch64.dmg | 10.5 MB |
| Biz.Novel.Studio_0.1.1-beta.1_x64.dmg | 10.9 MB |
| Biz.Novel.Studio_0.1.1-beta.1_x64-setup.exe | 7.5 MB |
| Biz.Novel.Studio_0.1.1-beta.1_amd64.deb | 11.2 MB |
| Biz.Novel.Studio_0.1.1-beta.1_amd64.AppImage | 88.8 MB |
| SHA256SUMS.txt | 539 B |

断言成立：

- unsigned-qa 模式**没有**产出 `latest.json`（stable 索引未被触碰）
- Draft 保持未公开；Tag 推送并没有触发任何公开发布
- 资产里没有 AppImage 内部文件、没有 deb 控制包

## 十一、状态更新

```text
RELEASE_QA_ONLY
```

- 未签名 QA 构建已由真实 GitHub Actions 产出并校验（4 平台 + Draft + 校验和）
- 设置页的更新检查已接入（`UpdateCheckCard` + 可单测状态机 + `updater:default` 权限），
  但只有在产出 `latest.json` 之后客户端才真能查到更新
- updater-signed 模式仍只有本地证据（GitHub Secret 未配置）
- nightly 通道从未真实执行
- 安装/升级验证（P6）仍为 NOT_RUN

推进到 `READY_FOR_RELEASE` 还需要：配置 `TAURI_SIGNING_PRIVATE_KEY`、跑一次 updater-signed 的预发布、
跑一次 nightly、以及至少一次真实安装/启动验证。
## 十二、updater-signed 实测（2026-10-29，已配置签名私钥）

配置 `TAURI_SIGNING_PRIVATE_KEY` 后用 Tag `tauri-v0.1.1-beta.2` 反复试运行，最终产出完整的
Draft Release，签名更新通道打通。

### 产物（Draft，未公开）

```text
Biz.Novel.Studio_0.1.1-beta.2_aarch64.dmg
Biz_Novel_Studio_0.1.1-beta.2_amd64.AppImage (+ .sig)
Biz_Novel_Studio_0.1.1-beta.2_x64-setup.exe (+ .sig)
Biz_Novel_Studio_aarch64.app.tar.gz (+ .sig)
Biz_Novel_Studio_x64.app.tar.gz (+ .sig)
latest.json
SHA256SUMS.txt
```

`latest.json` 四个平台齐全，签名都是 Tauri 私钥签出的真实 minisign 文本：

```text
linux-x86_64   -> …_amd64.AppImage
windows-x86_64 -> …_x64-setup.exe
darwin-aarch64 -> …_aarch64.app.tar.gz
darwin-x86_64  -> …_x64.app.tar.gz
```

### 期间修掉的问题（全部有单测覆盖，脚本单测 31 条）

1. 更新包命名冲突：两个 macOS 架构产出同名更新包，按架构重命名后才可同时发布
2. 安装包签名误入索引：Tauri 会给所有 bundle 签名，latest.json 只应采纳更新包签名
3. 分层产物目录找不到 `.sig`：download-artifact 保留相对路径，必须递归查找
4. v1/v2 更新包格式差异：v2 用安装包本身（`-setup.exe` / `.AppImage` / `.app.tar.gz`）
5. 路径前缀被裁剪：产物落地后没有 `target/` 前缀，三元组改为直接匹配
6. 同名清单冲突：各平台目录都有一份 `SHA256SUMS.txt`，只上传汇总那份

### 环境侧记录

macOS runner 当天长时间排队，排队超过约 15 分钟的作业会被取消；本次共触发 12 次 macOS 构建，
其中 3 次拿到 runner 并成功，最终一次产出了完整 Draft。这条属于环境容量问题，不是配置缺陷。

### 结论

```text
UPDATER_SIGNED_VERIFIED（已作为预发布公开发布）
```

发布方式：Draft 内容核对后手动执行 `gh release edit tauri-v0.1.1-beta.2 --draft=false --prerelease`。
工作流自身的「正式发布」步骤只在 stable 通道且显式允许时才会执行，这次没有走那条路径。

`latest.json` 在公开后可直接下载（HTTP 200），四个平台齐全；由于仓库还没有正式版，
`releases/latest` 仍然 404，因此预发布版本不会自动推送给现有用户。

仍未执行：安装 / 升级的人工验收（P6），以及把 Draft 真正公开（需要显式授权）。
