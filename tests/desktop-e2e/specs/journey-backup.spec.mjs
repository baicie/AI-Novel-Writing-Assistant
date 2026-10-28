import fs from "node:fs";
import { execSync } from "node:child_process";

const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;
const binary = process.env.BIZ_NOVEL_E2E_BINARY;
async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Shared fixture journey: backup and restore request", () => {
  it("backs up the finished library, edits a chapter and asks for a startup restore", async function () {
    this.timeout(180_000);
    const fixture = JSON.parse(fs.readFileSync(`${dataDir}/journey-fixture.json`, "utf8"));
    const novelId = fixture.novelId;
    const chapterId = fixture.chapterIds[1];
    const before = await invoke("get_chapter", { novelId, chapterId });
    expect(before.content.length).toBeGreaterThan(10);

    const backup = await invoke("create_backup");
    expect(backup.bytes).toBeGreaterThan(0);
    const integrity = await invoke("verify_backup", { fileName: backup.fileName });
    expect(integrity).toBe("ok");

    const edited = await invoke("save_chapter", {
      novelId, chapterId, expectedVersion: before.officialVersion,
      title: before.title, content: "备份之后改动的正文：这一行应该在恢复后消失。",
    });
    expect(edited.officialVersion).toBe(before.officialVersion + 1);

    const message = await invoke("request_restore", { fileName: backup.fileName });
    expect(typeof message).toBe("string");

    fs.writeFileSync(`${dataDir}/journey-backup.json`, JSON.stringify({
      novelId,
      chapterId,
      backupFile: backup.fileName,
      restoredContent: before.content,
      editedContent: edited.content,
      versionBeforeRestore: edited.officialVersion,
    }, null, 2));

    const pids = execSync(`pgrep -f ${JSON.stringify(binary)}`, { encoding: "utf8" })
      .trim().split(/\s+/).filter(Boolean);
    if (!pids.length) throw new Error("test Tauri app pid not found");
    for (const pid of pids) process.kill(Number(pid), "SIGKILL");
    try { await browser.pause(1000); } catch { /* expected SIGKILL */ }
  });
});
