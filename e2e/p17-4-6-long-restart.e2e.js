const invoke = (command, args) =>
  browser.execute((cmd, payload) => window.__TAURI_INTERNALS__.invoke(cmd, payload), command, args);

describe("P17.4.6 40 章长篇重启继续", () => {
  it("新进程读取同一数据目录后仍能看到 40 章与伏笔状态", async () => {
    await browser.pause(2500);
    const novels = await invoke("list_novels", { page: 1, limit: 50 });
    const items = Array.isArray(novels) ? novels : novels.items || [];
    const novel = items.find((item) => String(item.title || "").startsWith("P17.4.6 长篇验收"));
    expect(novel).toBeTruthy();
    const chapters = await invoke("list_chapters", { novelId: novel.id });
    expect(chapters.length).toBe(40);
    const ws = await invoke("get_plot_workspace", { novelId: novel.id, chapterId: chapters[39].id });
    expect(ws.threads.find((item) => item.id === "p1746-plot-1").status).toBe("resolved");
    expect(ws.threads.find((item) => item.id === "p1746-plot-8").status).toBe("developing");
    await browser.execute(
      (nid, cid) => { window.location.hash = `#/novels/${nid}/chapters/${cid}`; },
      novel.id,
      chapters[39].id,
    );
    await browser.pause(3000);
    await browser.saveScreenshot("/tmp/p1746-long-restart.png").catch(() => {});
  });
});
