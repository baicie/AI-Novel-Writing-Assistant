async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }
describe("tauri service api probe", () => {
  it("reports window APIs", async () => {
    const novel = await invoke("create_novel", { title: "API 探测", description: "" });
    const chapter = await invoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
    await invoke("open_editor_window", { novelId: novel.id, chapterId: chapter.id });
    const tauri = browser.tauri;
    console.log("TAURI_TYPE", typeof tauri);
    console.log("TAURI_KEYS", tauri ? Object.keys(tauri) : null);
    if (tauri?.listWindows) {
      try { console.log("TAURI_WINDOWS", await tauri.listWindows()); } catch (error) { console.log("LIST_FAILED", String(error)); }
    }
    try { console.log("HANDLES", await browser.getWindowHandles()); } catch (error) { console.log("HANDLES_FAILED", String(error)); }
    await invoke("close_editor_window");
  });
});
