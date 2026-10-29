import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, writeFile, mkdir } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { parseReleaseTag, readVersions, assertVersionConsistency } from "./versions.mjs";
import { resolveMode } from "./modes.mjs";
import { buildLatestJson, sha256SumsText, verifyArtifacts } from "./manifest.mjs";

process.env.NODE_NO_WARNINGS = "1";

test("tag 解析：正式版、预发布、nightly 与非 Tauri tag", () => {
  assert.deepEqual(parseReleaseTag("tauri-v0.2.0"), { kind: "release", version: "0.2.0", prerelease: null });
  assert.equal(parseReleaseTag("tauri-v0.2.0-beta.1").kind, "prerelease");
  assert.equal(parseReleaseTag("tauri-v0.2.0-beta.1").version, "0.2.0-beta.1");
  assert.deepEqual(parseReleaseTag("tauri-nightly-20260101-abc1234"), { kind: "nightly", date: "20260101", sha: "abc1234" });
  assert.equal(parseReleaseTag("v0.2.0").kind, "invalid", "Electron 的 v* 命名空间不能被 Tauri 复用");
  assert.equal(parseReleaseTag("desktop-v0.2.0").kind, "invalid");
});

test("版本一致性：三处一致才算通过", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "ver-ok-"));
  await writeFile(path.join(root, "Cargo.toml"), "[workspace.package]\nversion = \"0.2.0\"\n");
  await mkdir(path.join(root, "client"), { recursive: true });
  await writeFile(path.join(root, "client/package.json"), JSON.stringify({ version: "0.2.0" }));
  await mkdir(path.join(root, "src-tauri"), { recursive: true });
  await writeFile(path.join(root, "src-tauri/tauri.conf.json"), JSON.stringify({ version: "0.2.0" }));

  assert.deepEqual(await readVersions(root), { cargo: "0.2.0", client: "0.2.0", tauri: "0.2.0" });
  assert.equal((await assertVersionConsistency({ root, expected: "0.2.0" })).ok, true);
  const mismatch = await assertVersionConsistency({ root, expected: "0.3.0" });
  assert.equal(mismatch.ok, false);
  assert.match(mismatch.errors.join(), /不一致/);
});

test("版本一致性：三处不一致必须失败", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "ver-bad-"));
  await writeFile(path.join(root, "Cargo.toml"), "[workspace.package]\nversion = \"0.2.0\"\n");
  await mkdir(path.join(root, "client"), { recursive: true });
  await writeFile(path.join(root, "client/package.json"), JSON.stringify({ version: "0.2.1" }));
  await mkdir(path.join(root, "src-tauri"), { recursive: true });
  await writeFile(path.join(root, "src-tauri/tauri.conf.json"), JSON.stringify({ version: "0.2.0" }));
  const result = await assertVersionConsistency({ root });
  assert.equal(result.ok, false);
  assert.match(result.errors.join(), /三处版本不一致/);
});

test("模式解析：正式 Tag 无密钥时拒绝进入 stable", () => {
  const result = resolveMode({ eventName: "push", tag: "tauri-v0.2.0", hasUpdaterKey: false });
  assert.equal(result.ok, false);
  assert.equal(result.channel, "stable");
  assert.match(result.errors.join(), /stable 通道必须使用 updater-signed/);
});

test("模式解析：正式 Tag 有密钥即可 updater-signed", () => {
  const result = resolveMode({ eventName: "push", tag: "tauri-v0.2.0", hasUpdaterKey: true });
  assert.equal(result.ok, true);
  assert.equal(result.mode, "updater-signed");
  assert.equal(result.createUpdaterArtifacts, true);
  assert.equal(result.version, "0.2.0");
});

test("模式解析：预发布 Tag 允许 unsigned QA 且不生成更新产物", () => {
  const result = resolveMode({ eventName: "push", tag: "tauri-v0.2.0-beta.1", hasUpdaterKey: false });
  assert.equal(result.ok, true);
  assert.equal(result.mode, "unsigned-qa");
  assert.equal(result.channel, "prerelease");
  assert.equal(result.createUpdaterArtifacts, false);
});

test("模式解析：显式要求 updater-signed 却没有私钥时失败，不降级", () => {
  const result = resolveMode({ eventName: "workflow_dispatch", tag: "tauri-v0.2.0", inputs: { mode: "updater-signed", source_sha: "abc1234" }, hasUpdaterKey: false });
  assert.equal(result.ok, false);
  assert.match(result.errors.join(), /缺少 TAURI_SIGNING_PRIVATE_KEY/);
});

test("模式解析：手动触发必须给 source_sha", () => {
  const result = resolveMode({ eventName: "workflow_dispatch", tag: "tauri-v0.2.0-beta.2", inputs: { mode: "unsigned-qa" }, hasUpdaterKey: false });
  assert.equal(result.ok, false);
  assert.match(result.errors.join(), /source_sha/);
});

test("模式解析：nightly tag 默认 unsigned QA", () => {
  const result = resolveMode({ eventName: "push", tag: "tauri-nightly-20260101-abc1234", hasUpdaterKey: false });
  assert.equal(result.ok, true);
  assert.equal(result.channel, "nightly");
  assert.equal(result.mode, "unsigned-qa");
  assert.equal(result.nightlySha, "abc1234");
});

test("latest.json：缺签名或缺地址必须失败", () => {
  const bad = buildLatestJson({
    version: "0.2.0",
    pubDate: "2026-10-29T00:00:00Z",
    platforms: { "darwin-aarch64": { signature: "", url: "" } },
  });
  assert.equal(bad.ok, false);
  assert.equal(bad.errors.length, 2);

  const unknown = buildLatestJson({
    version: "0.2.0",
    pubDate: "2026-10-29T00:00:00Z",
    platforms: { "plan9-x86_64": { signature: "s", url: "u" } },
  });
  assert.equal(unknown.ok, false);
  assert.match(unknown.errors.join(), /未知平台标识/);

  const good = buildLatestJson({
    version: "0.2.0",
    pubDate: "2026-10-29T00:00:00Z",
    platforms: {
      "darwin-aarch64": { signature: "sig-a", url: "https://example.com/a.app.tar.gz" },
      "windows-x86_64": { signature: "sig-w", url: "https://example.com/w.nsis.zip" },
    },
  });
  assert.equal(good.ok, true);
  assert.equal(good.manifest.platforms["windows-x86_64"].signature, "sig-w");
});

test("latest.json：非 SemVer 版本失败", () => {
  const result = buildLatestJson({ version: "v0.2.0", pubDate: "2026-10-29T00:00:00Z", platforms: { "darwin-aarch64": { signature: "s", url: "u" } } });
  assert.equal(result.ok, false);
  assert.match(result.errors.join(), /SemVer/);
});

test("SHA256 清单：按文件名排序且每行两空格分隔", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "sums-"));
  const a = path.join(dir, "b.bin");
  const b = path.join(dir, "a.bin");
  await writeFile(a, "bravo");
  await writeFile(b, "alpha");
  const text = await sha256SumsText([{ path: a }, { path: b }]);
  const lines = text.trim().split("\n");
  assert.equal(lines.length, 2);
  assert.match(lines[0], /^[0-9a-f]{64} {2}a\.bin$/);
  assert.match(lines[1], /^[0-9a-f]{64} {2}b\.bin$/);
});

test("产物校验：缺失或空文件必须失败", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "art-"));
  const ok = path.join(dir, "ok.bin");
  await writeFile(ok, "payload");
  const empty = path.join(dir, "empty.bin");
  await writeFile(empty, "");
  const result = await verifyArtifacts([{ path: ok }, { path: empty }, { path: path.join(dir, "missing.bin") }]);
  assert.equal(result.ok, false);
  assert.equal(result.present.length, 1);
  assert.equal(result.missing.length, 2);
});
