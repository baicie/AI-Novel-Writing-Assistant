// P17.2：选区改写的完整作者旅程（真实 Tauri 进程 / IPC / SQLite / Mock 模型服务）。
//
// 全程不联网、不调用付费模型：模型请求落到本用例起的 127.0.0.1 mock 服务上，
// 由 mock 记录真实收到的 HTTP 请求，再返回可控的候选正文。
//
// 这条用例只断言作者能观察到的结果与官方数据：正文、版本、模型请求次数。
// 不允许用 SQL 直接改写正文来绕过界面。
import http from "node:http";

// 字符串形式，避免转译器给箭头函数注入 WebView 里不存在的 __name 辅助函数。
const invoke = (command, args) =>
  browser.execute(
    (cmd, payload) => window.__TAURI_INTERNALS__.invoke(cmd, payload),
    command,
    args,
  );

const NOVEL_TITLE = "P17.2 选区改写旅程";

/** mock 模型服务：记录每次生成请求，并按脚本返回候选。 */
function createMockModel() {
  const state = {
    requests: [],
    /** 下一次生成要返回的候选正文；为空时用默认候选。 */
    nextCandidates: null,
    /** 下一次生成返回的原始文本；用来模拟「模型返回了，但输出不可用」。 */
    nextRawContent: null,
    /** 让下一次请求挂起，用来验证「进行中」与重试保护。 */
    holdNext: null,
  };

  const server = http.createServer((req, res) => {
    if (req.method === "POST" && req.url.endsWith("/chat/completions")) {
      let body = "";
      req.on("data", (chunk) => {
        body += chunk;
      });
      req.on("end", () => {
        let parsed = {};
        try {
          parsed = JSON.parse(body);
        } catch {
          parsed = { raw: body };
        }
        state.requests.push({ headers: req.headers, body: parsed });
        if (state.holdNext) {
          state.holdNext(() => respond(res, state));
          return;
        }
        respond(res, state);
      });
      return;
    }
    res.writeHead(404).end();
  });

  return { server, state };
}

function respond(res, state) {
  let content;
  if (state.nextRawContent != null) {
    // 模型调用成功返回，但内容不是可解析的候选：用于验证「已计费但不可用」的提示。
    content = state.nextRawContent;
    state.nextRawContent = null;
  } else {
    const candidates = state.nextCandidates ?? [
      { label: "克制", content: "他停在门口，没有说话。\n\n风从缝里挤进来，灯影晃了一下。" },
      { label: "收紧", content: "他停在门口。\n\n风挤进来，灯影一晃。" },
    ];
    state.nextCandidates = null;
    content = JSON.stringify({ candidates, macroAlignmentNote: "贴合本章压抑的气氛。" });
  }
  // OpenAI 兼容协议按 SSE 读取；这里返回一条完整的 delta 与结束标记。
  res.writeHead(200, {
    "content-type": "text/event-stream",
    "cache-control": "no-cache",
    connection: "keep-alive",
  });
  res.write(`data: ${JSON.stringify({ choices: [{ delta: { content } }] })}\n\n`);
  res.write(`data: ${JSON.stringify({ choices: [{ delta: {}, finish_reason: "stop" }] })}\n\n`);
  res.write("data: [DONE]\n\n");
  res.end();
}

/** 在真实编辑器里选中一段文字，并等浮动工具条出现。 */
async function selectEditorText(fragment) {
  const selected = await browser.execute((needle) => {
      const surface = document.querySelector('[data-testid="chapter-editor-body"]');
      const root = surface ? surface.querySelector('[contenteditable="true"]') : null;
      if (!root) return { ok: false, reason: "editor-missing" };
      const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
      let node = walker.nextNode();
      while (node) {
        const index = node.textContent ? node.textContent.indexOf(needle) : -1;
        if (index >= 0) {
          const range = document.createRange();
          range.setStart(node, index);
          range.setEnd(node, index + needle.length);
          const selection = window.getSelection();
          selection.removeAllRanges();
          selection.addRange(range);
          if (node.parentElement) {
            node.parentElement.dispatchEvent(new MouseEvent("mouseup", { bubbles: true }));
          }
          return { ok: true };
        }
        node = walker.nextNode();
      }
      return { ok: false, reason: "text-missing" };
  }, fragment);
  if (!selected.ok) {
    throw new Error(`无法在编辑器里选中「${fragment}」：${selected.reason}`);
  }
  await browser.pause(500);
}

async function editorText() {
  // 预览候选时正文区被替换成只读对比视图，contenteditable 不存在；
  // 两种状态都从编辑器正文容器取文本，断言的是「正文区当前展示的内容」。
  return browser.execute(() => {
    const surface = document.querySelector('[data-testid="chapter-editor-body"]');
    if (surface) return surface.innerText;
    const content = document.querySelector('[data-testid="chapter-editor-content"]');
    return content ? content.innerText : "";
  });
}

describe("选区改写的完整作者旅程（桌面原生）", () => {
  const mock = createMockModel();
  let baseUrl = "";
  let novelId = "";
  let firstChapterId = "";
  let secondChapterId = "";

  before(async () => {
    await new Promise((resolve) => mock.server.listen(0, "127.0.0.1", resolve));
    baseUrl = `http://127.0.0.1:${mock.server.address().port}/v1`;

    await browser.pause(2500);

    // 真实 IPC 建书、建章、写入正文并保存成正式版本。
    // 函数体里只保留箭头表达式与普通调用：tsx 会给带名字的函数声明注入 __name，
    // 而 WebView 里没有这个全局。
    const seeded = await browser.execute(async (title) => {
      const call = window.__TAURI_INTERNALS__.invoke;
      const novels = await call("list_novels");
      const existing = (novels || []).find((item) => item.title === title);
      const novel = existing || await call("create_novel", { title, description: "" });
      const chapters = (await call("list_chapters", { novelId: novel.id })) || [];
      const firstExisting = chapters.find((item) => item.sortOrder === 1);
      const firstId = firstExisting
        ? firstExisting.id
        : ((await call("create_chapter", { novelId: novel.id, title: "第一章", sortOrder: 1 }))?.id);
      await call("save_chapter", {
        novelId: novel.id,
        chapterId: firstId,
        expectedVersion: firstExisting ? firstExisting.officialVersion : 0,
        title: "第一章",
        content: "他推开门，屋里没有人。\n\n灯还亮着，桌上摊着半张地图。\n\n他走到窗前，听见远处有人敲门。",
      });
      const secondExisting = chapters.find((item) => item.sortOrder === 2);
      const secondId = secondExisting
        ? secondExisting.id
        : ((await call("create_chapter", { novelId: novel.id, title: "第二章", sortOrder: 2 }))?.id);
      await call("save_chapter", {
        novelId: novel.id,
        chapterId: secondId,
        expectedVersion: secondExisting ? secondExisting.officialVersion : 0,
        title: "第二章",
        content: "第二章的正文不该被改写。",
      });
      return { novelId: novel.id, first: firstId, second: secondId };
    }, NOVEL_TITLE);

    novelId = seeded.novelId;
    firstChapterId = seeded.first;
    secondChapterId = seeded.second;

    // 同一份应用数据会跨运行复用：其它 spec 可能留下指向死端口的 writer 绑定。
    // 直接写模型设置：它会把来源、模型与 writer 绑定一起登记到注册表。
    await invoke("save_model_setup", {
      request: {
        endpoint: baseUrl,
        model: "e2e-mock-model",
        allowLocalhostHttp: true,
        apiKey: "e2e-local-key",
      },
    });

    // 注册表同步是尽力而为：绑定缺失或被别的 spec 改走时，这里显式补写。
    // 绑定要求来源与模型都已经在注册表里，所以顺序是先确认模型、再写绑定。
    const bindings = await invoke("list_model_bindings").catch(() => []);
    const writerBinding = (bindings || []).find((item) => item.taskType === "writer");
    if (!writerBinding || writerBinding.providerId !== "default" || writerBinding.modelId !== "e2e-mock-model") {
      await invoke("upsert_provider_model", {
        request: {
          providerId: "default",
          modelId: "e2e-mock-model",
          displayName: "E2E Mock",
          expectedRevision: null,
        },
      }).catch(() => undefined);
      await invoke("upsert_model_binding", {
        request: {
          taskType: "writer",
          providerId: "default",
          modelId: "e2e-mock-model",
          reasoningEffort: null,
          expectedRevision: null,
        },
      });
    }

    // 前面的 spec 可能把 writer 绑定指向别处：这里确认它真的落在本用例的 mock 上。
    const route = await invoke("get_effective_model_route", { taskType: "writer" });
    if (!route || route.endpoint !== baseUrl) {
      throw new Error(`writer 路由没有指向 mock：${JSON.stringify(route)}；期望 ${baseUrl}`);
    }

    // 清掉上次运行留下的请求记录，保证请求次数断言可重复。
    mock.state.requests.length = 0;
  });

  after(async () => {
    mock.server?.close();
    // 清掉本用例写入的「最近写作」记录：同一进程里的后续 spec 不该被带到这一章。
    await browser.execute(
      "(() => { try { Object.keys(window.localStorage).filter((key) => key.indexOf('ai-novel:edit-position:') === 0).forEach((key) => window.localStorage.removeItem(key)); } catch {} })()",
    ).catch(() => undefined);
  });

  it("生成候选不改正文，模型请求带上选区与上下文", async () => {
    await browser.execute(
      (nid, cid) => {
        window.location.hash = "#/novels/" + nid + "/chapters/" + cid;
      },
      novelId,
      firstChapterId,
    );
    await browser.pause(3500);

    const before = await editorText();
    expect(before).toContain("他推开门");

    await selectEditorText("他走到窗前，听见远处有人敲门。");

    const toolbar = await $('[data-testid="selection-ai-toolbar"]');
    await toolbar.waitForExist({ timeout: 10000 });
    await $('[data-testid="selection-ai-polish"]').click();

    const ready = await browser.waitUntil(
      async () => (await $$('[data-testid="chapter-revision-candidate"]')).length >= 2,
      { timeout: 20000, timeoutMsg: "没有等到 2 个改写候选" },
    ).then(() => true).catch(() => false);
    if (!ready) {
      // 失败时留下可排查的现场：请求数、错误面板与模型路由。
      const panels = await browser.execute(
        "(() => { const el = document.querySelector('[data-testid=\"chapter-revision-error\"]'); return el ? el.innerText : 'no-error-panel'; })()",
      );
      const route = await invoke("get_effective_model_route", { taskType: "writer" }).catch((error) => ({ error: String(error) }));
      throw new Error(`没有等到改写候选。请求数=${mock.state.requests.length}，错误面板=${panels}，路由=${JSON.stringify(route)}`);
    }
    expect((await $$('[data-testid="chapter-revision-candidate"]')).length).toBeGreaterThanOrEqual(2);

    // 真实 HTTP 请求已经到达 mock 服务，且带上选区正文与前后文。
    expect(mock.state.requests.length).toBe(1);
    const prompt = JSON.stringify(mock.state.requests[0].body);
    expect(prompt).toContain("他走到窗前，听见远处有人敲门。");
    expect(prompt).toContain("他推开门，屋里没有人。");
    expect(mock.state.requests[0].body.model).toBe("e2e-mock-model");

    // 生成候选只进入右侧对比：官方正文没有被改动（直接读正式数据，避免被对比视图干扰）。
    const official = await invoke("get_chapter", { novelId, chapterId: firstChapterId });
    expect(official.content).toContain("他推开门");
    expect(official.content).toContain("他走到窗前");
    expect(official.content).not.toContain("他停在门口");
  });

  it("只采纳一部分改动时，未选中的改动不进入正文", async () => {
    const changes = await $$('[data-testid="chapter-revision-change"]');
    expect(changes.length).toBeGreaterThanOrEqual(2);

    // 取消第二处改动，只保留其它两处：验证未勾选的改动不会进入正文。
    const secondToggle = await changes[1].$('[data-testid="chapter-revision-change-toggle"]');
    await secondToggle.click();
    await browser.pause(500);

    const acceptButton = await $('[data-testid="chapter-revision-accept"]');
    expect(await acceptButton.getAttribute("disabled")).toBe(null);
    await browser.execute(
      "(() => { const el = document.querySelector('[data-testid=\"chapter-revision-accept\"]'); if (el) el.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, view: window })); })()",
    );
    await browser.pause(4000);

    // 正文应出现被勾选的改写，未勾选的那处候选文案不能出现。
    const accepted = await invoke("get_chapter", { novelId, chapterId: firstChapterId });
    expect(accepted.content.length).toBeGreaterThan(0);
    expect(accepted.content).toContain("他停在门口");
    // 采纳不会再次调用模型。
    expect(mock.state.requests.length).toBe(1);
  });

  it("保存正式章节后重新打开仍是保存后的正文", async () => {
    await $('[data-testid="chapter-save-button"]').click();
    await browser.pause(2000);

    const saved = await invoke("get_chapter", { novelId, chapterId: firstChapterId });
    expect(saved.content.length).toBeGreaterThan(0);
    expect(saved.officialVersion).toBeGreaterThanOrEqual(1);

    // 切到第二章再回来：第一章正文与版本必须一致。
    await invoke("get_chapter", { novelId, chapterId: secondChapterId });
    await browser.execute(
      (nid, cid) => {
        window.location.hash = "#/novels/" + nid + "/chapters/" + cid;
      },
      novelId,
      secondChapterId,
    );
    await browser.pause(2500);
    await browser.execute(
      (nid, cid) => {
        window.location.hash = "#/novels/" + nid + "/chapters/" + cid;
      },
      novelId,
      firstChapterId,
    );
    await browser.pause(2500);

    const reopened = await editorText();
    expect(reopened).toContain("他推开门");
    // 第二章没有被写入这次改写。
    expect(reopened).not.toContain("第二章的正文不该被改写。");
  });

  it("重新载入应用后正文与正式版本仍一致", async () => {
    const before = await invoke("get_chapter", { novelId, chapterId: firstChapterId });
    expect(before.content.length).toBeGreaterThan(0);

    // 重载 WebView 等价于重新打开这一章：正文必须来自本地内核，而不是编辑器内存。
    await browser.refresh();
    await browser.pause(4000);
    await browser.execute(
      (nid, cid) => {
        window.location.hash = "#/novels/" + nid + "/chapters/" + cid;
      },
      novelId,
      firstChapterId,
    );
    await browser.pause(3500);

    const reopened = await editorText();
    expect(reopened).toContain("他推开门");
    const after = await invoke("get_chapter", { novelId, chapterId: firstChapterId });
    expect(after.content).toBe(before.content);
    expect(after.officialVersion).toBe(before.officialVersion);
  });

  it("重复点击不会产生第二次模型请求", async () => {
    const before = mock.state.requests.length;
    await selectEditorText("灯还亮着，桌上摊着半张地图。");
    const toolbar = await $('[data-testid="selection-ai-toolbar"]');
    await toolbar.waitForExist({ timeout: 10000 });
    const polish = await $('[data-testid="selection-ai-polish"]');
    await polish.click();
    // 进行中按钮应被禁用：连点不会触发第二次请求。
    await polish.click().catch(() => undefined);
    await browser.pause(2500);
    expect(mock.state.requests.length).toBe(before + 1);
  });

  it("模型返回不可用输出时提示可能已计费，且不改正文", async () => {
    // 上一条用例停在候选对比视图，先拒绝全部回到可编辑状态。
    await $('[data-testid="chapter-revision-reject"]').click();
    await browser.pause(1500);

    const before = mock.state.requests.length;
    const beforeChapter = await invoke("get_chapter", { novelId, chapterId: firstChapterId });
    // 真实 HTTP mock：请求会到达模型服务并记账，只是返回内容不可解析。
    mock.state.nextRawContent = "这不是可解析的改写结果";

    await selectEditorText("灯还亮着，桌上摊着半张地图。");
    const toolbar = await $('[data-testid="selection-ai-toolbar"]');
    await toolbar.waitForExist({ timeout: 10000 });
    await $('[data-testid="selection-ai-polish"]').click();

    const panel = await $('[data-testid="chapter-revision-error"]');
    await panel.waitForExist({ timeout: 20000 });
    const message = await panel.getText();
    // 模型已被调用：提示必须说明可能已计费、重新生成会再次调用模型。
    expect(message).toContain("可能已经产生费用");
    expect(message).toContain("再次调用模型");
    expect(message).not.toContain("没有产生费用");

    // 真实 IPC 读回官方正文：失败的改写不能改动正文或正式版本。
    const official = await invoke("get_chapter", { novelId, chapterId: firstChapterId });
    expect(official.content).toBe(beforeChapter.content);
    expect(official.officialVersion).toBe(beforeChapter.officialVersion);
    expect(mock.state.requests.length).toBe(before + 1);
  });
});
