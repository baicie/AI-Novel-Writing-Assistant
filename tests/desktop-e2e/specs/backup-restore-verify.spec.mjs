import fs from "node:fs";
import path from "node:path";

async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Native backup restore startup", () => {
  it("applies the staged backup before opening the database and preserves the original", async () => {
    const state = JSON.parse(fs.readFileSync(path.join(process.env.BIZ_NOVEL_E2E_DATA_DIR, "restore-state.json"), "utf8"));
    const chapter = await invoke("get_chapter", { novelId: state.novelId, chapterId: state.chapterId });
    expect(chapter.content).toBe("备份前正文");
    expect(chapter.officialVersion).toBe(1);
    const backups = await invoke("list_backups");
    expect(backups.some((backup) => backup.fileName.startsWith("pre-restore-"))).toBe(true);
    const novels = await invoke("list_novels");
    expect(novels.some((novel) => novel.id === state.novelId)).toBe(true);
  });
});
