async function invoke(command, args = {}) {
  return browser.execute((name, payload) => window.__TAURI__.core.invoke(name, payload), command, args);
}

describe("Native chapter history restore", () => {
  it("restores an older official revision as a new revision without deleting history", async () => {
    const novel = await invoke("create_novel", { title: "WDIO 历史恢复", description: "" });
    const chapter = await invoke("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 });
    const first = await invoke("save_chapter", {
      novelId: novel.id, chapterId: chapter.id, expectedVersion: 0,
      title: "第一章", content: "第一版历史正文",
    });
    expect(first.officialVersion).toBe(1);
    const second = await invoke("save_chapter", {
      novelId: novel.id, chapterId: chapter.id, expectedVersion: 1,
      title: "第一章", content: "第二版历史正文",
    });
    expect(second.officialVersion).toBe(2);

    await browser.execute(([n, c]) => { window.location.hash = `#/novels/${n}/chapters/${c}`; }, [novel.id, chapter.id]);
    await (await $("[data-testid=chapter-editor-surface]")).waitForDisplayed({ timeout: 20_000 });
    await (await $("[data-testid=chapter-version-history-button]")).click();
    await (await $("[data-testid=chapter-history-version-1]")).waitForDisplayed({ timeout: 20_000 });
    await (await $("[data-testid=chapter-history-version-1]")).click();
    await (await $("[data-testid=chapter-history-diff-toggle]")).click();
    const diff = await $("[data-testid=chapter-history-diff]");
    await diff.waitForDisplayed({ timeout: 20_000 });
    const diffText = await diff.getText();
    expect(diffText).toContain("第二版历史正文");
    await (await $("[data-testid=chapter-history-restore]")).click();
    await (await $("[data-testid=chapter-history-restore]")).click();
    await browser.waitUntil(async () => {
      const current = await invoke("get_chapter", { novelId: novel.id, chapterId: chapter.id });
      return current.officialVersion === 3 && current.content === "第一版历史正文";
    }, { timeout: 20_000, timeoutMsg: "restore did not create a new official revision" });
    const revisions = await invoke("list_chapter_revisions", { novelId: novel.id, chapterId: chapter.id });
    expect(revisions.length).toBe(3);
    expect(revisions[0].content).toBe("第一版历史正文");
    expect(revisions[0].originDetail).toBe("restored_from_history");
    expect(revisions[0].restoredFromRevisionId).toBe(revisions[2].id);
    expect(revisions.some((revision) => revision.content === "第二版历史正文")).toBe(true);
    expect(revisions.some((revision) => revision.content === "第二版历史正文")).toBe(true);
  });
});
