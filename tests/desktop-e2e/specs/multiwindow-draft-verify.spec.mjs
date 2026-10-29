import fs from "node:fs";
import path from "node:path";
async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }
describe("Two-window drafts after restart", () => {
  it("finds the second writer draft in the new process", async () => {
    const state = JSON.parse(fs.readFileSync(path.join(process.env.BIZ_NOVEL_E2E_DATA_DIR, "draft-restart-state.json"), "utf8"));
    const drafts = await invoke("list_chapter_drafts", { novelId: state.novelId, chapterId: state.chapterId });
    expect(drafts.some((draft) => draft.clientId === "desktop-editor-b" && draft.content.includes("重启后的B草稿"))).toBe(true);
  });
});
