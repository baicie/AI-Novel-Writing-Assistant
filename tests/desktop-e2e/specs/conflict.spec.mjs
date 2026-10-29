async function invoke(command, args = {}) {
  return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args);
}
describe("Memory conflict resolution", () => {
  it("shows conflicting extracted candidates and persists an author decision", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 冲突", description: "" });
    const chapter = await invoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
    await invoke("save_chapter", {
      novelId: novel.id, chapterId: chapter.id, expectedVersion: 0, title: "第一章",
      content: "林动踏上山门石阶，夜雨压低了灯影。",
    });
    await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, novel.id);
    await browser.refresh();
    await (await $("[data-tauri-director-page]")).waitForDisplayed({ timeout: 20_000 });
    await (await $("[data-testid=director-start-button]")).click();
    await (await $("[data-testid=director-story-memory]")).waitForDisplayed({ timeout: 30_000 });
    await (await $("[data-testid=director-extract-memory]")).click();
    await (await $("[data-testid=director-memory-conflicts]")).waitForDisplayed({ timeout: 30_000 });
    await browser.waitUntil(async () => (await $("[data-testid=director-conflict-confirm-new]")).isExisting(), { timeout: 30_000, timeoutMsg: "conflict did not appear" });
    await (await $("[data-testid=director-conflict-confirm-new]")).click();
    await browser.waitUntil(async () => {
      const conflicts = await invoke("list_memory_conflicts", { novelId: novel.id });
      return conflicts.some(conflict => conflict.status === "resolved");
    }, { timeout: 20_000, timeoutMsg: "conflict decision was not persisted" });
    const facts = await invoke("list_story_facts", { novelId: novel.id, beforeChapterOrder: 2 });
    expect(facts.some(fact => fact.body.includes("E2E 记忆抽取事实"))).toBe(true);
  });
});
