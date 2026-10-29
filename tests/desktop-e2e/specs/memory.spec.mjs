async function invoke(command, args = {}) {
  return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args);
}
async function openDirector(novelId) {
  await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, novelId);
  await browser.refresh();
  await (await $("[data-tauri-director-page]")).waitForDisplayed({ timeout: 20_000 });
  await (await $("[data-testid=director-start-button]")).waitForDisplayed({ timeout: 20_000 });
}
describe("Director long-term memory", () => {
  it("records author-confirmed timeline, plot and character facts through the Native UI", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 长篇记忆", description: "" });
    await invoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
    await invoke("create_chapter", { novelId: novel.id, title: "第二章", sortOrder: 2 });
    await openDirector(novel.id);
    await (await $("[data-testid=director-chapter-select]")).click();
    const options = await $$("[role=option]");
    for (const option of options) {
      if ((await option.getText()).includes("第二章")) { await option.click(); break; }
    }
    await (await $("[data-testid=director-start-button]")).click();
    await (await $("[data-testid=director-story-memory]")).waitForDisplayed({ timeout: 30_000 });
    const input = await $("[data-testid=director-memory-input]");
    await input.waitForDisplayed({ timeout: 20_000 });
    await input.addValue("WDIO 时间线事实：林动进入祖祠。");
    await (await $("[data-testid=director-save-memory]")).click();
    await browser.waitUntil(async () => (await browser.execute(() => document.body.innerText)).includes("WDIO 时间线事实"), { timeout: 20_000, timeoutMsg: "memory fact did not appear" });
    await (await $("[data-testid=director-plot-title]")).addValue("WDIO 祖祠伏笔");
    await (await $("[data-testid=director-plot-body]")).addValue("封印尚未解释");
    await (await $("[data-testid=director-save-plot]")).click();
    await browser.waitUntil(async () => (await browser.execute(() => document.body.innerText)).includes("WDIO 祖祠伏笔"), { timeout: 20_000, timeoutMsg: "plot thread did not appear" });
    await (await $("[data-testid=director-new-character]")).addValue("WDIO 林动");
    await (await $("[data-testid=director-add-character]")).click();
    await browser.pause(500);
    await (await $("[data-testid=director-state-body]")).addValue("WDIO 状态事实：林动成为掌门。");
    await (await $("[data-testid=director-save-state]")).click();
    await browser.waitUntil(async () => (await browser.execute(() => document.body.innerText)).includes("WDIO 状态事实"), { timeout: 20_000, timeoutMsg: "character state did not appear" });
    const facts = await invoke("list_story_facts", { novelId: novel.id, beforeChapterOrder: 2 });
    expect(facts.some(fact => fact.body.includes("WDIO 时间线事实"))).toBe(true);
    expect(facts.some(fact => fact.factType === "plot_thread" && fact.body.includes("WDIO 祖祠伏笔"))).toBe(true);
    expect(facts.some(fact => fact.factType === "character_state" && fact.body.includes("WDIO 状态事实"))).toBe(true);
  });
});
