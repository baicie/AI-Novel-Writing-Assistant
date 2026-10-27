async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }
describe("editor probe", () => {
  it("opens chapter route", async () => {
    const novel = await invoke("create_novel", { title: "编辑器探测", description: "" });
    const chapter = await invoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
    await invoke("save_chapter", { novelId: novel.id, chapterId: chapter.id, expectedVersion: 0, title: "第一章", content: "初始正文" });
    await browser.execute((id, chapterId) => { window.location.hash = `#/novels/${id}/chapters/${chapterId}`; }, novel.id, chapter.id);
    await browser.pause(3000);
    console.log("URL", await browser.execute(() => location.href));
    console.log("EDITABLES", await browser.execute(() => document.querySelectorAll("[contenteditable]").length));
    console.log("BODY", (await browser.execute(() => document.body.innerText)).slice(0, 1000));
    console.log("RECT", await browser.execute(() => { const el = document.querySelector("[contenteditable]"); const r = el?.getBoundingClientRect(); return el ? { tag: el.tagName, html: el.outerHTML.slice(0, 300), rect: r && { w: r.width, h: r.height, top: r.top }, display: getComputedStyle(el).display } : null; }));
    console.log("ERRORS", await browser.execute(() => (window.__E2E_ERRORS__ || []).slice(-5)));
  });
});
