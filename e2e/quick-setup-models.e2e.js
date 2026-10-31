// 回归：首次上手「获取模型列表」走本地内核，不得出现未接入提示。
import http from "node:http";

describe("首次上手：获取模型列表", () => {
  let server;
  let baseUrl;

  before(async () => {
    server = http.createServer((req, res) => {
      if (req.method === "GET" && req.url.endsWith("/models")) {
        res.writeHead(200, { "content-type": "application/json" });
        res.end(JSON.stringify({ data: [{ id: "quick-model-a" }, { id: "quick-model-b" }] }));
        return;
      }
      res.writeHead(404).end();
    });
    await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
    baseUrl = `http://127.0.0.1:${server.address().port}/v1`;
  });

  after(() => server?.close());

  const clickExact = async (text) =>
    browser.execute((label) => {
      const el = Array.from(document.querySelectorAll("button, a")).find(
        (node) => (node.textContent || "").trim() === label,
      );
      if (!el) return false;
      el.click();
      return true;
    }, text);

  const clickContains = async (text) =>
    browser.execute((label) => {
      const el = Array.from(document.querySelectorAll("button")).find((node) =>
        (node.textContent || "").trim().includes(label),
      );
      if (!el) return false;
      el.click();
      return true;
    }, text);

  it("第三方厂商获取模型列表不报未接入", async () => {
    await browser.pause(2500);
    await browser.execute(() => { window.__missingRoutes = []; });

    expect(await clickExact("模型设置")).toBe(true);
    await browser.pause(1200);
    expect(await clickContains("添加第三方厂商")).toBe(true);
    await browser.pause(1200);

    await browser.execute((url) => {
      const list = Array.from(document.querySelectorAll("input"));
      const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value").set;
      const urlInput = list.find((el) => (el.getAttribute("placeholder") || "").includes("http"));
      const keyInput = list.find((el) => (el.getAttribute("placeholder") || "").toLowerCase().includes("sk-"));
      if (urlInput) {
        setter.call(urlInput, url);
        urlInput.dispatchEvent(new Event("input", { bubbles: true }));
      }
      if (keyInput) {
        setter.call(keyInput, "sk-local");
        keyInput.dispatchEvent(new Event("input", { bubbles: true }));
      }
    }, baseUrl);

    expect(await clickContains("获取模型列表")).toBe(true);
    await browser.pause(2500);

    const state = await browser.execute(() => ({
      missing: window.__missingRoutes ?? [],
      text: document.body.innerText,
    }));
    expect(state.missing).toEqual([]);
    expect(state.text).not.toContain("暂未接入");
    expect(state.text).toContain("找到 2 个可用模型");
  });
});
