import fs from "node:fs";
import { execSync } from "node:child_process";

const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;
const binary = process.env.BIZ_NOVEL_E2E_BINARY;
async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Native backup restore request", () => {
  it("creates a backup, changes data, requests restore, then SIGKILLs the app", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 恢复重启", description: "" });
    const chapter = await invoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
    await invoke("save_chapter", { novelId: novel.id, chapterId: chapter.id, expectedVersion: 0, title: "第一章", content: "备份前正文" });
    const backup = await invoke("create_backup");
    await invoke("save_chapter", { novelId: novel.id, chapterId: chapter.id, expectedVersion: 1, title: "第一章", content: "备份后正文" });
    const message = await invoke("request_restore", { fileName: backup.fileName });
    expect(message).toContain("重新打开应用");
    fs.writeFileSync(`${dataDir}/restore-state.json`, JSON.stringify({ novelId: novel.id, chapterId: chapter.id, backup: backup.fileName }));
    const pids = execSync(`pgrep -f ${JSON.stringify(binary)}`, { encoding: "utf8" }).trim().split(/\s+/).filter(Boolean);
    for (const pid of pids) process.kill(Number(pid), "SIGKILL");
    try { await browser.pause(1000); } catch { /* expected */ }
  });
});
