// P17.2：进程重启后的只读校验。
//
// 这个 spec 不写入任何正文，只验证上一个进程保存的结果在全新进程里仍然一致。
// 由 scripts/run-tauri-p172-rewrite-restart-e2e.sh 使用同一个数据目录启动。
const invoke = (command, args) =>
  browser.execute(
    (cmd, payload) => window.__TAURI_INTERNALS__.invoke(cmd, payload),
    command,
    args,
  );

describe("选区改写结果在进程重启后仍然一致", () => {
  it("新进程读到的正文、正式版本与保存时一致", async () => {
    await browser.pause(2500);

    const found = await browser.execute(async (title) => {
      const call = window.__TAURI_INTERNALS__.invoke;
      const novels = (await call("list_novels")) || [];
      let novel = null;
      for (let index = 0; index < novels.length; index += 1) {
        if (novels[index].title === title) novel = novels[index];
      }
      if (!novel) return null;
      const chapters = (await call("list_chapters", { novelId: novel.id })) || [];
      let first = null;
      for (let index = 0; index < chapters.length; index += 1) {
        if (chapters[index].sortOrder === 1) first = chapters[index];
      }
      return { novelId: novel.id, chapterId: first ? first.id : null };
    }, "P17.2 选区改写旅程");
    expect(found).not.toBeNull();
    expect(found.chapterId).toBeTruthy();

    const chapter = await invoke("get_chapter", { novelId: found.novelId, chapterId: found.chapterId });
    // 上一个进程采纳并保存过改写：正文必须仍在，且已计入正式版本。
    expect(chapter.content).toContain("他推开门");
    expect(chapter.content).toContain("他停在门口");
    expect(chapter.officialVersion).toBeGreaterThanOrEqual(1);

    // 打开这一章，编辑器展示的正文必须与内核一致。
    await browser.execute(
      (nid, cid) => {
        window.location.hash = "#/novels/" + nid + "/chapters/" + cid;
      },
      found.novelId,
      found.chapterId,
    );
    await browser.pause(3500);
    const shown = await browser.execute(
      "(() => { const surface = document.querySelector('[data-testid=\"chapter-editor-body\"]'); return surface ? surface.innerText : ''; })()",
    );
    expect(shown).toContain("他推开门");
  });
});
