// 回归：桌面端任意入口都不应出现「暂未接入桌面本地内核」。
describe("桌面入口：没有未接入的接口", () => {
  const missing = async () => browser.execute(() => window.__missingRoutes ?? []);
  const reset = async () => browser.execute(() => { window.__missingRoutes = []; });
  const clickByText = async (pattern) =>
    browser.execute((src) => {
      const re = new RegExp(src);
      const el = Array.from(document.querySelectorAll("button, a")).find((node) =>
        re.test((node.textContent || "").trim()),
      );
      if (!el) return null;
      el.click();
      return (el.textContent || "").trim();
    }, pattern);

  it("首屏不请求未接入的接口", async () => {
    await browser.pause(2500);
    expect(await missing()).toEqual([]);
  });

  for (const step of ["模型设置", "创作向导", "小说列表", "系统设置", "运行记录", "知识库", "题材基底库"]) {
    it("「" + step + "」不请求未接入的接口", async () => {
      await reset();
      await clickByText(step);
      await browser.pause(1500);
      expect(await missing()).toEqual([]);
    });
  }
});
