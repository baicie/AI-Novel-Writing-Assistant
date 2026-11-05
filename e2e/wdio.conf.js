// P16 原生 E2E：用 @wdio/tauri-service 驱动带 e2e 特性的桌面产物。
// 需要先构建：pnpm tauri build --features e2e --bundles app
export const config = {
  runner: "local",
  specs: [
    "./provider-smoke.e2e.js",
    "./provider-flow.e2e.js",
    "./provider-no-missing-routes.e2e.js",
    "./quick-setup-models.e2e.js",
    "./provider-selector.e2e.js",
    "./provider-protocol-guard.e2e.js",
    "./author-journey.e2e.js",
    "./p17-workbench.e2e.js",
    "./p17-cancel-navigation.e2e.js",
    "./p17-selection-rewrite.e2e.js",
    "./p17-edit-position.e2e.js",
    "./p17-3-continuity.e2e.js",
    "./p17-3-context.e2e.js",
    "./p17-4-plot-timeline.e2e.js",
  ],
  // 这些用例共用同一份本地数据目录与同一个应用窗口：
  // 并行跑会让两个实例争抢 SQLite 与窗口，产生与代码无关的失败。
  // wdio 9 只认 root 级的 maxInstances，capability 内的同名字段不会限制 worker 数。
  maxInstances: 1,
  maxInstancesPerCapability: 1,
  logLevel: "warn",
  waitforTimeout: 15000,
  connectionRetryTimeout: 120000,
  connectionRetryCount: 1,
  framework: "mocha",
  mochaOpts: { ui: "bdd", timeout: 60000 },
  reporters: ["spec"],
  services: ["@wdio/tauri-service"],
  capabilities: [
    {
      browserName: "tauri",
      "tauri:options": {
        // 默认优先用带 e2e 特性的产物：普通产物没有注册 wdio 插件，
        // 用它会在 onPrepare 阶段等待 WebDriver 直到超时。
        application:
          process.env.P16_E2E_APP ??
          "./target/aarch64-apple-darwin/release/bundle/macos/Biz Novel Studio E2E.app/Contents/MacOS/novel-desktop",
      },
    },
  ],
};
