async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }
async function replace(text) {
  await browser.execute((value) => {
    const el = document.querySelector("[contenteditable=true]");
    el.focus();
    document.execCommand("selectAll", false);
    document.execCommand("insertText", false, value);
  }, text);
}

describe("Native two-window draft isolation", () => {
  it("persists independent drafts from two real windows", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 双窗口草稿", description: "" });
    const chapter = await invoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
    await invoke("save_chapter", { novelId: novel.id, chapterId: chapter.id, expectedVersion: 0, title: "第一章", content: "初始正文" });
    await browser.execute((id, chapterId) => { window.location.hash = `#/novels/${id}/chapters/${chapterId}`; }, novel.id, chapter.id);
    await browser.pause(1500);
    await replace("窗口A草稿");
    await browser.pause(1500);
    const aDrafts = await invoke("list_chapter_drafts", { novelId: novel.id, chapterId: chapter.id });
    expect(aDrafts.some((draft) => draft.content.includes("窗口A草稿"))).toBe(true);

    await invoke("open_editor_window", { novelId: novel.id, chapterId: chapter.id });
    await browser.pause(800);
    console.log("WINDOWS", await invoke("list_app_windows"));
    await browser.tauri.switchWindow("editor-b");
    await browser.pause(1500);
    console.log("B_URL", await browser.execute(() => location.href));
    console.log("B_EDITABLES", await browser.execute(() => document.querySelectorAll("[contenteditable=true]").length));
    await replace("窗口B草稿");
    await browser.pause(2000);
    const drafts = await invoke("list_chapter_drafts", { novelId: novel.id, chapterId: chapter.id });
    console.log("DRAFTS", drafts.map((draft) => ({ id: draft.clientId, content: draft.content })));
    expect(drafts.length).toBeGreaterThanOrEqual(2);
    expect(drafts.some((draft) => draft.content.includes("窗口A草稿"))).toBe(true);
    expect(drafts.some((draft) => draft.content.includes("窗口B草稿"))).toBe(true);
    await browser.tauri.switchWindow("main");
    await invoke("close_editor_window");
  });
});

describe("Native two-window official CAS", () => {
  it("rejects the stale official save without deleting the second writer draft", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 双窗口冲突", description: "" });
    const chapter = await invoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
    await invoke("save_chapter", { novelId: novel.id, chapterId: chapter.id, expectedVersion: 0, title: "第一章", content: "初始正文" });
    await browser.execute((id, chapterId) => { window.location.hash = `#/novels/${id}/chapters/${chapterId}`; }, novel.id, chapter.id);
    await browser.pause(1200);
    await replace("窗口A正式稿");
    await browser.pause(1200);
    await invoke("open_editor_window", { novelId: novel.id, chapterId: chapter.id });
    await browser.pause(700);
    await browser.tauri.switchWindow("editor-b");
    await browser.pause(1000);
    await replace("窗口B旧稿");
    await browser.pause(1600);
    await browser.tauri.switchWindow("main");
    const saveA = await $("[data-testid=chapter-save-button]");
    await saveA.waitForExist({ timeout: 10_000 });
    await saveA.click();
    await browser.waitUntil(async () => (await invoke("get_chapter", { novelId: novel.id, chapterId: chapter.id })).officialVersion >= 2, { timeout: 20_000 });
    await browser.tauri.switchWindow("editor-b");
    const saveB = await $("[data-testid=chapter-save-button]");
    await saveB.waitForExist({ timeout: 10_000 });
    await saveB.click();
    await browser.waitUntil(async () => {
      const text = await browser.execute(() => document.body.innerText);
      return text.includes("冲突") || text.includes("版本") || text.includes("已被更新");
    }, { timeout: 20_000, timeoutMsg: "stale official save must show conflict" });
    const official = await invoke("get_chapter", { novelId: novel.id, chapterId: chapter.id });
    expect(official.content).toBe("窗口A正式稿");
    const drafts = await invoke("list_chapter_drafts", { novelId: novel.id, chapterId: chapter.id });
    expect(drafts.some((draft) => draft.clientId === "desktop-editor-b" && draft.content.includes("窗口B旧稿"))).toBe(true);
    await browser.tauri.switchWindow("main");
    await invoke("close_editor_window");
  });
});
