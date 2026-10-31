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
  ],
  maxInstances: 1,
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
        application:
          process.env.P16_E2E_APP ??
          "./target/aarch64-apple-darwin/release/bundle/macos/Biz Novel Studio.app/Contents/MacOS/novel-desktop",
      },
    },
  ],
};
