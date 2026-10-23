import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { startMockProvider } from "./support/mock-provider.mjs";

const root = path.resolve(import.meta.dirname, "../..");
const reuseDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;
const dataDir = reuseDir || fs.mkdtempSync(path.join(os.tmpdir(), "biz-novel-wdio-"));
const keepData = process.env.BIZ_NOVEL_E2E_KEEP === "1" || Boolean(reuseDir);
const binary = path.join(root, "target/aarch64-apple-darwin/release/novel-desktop");
let mock;

export const config = {
  runner: "local",
  specs: ["./specs/director.spec.mjs"],
  maxInstances: 1,
  services: [["@wdio/tauri-service", {
    appBinaryPath: binary,
    driverProvider: "embedded",
    embeddedPort: 4445,
    startTimeout: 90_000,
    commandTimeout: 30_000,
    captureBackendLogs: true,
    captureFrontendLogs: true,
    backendLogLevel: "info",
    frontendLogLevel: "info",
    logLevel: "info",
  }]],
  capabilities: [{
    browserName: "tauri",
    "tauri:options": { application: binary },
  }],
  logLevel: "info",
  bail: 0,
  waitforTimeout: 20_000,
  connectionRetryTimeout: 120_000,
  connectionRetryCount: 2,
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: { ui: "bdd", timeout: 180_000 },
  onPrepare: async () => {
    mock = await startMockProvider();
    process.env.BIZ_NOVEL_E2E_ENDPOINT = mock.endpoint;
    process.env.BIZ_NOVEL_DATA_DIR = dataDir;
    process.env.BIZ_NOVEL_E2E_SCENARIO = process.env.BIZ_NOVEL_E2E_SCENARIO ?? "clean";
    process.env.BIZ_NOVEL_E2E_DATA_DIR = dataDir;
    process.env.BIZ_NOVEL_E2E_BINARY = binary;
    console.log(`[e2e] data=${dataDir} endpoint=${mock.endpoint}`);
  },
  onComplete: async () => {
    await mock?.close();
    if (!keepData) fs.rmSync(dataDir, { recursive: true, force: true });
  },
  afterTest: async function (test, context, { error }) {
    if (!error) return;
    const shotDir = path.join(root, "tests/desktop-e2e/artifacts");
    fs.mkdirSync(shotDir, { recursive: true });
    const name = test.title.replace(/[^a-z0-9]+/gi, "-").replace(/^-|-$/g, "") || "failure";
    try { await browser.saveScreenshot(path.join(shotDir, `${name}.png`)); } catch {}
    try {
      const body = await browser.execute(() => document.body.innerText.slice(0, 4000));
      fs.writeFileSync(path.join(shotDir, `${name}.txt`), body);
    } catch {}
  },
};
