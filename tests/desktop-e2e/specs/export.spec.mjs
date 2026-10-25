async function invoke(command, args = {}) {
  return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args);
}

describe("Native chapter export", () => {
  it("exports only selected official chapters and keeps novels isolated", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 导出", description: "导出说明" });
    const first = await invoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
    const second = await invoke("create_chapter", { novelId: novel.id, title: "第二章", sortOrder: 2 });
    await invoke("save_chapter", { novelId: novel.id, chapterId: first.id, expectedVersion: 0, title: "第一章", content: "第一章中文正文" });
    await invoke("save_chapter", { novelId: novel.id, chapterId: second.id, expectedVersion: 0, title: "第二章", content: "第二章中文正文" });
    const other = await invoke("create_novel", { title: "另一本书", description: "" });
    const otherChapter = await invoke("create_chapter", { novelId: other.id, title: "外书章节", sortOrder: 1 });
    await invoke("save_chapter", { novelId: other.id, chapterId: otherChapter.id, expectedVersion: 0, title: "外书章节", content: "不应导出的内容" });

    const markdown = await invoke("export_chapters", { novelId: novel.id, chapterIds: [first.id], format: "markdown" });
    expect(markdown).toContain("第一章中文正文");
    expect(markdown).not.toContain("第二章中文正文");
    expect(markdown).not.toContain("不应导出的内容");
    const txt = await invoke("export_chapters", { novelId: novel.id, chapterIds: [second.id], format: "txt" });
    expect(txt).toContain("第二章中文正文");
    expect(txt).not.toContain("第一章中文正文");
  });
});
