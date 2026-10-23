# macOS Native UI E2E

基线：`f84c704a`。本轮新增真实 macOS WKWebView WebDriverIO 测试，不再以页面存在性 smoke 代替按钮操作。

## 环境

- macOS 15.7.7 arm64，Aqua 图形会话可用
- Node 22.12.0，pnpm 10.6.0
- Rust 1.96.0，Tauri 2.12.3
- WebdriverIO 9.31.9
- `@wdio/tauri-service` 1.5.0
- `@wdio/tauri-plugin` 1.5.0
- Rust `tauri-plugin-wdio` 1.5.0
- Rust `tauri-plugin-wdio-webdriver` 1.5.0
- driverProvider：`embedded`
- embedded WebDriver port：4445，仅绑定本机

## E2E 专用构建

生产配置保持：

- `withGlobalTauri: false`
- capabilities 只加载 `default`
- 无 WDIO 插件依赖

E2E 使用独立配置：

```bash
cargo tauri build \
  --features e2e \
  --config src-tauri/tauri.e2e.conf.json \
  --target aarch64-apple-darwin \
  --bundles app
```

E2E 配置：

- `productName: Biz Novel Studio E2E`
- `identifier: com.ai-novel.studio.e2e`
- `withGlobalTauri: true`
- `capabilities: [default, e2e]`
- `frontendDist: ../client/dist-e2e`
- `beforeBuildCommand: pnpm --filter @ai-novel/client build:e2e`

Rust 插件只在 `#[cfg(feature = "e2e")]` 注册：

```rust
#[cfg(feature = "e2e")]
let builder = builder
    .plugin(tauri_plugin_wdio::init())
    .plugin(tauri_plugin_wdio_webdriver::init());
```

前端插件只在 `VITE_TAURI_E2E=1` 的 E2E 构建导入。

## 测试 Fixture

`tests/desktop-e2e/`：

- 临时 SQLite：`BIZ_NOVEL_DATA_DIR`
- 临时测试数据目录，不读取真实用户书库
- `tests/desktop-e2e/support/mock-provider.mjs`：本机 HTTP/SSE 模型
- 假 Key：E2E runtime 专用 `E2eModelSecrets`
- mock endpoint 由 `BIZ_NOVEL_E2E_ENDPOINT` 注入，启动时写入临时模型设置
- 测试结束删除临时目录（保留重启场景）
- 通过真实 Rust Runtime、Tauri Commands、SQLite、Workflow、Provider HTTP 路径

Mock Provider 返回：

- Story Macro
- Character Cast
- Volume Strategy
- Chapter Plan
- Chapter Draft
- Consistency Review
- 场景 B：`issues > 0`

## 已通过的真实按钮测试

```bash
pnpm --filter @ai-novel/desktop-e2e test:director
```

结果：**2 independent tests × 3 rounds 全通过**

1. 独立创建测试小说，渲染 Tauri Director 页面和真实启动表单
2. 独立创建另一测试小说，点击“提交并开始创作”创建真实 Rust Run；点击 Character Cast Gate、Volume Strategy Gate、Chapter Approval；Run succeeded；正式正文写入；页面刷新恢复

连续 3 轮：16s / 14s / 24s，均 pass，无 flaky 失败。

```bash
pnpm --filter @ai-novel/desktop-e2e test:cancel
```

结果：**1 passing**

- 点击取消
- UI 显示“已取消”
- Run detail status = cancelled
- 正式正文为空

```bash
BIZ_NOVEL_E2E_SCENARIO=issues pnpm --filter @ai-novel/desktop-e2e test:safety
```

结果：**1 passing**

- 真实审校 issues > 0
- UI 显示暂停
- `pauseKind = safety`
- 无 repair_notes、无 chapter_commit
- 无普通 Resume 按钮
- 正文为空

重启持久化使用同一临时数据目录启动第二个 WDIO 进程：

```bash
# 首次运行保留数据
BIZ_NOVEL_E2E_KEEP=1 pnpm --filter @ai-novel/desktop-e2e test:director

# 用上一步打印的数据目录启动新进程
BIZ_NOVEL_E2E_DATA_DIR=<temp-dir> pnpm --filter @ai-novel/desktop-e2e test:restart
```

结果：**1 passing**

- 新 App 进程
- 正式正文和 officialVersion 存在
- Run 历史状态 succeeded
- story_macro / chapter_commit artifact 存在

## 真实交互边界

已通过真实按钮操作：

- 启动
- Character Cast 确认
- Volume Strategy 确认
- Chapter Approval 确认
- 取消
- Safety 暂停展示
- 页面刷新恢复
- 独立进程重启恢复

未覆盖：

- 连续双击启动/确认
- 两个窗口同时启动
- 过期 expectedVersion 的 UI 展示（后端已有测试）
- Channel 断线重连的 UI 断言
- 卸载后订阅对象计数
- 中文拼音输入法 composition
- 撤销/重做与跨章节编辑器完整交互

这些未完成项不得用 DOM 存在性代替。
