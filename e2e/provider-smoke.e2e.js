// 原生桌面冒烟：真实启动应用，确认壳层渲染出内容。
describe("桌面壳层", () => {
  it("打开后能拿到窗口标题", async () => {
    const title = await browser.getTitle();
    expect(typeof title).toBe("string");
  });

  it("页面里有实际渲染的节点", async () => {
    await browser.waitUntil(
      async () => {
        const count = await browser.execute(() => document.body.querySelectorAll("*").length);
        return count > 10;
      },
      { timeout: 30000, timeoutMsg: "页面在 30 秒内没有渲染出内容" },
    );
  });
});
