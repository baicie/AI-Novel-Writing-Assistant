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

describe("Native structured memory conflicts and relationship", () => {
  it("creates alias and directed relationship, then resolves a same-chapter location conflict", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 记忆冲突", description: "" });
    const first = await invoke("create_character", { request: { id: crypto.randomUUID(), novelId: novel.id, name: "林动", role: "主角" } });
    const second = await invoke("create_character", { request: { id: crypto.randomUUID(), novelId: novel.id, name: "苏晚", role: "同伴" } });
    const chapter = await invoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
    await invoke("save_chapter", { novelId: novel.id, chapterId: chapter.id, expectedVersion: 0, title: chapter.title, content: "林动与苏晚在山门。" });
    const base = { novelId: novel.id, entityId: first.id, sourceChapterId: chapter.id, sourceRevision: 1, effectiveChapterOrder: 1, nowMs: Date.now() };
    await invoke("confirm_structured_fact", { fact: { ...base, id: crypto.randomUUID(), factType: "alias", valueJson: JSON.stringify({ alias: "林公子" }), evidenceQuote: "人称林公子。" } });
    await invoke("confirm_structured_fact", { fact: { ...base, id: crypto.randomUUID(), factType: "relationship", valueJson: JSON.stringify({ relation: "ally_of", targetEntityId: second.id, targetName: "苏晚" }), evidenceQuote: "两人结盟。" } });
    const facts = await invoke("list_structured_facts", { novelId: novel.id, entityId: first.id });
    expect(facts.some((fact) => fact.factType === "alias" && fact.valueJson.includes("林公子"))).toBe(true);
    expect(facts.some((fact) => fact.factType === "relationship" && fact.valueJson.includes(second.id))).toBe(true);
    await invoke("confirm_structured_fact", { fact: { ...base, id: crypto.randomUUID(), factType: "location", valueJson: JSON.stringify({ location: "山门" }), evidenceQuote: "林动在山门。" } });
    const proposed = await invoke("confirm_structured_fact", { fact: { ...base, id: crypto.randomUUID(), factType: "location", valueJson: JSON.stringify({ location: "祖祠" }), evidenceQuote: "林动在祖祠。" } });
    expect(proposed.truthfulness).toBe("proposed");
    await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, novel.id);
    await browser.refresh();
    const confirm = await $("[data-testid=structured-conflict-confirm-new]");
    await confirm.waitForDisplayed({ timeout: 20_000 });
    await confirm.click();
    await browser.waitUntil(async () => {
      const conflicts = await invoke("list_memory_conflicts", { novelId: novel.id });
      return conflicts.every((conflict) => conflict.status !== "open");
    }, { timeout: 20_000 });
    const after = await invoke("list_structured_facts", { novelId: novel.id, entityId: first.id });
    const locations = after.filter((fact) => fact.factType === "location");
    expect(locations.some((fact) => fact.status === "active" && fact.valueJson.includes("祖祠"))).toBe(true);
  });
});
