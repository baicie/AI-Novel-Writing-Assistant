import fs from "node:fs";
import { execSync } from "node:child_process";
const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;
const binary = process.env.BIZ_NOVEL_E2E_BINARY;
async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }
async function replace(text) {
  await browser.execute((value) => {
    const el = document.querySelector("[contenteditable=true]");
    el.focus();
    document.execCommand("selectAll", false);
    document.execCommand("insertText", false, value);
  }, text);
}
describe("Two-window drafts before restart", () => {
  it("persists both writers then SIGKILLs the app", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 双窗口重启", description: "" });
    const chapter = await invoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
    await invoke("save_chapter", { novelId: novel.id, chapterId: chapter.id, expectedVersion: 0, title: "第一章", content: "初始正文" });
    await browser.execute((id, chapterId) => { window.location.hash = `#/novels/${id}/chapters/${chapterId}`; }, novel.id, chapter.id);
    await browser.pause(1200);
    await replace("重启后的A草稿");
    await browser.pause(1200);
    await invoke("open_editor_window", { novelId: novel.id, chapterId: chapter.id });
    await browser.pause(700);
    await browser.tauri.switchWindow("editor-b");
    await browser.pause(1000);
    await replace("重启后的B草稿");
    await browser.pause(1600);
    const drafts = await invoke("list_chapter_drafts", { novelId: novel.id, chapterId: chapter.id });
    expect(drafts.some((draft) => draft.clientId === "desktop-editor-b" && draft.content.includes("重启后的B草稿"))).toBe(true);
    fs.writeFileSync(`${dataDir}/draft-restart-state.json`, JSON.stringify({ novelId: novel.id, chapterId: chapter.id }));
    const pids = execSync(`pgrep -f ${JSON.stringify(binary)}`, { encoding: "utf8" }).trim().split(/\s+/).filter(Boolean);
    for (const pid of pids) process.kill(Number(pid), "SIGKILL");
    try { await browser.pause(1000); } catch { /* expected */ }
  });
});
