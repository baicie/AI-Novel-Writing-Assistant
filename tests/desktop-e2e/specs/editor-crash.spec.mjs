import fs from "node:fs";
import { execSync } from "node:child_process";

const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;
const binary = process.env.BIZ_NOVEL_E2E_BINARY;

async function tauriInvoke(command, args = {}) {
  return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args);
}

describe("Chapter editor crash recovery", () => {
  it("writes a durable draft before SIGKILL", async () => {
    const novel = await tauriInvoke("create_novel", { title: "WDIO 崩溃恢复", description: "" });
    const chapter = await tauriInvoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
    await browser.execute(([n, c]) => { window.location.hash = `#/novels/${n}/chapters/${c}`; }, [novel.id, chapter.id]);
    await (await $("[data-testid=chapter-editor-surface]")).waitForDisplayed({ timeout: 20_000 });
    const editor = await $('[contenteditable="true"]');
    await editor.waitForDisplayed({ timeout: 20_000 });
    await editor.click();
    await editor.addValue("SIGKILL 前未保存的正文");
    await (await $("[data-testid=chapter-local-draft-saved]")).waitForDisplayed({ timeout: 20_000 });
    const pids = execSync(`pgrep -f ${JSON.stringify(binary)}`, { encoding: "utf8" })
      .trim().split(/\s+/).filter(Boolean);
    if (!pids.length) throw new Error("test Tauri app pid not found");
    fs.writeFileSync(`${dataDir}/editor-crash-state.json`, JSON.stringify({ novelId: novel.id, chapterId: chapter.id, pids }));
    for (const pid of pids) process.kill(Number(pid), "SIGKILL");
    try { await browser.pause(1000); } catch { /* expected: the WebView process is gone */ }
  });
});
