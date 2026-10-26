async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Native structured memory", () => {
  it("confirms a character location with source evidence and shows it", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 人物状态", description: "" });
    const character = await invoke("create_character", { request: { id: crypto.randomUUID(), novelId: novel.id, name: "林动", role: "主角" } });
    const chapter = await invoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
    await invoke("save_chapter", { novelId: novel.id, chapterId: chapter.id, expectedVersion: 0, title: chapter.title, content: "林动踏上山门。" });
    await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, novel.id);
    const panel = await $("[data-testid=structured-memory-panel]");
    await panel.waitForDisplayed({ timeout: 20_000 });
    await browser.waitUntil(async () => (await $$("[data-testid=memory-chapter] option")).length > 0, { timeout: 20_000 });
    await (await $("[data-testid=memory-character]")).selectByAttribute("value", character.id);
    await (await $("[data-testid=memory-type]")).selectByAttribute("value", "location");
    await (await $("[data-testid=memory-value]")).setValue("山门");
    await (await $("[data-testid=memory-chapter]")).selectByAttribute("value", chapter.id);
    await (await $("[data-testid=memory-evidence]")).setValue("林动踏上山门。");
    await (await $("[data-testid=memory-confirm]")).click();
    await browser.waitUntil(async () => (await browser.execute(() => document.body.innerText)).includes("山门"), { timeout: 20_000 });
    const facts = await invoke("list_structured_facts", { novelId: novel.id, entityId: character.id });
    expect(facts.length).toBe(1);
    expect(facts[0].factType).toBe("location");
    expect(facts[0].valueJson).toContain("山门");
    expect(facts[0].truthfulness).toBe("confirmed");
  });
});
