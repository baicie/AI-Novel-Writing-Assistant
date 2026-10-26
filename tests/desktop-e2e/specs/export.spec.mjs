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

describe("Native novel export selection", () => {
  it("exports full book, range and selected chapter with novel isolation", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 范围导出", description: "说明" });
    const ids = [];
    for (let index = 1; index <= 3; index += 1) {
      const chapter = await invoke("create_chapter", { novelId: novel.id, title: `第${index}章`, sortOrder: index });
      await invoke("save_chapter", { novelId: novel.id, chapterId: chapter.id, expectedVersion: 0, title: `第${index}章`, content: `范围导出正文${index}` });
      ids.push(chapter.id);
    }
    const full = await invoke("export_novel_selection", { novelId: novel.id, selection: "entire_book", format: "markdown" });
    expect(full).toContain("范围导出正文1");
    expect(full).toContain("范围导出正文3");
    const range = await invoke("export_novel_selection", { novelId: novel.id, selection: "range", format: "txt", startOrder: 1, endOrder: 2 });
    expect(range).toContain("范围导出正文1");
    expect(range).toContain("范围导出正文2");
    expect(range).not.toContain("范围导出正文3");
    const selected = await invoke("export_novel_selection", { novelId: novel.id, selection: "chapter", format: "markdown", chapterId: ids[2] });
    expect(selected).toContain("范围导出正文3");
    expect(selected).not.toContain("范围导出正文1");
  });
});

describe("Native volume export", () => {
  it("exports only chapters in the selected volume", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 卷导出", description: "" });
    const volumeA = await invoke("create_volume", { novelId: novel.id, title: "第一卷", sortOrder: 1, summary: "" });
    const volumeB = await invoke("create_volume", { novelId: novel.id, title: "第二卷", sortOrder: 2, summary: "" });
    const first = await invoke("create_chapter", { novelId: novel.id, title: "卷一章", sortOrder: 1, volumeId: volumeA.id });
    const second = await invoke("create_chapter", { novelId: novel.id, title: "卷二章", sortOrder: 2, volumeId: volumeB.id });
    await invoke("save_chapter", { novelId: novel.id, chapterId: first.id, expectedVersion: 0, title: "卷一章", content: "第一卷正文" });
    await invoke("save_chapter", { novelId: novel.id, chapterId: second.id, expectedVersion: 0, title: "卷二章", content: "第二卷正文" });
    const text = await invoke("export_novel_selection", { novelId: novel.id, selection: "volumes", format: "markdown", volumeIds: [volumeA.id] });
    expect(text).toContain("第一卷正文");
    expect(text).not.toContain("第二卷正文");
  });
});
