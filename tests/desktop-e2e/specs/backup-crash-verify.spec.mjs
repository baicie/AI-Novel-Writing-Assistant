import fs from "node:fs";
import path from "node:path";

const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;
async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Backup crash window: restart", () => {
  it("never shows the half-written file as a backup and keeps backing up afterwards", async function () {
    this.timeout(180_000);
    const state = JSON.parse(fs.readFileSync(`${dataDir}/backup-crash.json`, "utf8"));
    const backupDir = path.join(dataDir, "backups");

    const listed = await invoke("list_backups");
    expect(listed).toHaveLength(0);
    expect(fs.readdirSync(backupDir).some((name) => name.endsWith(".tmp"))).toBe(true);

    const chapters = await invoke("list_chapters", { novelId: state.novelId });
    expect(chapters[0].content).toContain("崩溃窗口");

    const fresh = await invoke("create_backup");
    expect(fresh.bytes).toBeGreaterThan(0);
    expect(await invoke("verify_backup", { fileName: fresh.fileName })).toBe("ok");

    const after = await invoke("list_backups");
    expect(after).toHaveLength(1);
    expect(after[0].fileName).toBe(fresh.fileName);

    let rejected = false;
    try { await invoke("verify_backup", { fileName: "../novel.sqlite" }); } catch { rejected = true; }
    expect(rejected).toBe(true);
  });
});
