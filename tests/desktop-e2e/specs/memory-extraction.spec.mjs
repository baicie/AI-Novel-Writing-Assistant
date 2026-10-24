async function invoke(command, args = {}) {
  return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args);
}
describe("AI memory extraction review", () => {
  it("extracts a candidate from an official chapter, confirms it, and exposes it to Context", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 记忆抽取", description: "" });
    const chapter = await invoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
    await invoke("save_chapter", {
      novelId: novel.id, chapterId: chapter.id, expectedVersion: 0, title: "第一章",
      content: "林动踏上山门石阶，夜雨压低了灯影。",
    });
    await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, novel.id);
    await browser.refresh();
    await (await $("[data-tauri-director-page]")).waitForDisplayed({ timeout: 20_000 });
    await (await $("[data-testid=director-start-button]")).waitForDisplayed({ timeout: 20_000 });
    await (await $("[data-testid=director-start-button]")).click();
    await (await $("[data-testid=director-story-memory]")).waitForDisplayed({ timeout: 30_000 });
    await (await $("[data-testid=director-extract-memory]")).click();
    await browser.waitUntil(async () => (await browser.execute(() => document.body.innerText)).includes("E2E 记忆抽取事实"), { timeout: 30_000, timeoutMsg: "memory candidate did not appear" });
    const confirm = await $("[data-testid=director-confirm-memory-candidate]");
    await confirm.waitForDisplayed({ timeout: 10_000 });
    await confirm.click();
    await browser.waitUntil(async () => (await browser.execute(() => document.body.innerText)).includes("E2E 记忆抽取事实"), { timeout: 20_000 });
    await browser.waitUntil(async () => {
      const facts = await invoke("list_story_facts", { novelId: novel.id, beforeChapterOrder: 2 });
      return facts.some(fact => fact.body.includes("E2E 记忆抽取事实"));
    }, { timeout: 20_000, timeoutMsg: "confirmed memory fact was not visible to Context" });
    const facts = await invoke("list_story_facts", { novelId: novel.id, beforeChapterOrder: 2 });
    expect(facts.some(fact => fact.body.includes("E2E 记忆抽取事实"))).toBe(true);
  });
});
