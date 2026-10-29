import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, writeFile, readFile, mkdir } from "node:fs/promises";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import os from "node:os";
import path from "node:path";

import { detectTarget, collectUpdaterPlatforms } from "./cli.mjs";

const run = promisify(execFile);
const CLI = path.join(import.meta.dirname, "cli.mjs");


import { checkWorkflowText, checkWorkflows } from "./check-workflows.mjs";

test("workflow 自检：仓库里的三个 workflow 全部通过", async () => {
  const root = path.resolve(import.meta.dirname, "../..");
  const result = await checkWorkflows(root);
  assert.deepEqual(result.errors, []);
  assert.equal(result.ok, true);
});

test("workflow 自检：\\|\\| true 与 continue-on-error 会被抓住", () => {
  const bad = "name: x\npermissions:\n  contents: read\nconcurrency:\n  group: x\nsteps:\n  - run: cargo test || true\n";
  const result = checkWorkflowText("tauri-ci.yml", bad);
  assert.equal(result.ok, false);
  assert.match(result.errors.join(), /\|\| true/);

  const masked = "name: x\npermissions:\n  contents: read\nconcurrency:\n  group: x\nsteps:\n  - continue-on-error: true\n    run: echo hi\n";
  assert.match(checkWorkflowText("tauri-ci.yml", masked).errors.join(), /continue-on-error/);
});

test("workflow 自检：release 必须只接受 tauri-v* 且先建 Draft", () => {
  const bad = "name: x\npermissions:\n  contents: write\nconcurrency:\n  group: x\non:\n  push:\n    tags:\n      - \"v*\"\n";
  const result = checkWorkflowText("tauri-release.yml", bad);
  assert.equal(result.ok, false);
  assert.match(result.errors.join(), /tauri-v\*/);
  assert.match(result.errors.join(), /Draft/);
});

test("更新包文件名映射到平台", () => {
  assert.equal(detectTarget("Biz Novel Studio_0.2.0_aarch64.app.tar.gz"), "darwin-aarch64");
  assert.equal(detectTarget("Biz Novel Studio_0.2.0_x64.app.tar.gz"), "darwin-x86_64");
  assert.equal(detectTarget("Biz Novel Studio_0.2.0_x64-setup.nsis.zip"), "windows-x86_64");
  assert.equal(detectTarget("biz-novel-studio_0.2.0_amd64.AppImage.tar.gz"), "linux-x86_64");
  assert.equal(detectTarget("Biz Novel Studio_0.2.0_aarch64.dmg"), null);
});

test("latest.json 生成：三个平台各自签名，内容取自 .sig 文件", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "latest-"));
  await writeFile(path.join(dir, "App_0.2.0_aarch64.app.tar.gz"), "mac-arm");
  await writeFile(path.join(dir, "App_0.2.0_aarch64.app.tar.gz.sig"), "sig-mac-arm\n");
  await writeFile(path.join(dir, "App_0.2.0_x64-setup.nsis.zip"), "win");
  await writeFile(path.join(dir, "App_0.2.0_x64-setup.nsis.zip.sig"), "sig-win\n");

  const { platforms, errors } = await collectUpdaterPlatforms({ dir, baseUrl: "https://example.com/download/" });
  assert.deepEqual(errors, []);
  assert.equal(platforms["darwin-aarch64"].signature, "sig-mac-arm");
  assert.equal(platforms["windows-x86_64"].url, "https://example.com/download/App_0.2.0_x64-setup.nsis.zip");

  const out = path.join(dir, "latest.json");
  await run("node", [CLI, "write-latest", "--dir", dir, "--version", "0.2.0", "--base-url", "https://example.com/download", "--pub-date", "2026-10-29T00:00:00Z", "--out", out]);
  const manifest = JSON.parse(await readFile(out, "utf8"));
  assert.equal(manifest.version, "0.2.0");
  assert.equal(Object.keys(manifest.platforms).length, 2);
  assert.equal(manifest.platforms["windows-x86_64"].signature, "sig-win");
});

test("latest.json 生成：有 .sig 但缺更新包必须失败", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "latest-bad-"));
  await writeFile(path.join(dir, "App_0.2.0_aarch64.app.tar.gz.sig"), "sig\n");
  const { platforms, errors } = await collectUpdaterPlatforms({ dir, baseUrl: "https://example.com" });
  assert.deepEqual(platforms, {});
  assert.match(errors.join("\n"), /缺少对应的更新包/);
});

test("latest.json 生成：没有签名文件必须失败（不能产出空索引）", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "latest-empty-"));
  await writeFile(path.join(dir, "App_0.2.0_aarch64.app.tar.gz"), "mac");
  const { errors } = await collectUpdaterPlatforms({ dir, baseUrl: "https://example.com" });
  assert.match(errors.join(), /没有找到任何 .sig/);
});


test("assert-bundles：按类型检查产物，缺一即失败", async () => {
  const { assertBundles } = await import("./cli.mjs");
  const dir = await mkdtemp(path.join(os.tmpdir(), "bundles-"));
  await mkdir(path.join(dir, "macos", "Biz.app"), { recursive: true });
  await writeFile(path.join(dir, "macos", "Biz.app", "Info.plist"), "plist");
  await mkdir(path.join(dir, "deb"), { recursive: true });
  await writeFile(path.join(dir, "deb", "biz_0.1.0_amd64.deb"), "deb");
  await mkdir(path.join(dir, "linux"), { recursive: true });
  await writeFile(path.join(dir, "linux", "biz_0.1.0_amd64.AppImage"), "appimage");

  const ok = await assertBundles({ dir, kinds: ["app", "deb", "appimage"] });
  assert.equal(ok.ok, true);
  assert.deepEqual(ok.missing, []);

  const missing = await assertBundles({ dir, kinds: ["app", "msi"] });
  assert.equal(missing.ok, false);
  assert.deepEqual(missing.missing, ["msi"]);
});

test("CLI write-sums 产出可校验的清单", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "sums-cli-"));
  await writeFile(path.join(dir, "a.dmg"), "aaa");
  await writeFile(path.join(dir, "b.msi"), "bbb");
  await run("node", [CLI, "write-sums", "--dir", dir]);
  const text = await readFile(path.join(dir, "SHA256SUMS.txt"), "utf8");
  assert.equal(text.trim().split("\n").length, 2);
  assert.match(text, /a\.dmg/);
  assert.match(text, /b\.msi/);
});

test("CLI verify-artifacts 缺文件时退出码非零", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "verify-cli-"));
  await writeFile(path.join(dir, "present.dmg"), "x");
  await assert.doesNotReject(() => run("node", [CLI, "verify-artifacts", "--dir", dir, "--names", "present.dmg"]));
  await assert.rejects(
    () => run("node", [CLI, "verify-artifacts", "--dir", dir, "--names", "present.dmg,absent.dmg"]),
    (error) => {
      assert.match(error.stderr, /absent\.dmg/);
      return true;
    },
  );
});
