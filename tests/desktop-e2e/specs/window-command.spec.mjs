async function invoke(command, args = {}) { return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args); }

describe("Native second editor window", () => {
  it("creates a real Tauri window labelled editor-b", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 双窗口", description: "" });
    const chapter = await invoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
    const label = await invoke("open_editor_window", { novelId: novel.id, chapterId: chapter.id });
    expect(label).toBe("editor-b");
    await browser.pause(800);
    const labels = await invoke("list_app_windows");
    expect(labels).toContain("main");
    expect(labels).toContain("editor-b");
    await invoke("close_editor_window");
    await browser.pause(500);
    const after = await invoke("list_app_windows");
    expect(after).not.toContain("editor-b");
  });
});
