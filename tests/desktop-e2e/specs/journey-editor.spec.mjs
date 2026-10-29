import fs from "node:fs";

const dataDir = process.env.BIZ_NOVEL_E2E_DATA_DIR;
async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Shared fixture journey: editor, history and restore", () => {
  it("keeps two writers' drafts apart, rejects a stale official save and restores history", async function () {
    this.timeout(180_000);
    const fixture = JSON.parse(fs.readFileSync(`${dataDir}/journey-fixture.json`, "utf8"));
    const novelId = fixture.novelId;
    const chapterId = fixture.chapterIds[0];
    const before = await invoke("get_chapter", { novelId, chapterId });
    const baseVersion = before.officialVersion;
    const baseUpdatedAt = String(before.updatedAt ?? "");
    expect(baseVersion).toBeGreaterThanOrEqual(1);

    const draftA = await invoke("save_chapter_draft", { request: {
      novelId, chapterId, clientId: "journey-writer-a",
      content: "作者 A 的独立草稿：祖祠之下有风。",
      baseOfficialVersion: baseVersion, baseUpdatedAt, expectedRevision: 0,
    } });
    const draftB = await invoke("save_chapter_draft", { request: {
      novelId, chapterId, clientId: "journey-writer-b",
      content: "作者 B 的独立草稿：山门之外有人。",
      baseOfficialVersion: baseVersion, baseUpdatedAt, expectedRevision: 0,
    } });
    expect(draftA.content).toContain("作者 A");
    expect(draftB.content).toContain("作者 B");
    expect(draftA.clientId).not.toBe(draftB.clientId);

    const saved = await invoke("save_chapter", {
      novelId, chapterId, expectedVersion: baseVersion,
      title: before.title, content: "正式改写：林动在祖祠守夜，听见剑鸣。",
    });
    expect(saved.officialVersion).toBe(baseVersion + 1);

    let staleRejected = false;
    try {
      await invoke("save_chapter", {
        novelId, chapterId, expectedVersion: baseVersion,
        title: before.title, content: "作者 B 用过期版本覆盖。",
      });
    } catch {
      staleRejected = true;
    }
    expect(staleRejected).toBe(true);
    const stillB = await invoke("get_chapter_draft", { novelId, chapterId, clientId: "journey-writer-b" });
    expect(stillB.content).toContain("作者 B");

    const revisions = await invoke("list_chapter_revisions", { novelId, chapterId });
    expect(revisions.length).toBeGreaterThanOrEqual(2);
    const oldest = revisions[revisions.length - 1];
    const restored = await invoke("restore_chapter_revision", {
      novelId, chapterId, revisionId: oldest.id,
      expectedVersion: saved.officialVersion,
      commandId: `journey-restore-${Date.now()}`,
      draftDecision: "keep", draftClientId: null, expectedDraftRevision: null,
    });
    expect(restored.officialVersion).toBe(saved.officialVersion + 1);
    expect(restored.content).toBe(oldest.content);

    const draftAKept = await invoke("get_chapter_draft", { novelId, chapterId, clientId: "journey-writer-a" });
    const draftBKept = await invoke("get_chapter_draft", { novelId, chapterId, clientId: "journey-writer-b" });
    expect(draftAKept.content).toContain("作者 A");
    expect(draftBKept.content).toContain("作者 B");
  });
});
