import fs from "node:fs";

const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;
async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Shared fixture journey: restore verification", () => {
  it("starts on the restored database and keeps the protection copy and everything before the edit", async function () {
    this.timeout(180_000);
    const state = JSON.parse(fs.readFileSync(`${dataDir}/journey-backup.json`, "utf8"));
    const fixture = JSON.parse(fs.readFileSync(`${dataDir}/journey-fixture.json`, "utf8"));

    const chapter = await invoke("get_chapter", { novelId: state.novelId, chapterId: state.chapterId });
    expect(chapter.content).toBe(state.restoredContent);
    expect(chapter.content).not.toContain("备份之后改动的正文");

    const backups = await invoke("list_backups");
    expect(backups.some((item) => String(item.fileName).startsWith("pre-restore-"))).toBe(true);
    expect(backups.some((item) => item.fileName === state.backupFile)).toBe(true);

    const draft = await invoke("get_chapter_draft", {
      novelId: state.novelId, chapterId: fixture.chapterIds[0], clientId: "journey-writer-a",
    });
    expect(draft.content).toContain("作者 A");

    const batches = await invoke("list_batches", { novelId: state.novelId });
    expect(batches[0].status).toBe("completed");
    expect(batches[0].items.every((item) => item.status === "succeeded")).toBe(true);

    const facts = await invoke("list_structured_facts", { novelId: state.novelId, entityId: fixture.characterIds[0] });
    const locations = facts.filter((item) => item.factType === "location" && item.status === "active");
    expect(locations).toHaveLength(1);
    expect(locations[0].valueJson).toContain("祖祠");

    const hits = await invoke("search_knowledge", { novelId: state.novelId, query: "剑谱", limit: 5 });
    expect(hits.some((hit) => hit.body.includes("剑谱"))).toBe(true);

    const wholeBook = await invoke("export_novel_selection", {
      novelId: state.novelId, selection: "entire_book", format: "markdown",
    });
    expect(wholeBook).toContain("第10章");
  });
});
