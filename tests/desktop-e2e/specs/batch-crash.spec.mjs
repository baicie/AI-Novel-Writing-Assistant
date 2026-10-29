import fs from "node:fs";
import { execSync } from "node:child_process";

const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;
const binary = process.env.BIZ_NOVEL_E2E_BINARY;
async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Native batch crash persistence", () => {
  it("persists batch, item and run before SIGKILL", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 批量重启", description: "" });
    const chapterIds = [];
    for (let index = 1; index <= 3; index += 1) {
      const chapter = await invoke("create_chapter", { novelId: novel.id, title: `第${index}章`, sortOrder: index });
      chapterIds.push(chapter.id);
    }
    await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, novel.id);
    await (await $("[data-testid=batch-workspace]")).waitForDisplayed({ timeout: 20_000 });
    await (await $("[data-testid=batch-create]")).click();
    await browser.waitUntil(async () => {
      const rows = await invoke("list_batches", { novelId: novel.id });
      return rows.length === 1 && rows[0].items.some((item) => item.runId);
    }, { timeout: 20_000, timeoutMsg: "batch did not bind its first run" });
    const batch = (await invoke("list_batches", { novelId: novel.id }))[0];
    const pids = execSync(`pgrep -f ${JSON.stringify(binary)}`, { encoding: "utf8" }).trim().split(/\s+/).filter(Boolean);
    if (!pids.length) throw new Error("test Tauri app pid not found");
    fs.writeFileSync(`${dataDir}/batch-restart-state.json`, JSON.stringify({ novelId: novel.id, chapterIds, batchId: batch.id }));
    for (const pid of pids) process.kill(Number(pid), "SIGKILL");
    try { await browser.pause(1000); } catch { /* expected SIGKILL */ }
  });
});
