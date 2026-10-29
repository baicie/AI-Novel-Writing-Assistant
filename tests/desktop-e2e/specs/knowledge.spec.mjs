async function invoke(command, args = {}) {
  return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args);
}
async function openDirector(novelId) {
  await browser.execute((id) => { window.location.hash = `#/lab/director/${id}`; }, novelId);
  await browser.refresh();
  await (await $("[data-tauri-director-page]")).waitForDisplayed({ timeout: 20_000 });
  await (await $("[data-testid=director-start-button]")).waitForDisplayed({ timeout: 20_000 });
}
describe("Native knowledge CRUD", () => {
  it("creates, edits and deletes a knowledge entry while keeping its source revision", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 知识库", description: "" });
    await invoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
    await openDirector(novel.id);
    await (await $("[data-testid=director-start-button]")).click();
    await (await $("[data-testid=director-knowledge-health]")).waitForDisplayed({ timeout: 30_000 });
    await (await $("[data-testid=director-knowledge-title]")).addValue("WDIO 祖祠知识");
    await (await $("[data-testid=director-knowledge-body]")).addValue("祖祠封印着魂兽，林动在第一章发现。");
    await (await $("[data-testid=director-save-knowledge]")).click();
    await browser.waitUntil(async () => (await browser.execute(() => document.body.innerText)).includes("WDIO 祖祠知识"), { timeout: 20_000, timeoutMsg: "knowledge entry did not appear" });
    await (await $("[data-testid=director-edit-knowledge]")).click();
    await browser.waitUntil(async () => (await (await $("[data-testid=director-knowledge-body]")).getValue()).includes("祖祠封印"), { timeout: 10_000 });
    await (await $("[data-testid=director-knowledge-body]")).addValue("（作者修订）");
    await (await $("[data-testid=director-save-knowledge]")).click();
    await browser.execute(() => { window.confirm = () => true; });
    await (await $("[data-testid=director-delete-knowledge]")).click();
    await browser.waitUntil(async () => !(await browser.execute(() => document.body.innerText)).includes("WDIO 祖祠知识"), { timeout: 20_000, timeoutMsg: "knowledge entry was not deleted" });
  });
});
