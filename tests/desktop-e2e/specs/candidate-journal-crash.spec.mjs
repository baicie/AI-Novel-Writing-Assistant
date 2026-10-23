import fs from "node:fs";
import { execSync } from "node:child_process";

const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;
const binary = process.env.BIZ_NOVEL_E2E_BINARY;
async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }
async function fixture(title) {
  const novel = await invoke("create_novel", { title, description: "" });
  const chapter = await invoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
  return { novelId: novel.id, chapterId: chapter.id };
}
async function open(novelId) {
  await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, novelId);
  await browser.refresh();
  await (await $("[data-testid=director-start-button]")).waitForDisplayed({ timeout: 20_000 });
}
async function approveGate() {
  await (await $("[data-testid=director-approve-button]")).waitForDisplayed({ timeout: 45_000 });
  const detail = await invoke("get_run_detail", {
    novelId: JSON.parse(fs.readFileSync(`${dataDir}/candidate-state.json`, "utf8")).novelId,
    runId: JSON.parse(fs.readFileSync(`${dataDir}/candidate-state.json`, "utf8")).runId,
  });
  await (await $("[data-testid=director-approve-button]")).click();
  await browser.waitUntil(async () => {
    const next = await invoke("get_run_detail", { novelId: JSON.parse(fs.readFileSync(`${dataDir}/candidate-state.json`, "utf8")).novelId, runId: JSON.parse(fs.readFileSync(`${dataDir}/candidate-state.json`, "utf8")).runId });
    return next.controlVersion > detail.controlVersion;
  }, { timeout: 30_000 });
}

describe("Candidate edit journal SIGKILL", () => {
  it("persists an unconfirmed candidate edit and stops the app by SIGKILL", async () => {
    const f = await fixture("WDIO 候选草稿崩溃");
    await open(f.novelId);
    await (await $("[data-testid=director-start-button]")).click();
    await (await $("[data-testid=director-run-meta]")).waitForDisplayed({ timeout: 30_000 });
    const meta = await (await $("[data-testid=director-run-meta]")).getText();
    const runId = meta.match(/Run ([^ ]+)/)?.[1] ?? "";
    fs.writeFileSync(`${dataDir}/candidate-state.json`, JSON.stringify({ ...f, runId }));
    await approveGate();
    await approveGate();
    await (await $("[data-testid=director-approve-button]")).waitForDisplayed({ timeout: 45_000 });
    await (await $("[data-testid=director-edit-candidate-button]")).click();
    const input = await $("[data-testid=director-candidate-edit-input]");
    await input.waitForDisplayed({ timeout: 20_000 });
    await input.clearValue();
    await input.addValue("候选草稿跨进程恢复正文");
    await browser.waitUntil(async () => (await (await $("[data-testid=director-candidate-draft-status]")).getText()).includes("已本地保存"), { timeout: 20_000, timeoutMsg: "candidate journal was not saved" });
    fs.writeFileSync(`${dataDir}/candidate-state.json`, JSON.stringify({ ...f, runId, edited: "候选草稿跨进程恢复正文" }));
    const pids = execSync(`pgrep -f ${JSON.stringify(binary)}`, { encoding: "utf8" }).trim().split(/\s+/).filter(Boolean);
    for (const pid of pids) process.kill(Number(pid), "SIGKILL");
    try { await browser.pause(1000); } catch { /* expected */ }
  });
});
