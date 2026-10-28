import fs from "node:fs";
import path from "node:path";
import { execSync } from "node:child_process";

const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;
const binary = process.env.BIZ_NOVEL_E2E_BINARY;
async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Backup crash window", () => {
  it("kills the app between writing the temporary copy and publishing the backup", async function () {
    this.timeout(180_000);
    const backupDir = path.join(dataDir, "backups");
    const created = await invoke("create_novel", { title: "备份崩溃样本", description: "" });
    const novelId = created.id;
    await invoke("create_chapter", { novelId, title: "第一章", sortOrder: 1 });
    const chapters = await invoke("list_chapters", { novelId });
    await invoke("save_chapter", {
      novelId, chapterId: chapters[0].id, expectedVersion: 0,
      title: "第一章", content: "崩溃窗口用例的正文。",
    });

    await browser.execute(() => { window.__TAURI__.core.invoke("create_backup").catch(() => {}); });
    await browser.pause(3000);
    const pending = fs.readdirSync(backupDir).filter((name) => name.endsWith(".tmp"));
    expect(pending.length).toBeGreaterThan(0);
    const published = fs.readdirSync(backupDir).filter((name) => name.startsWith("novel-") && name.endsWith(".sqlite"));
    expect(published).toHaveLength(0);

    fs.writeFileSync(`${dataDir}/backup-crash.json`, JSON.stringify({ novelId, pending }, null, 2));
    const pids = execSync(`pgrep -f ${JSON.stringify(binary)}`, { encoding: "utf8" }).trim().split(/\s+/).filter(Boolean);
    if (!pids.length) throw new Error("test Tauri app pid not found");
    for (const pid of pids) process.kill(Number(pid), "SIGKILL");
    try { await browser.pause(1000); } catch { /* expected SIGKILL */ }
  });
});
