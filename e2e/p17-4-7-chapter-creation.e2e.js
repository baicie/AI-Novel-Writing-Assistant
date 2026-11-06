// P17.4.7：章节创建的默认标题、顺序与失败恢复（真实桌面 IPC / SQLite / UI）。
//
// 小说与底层数据通过 IPC 初始化；章节创建、按钮点击和错误提示都走真实 GUI。
// 失败恢复用一个独立的 SQLite 排他锁制造真实的写入失败，不修改产品代码、不跳过断言。
import fs from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";

const invoke = (command, args) =>
  browser.execute((cmd, payload) => window.__TAURI_INTERNALS__.invoke(cmd, payload), command, args);

const CREATE_BUTTON = '[data-testid="chapter-create-button"]';
const OPERATION_MESSAGE = '[data-testid="chapter-operation-message"]';

async function createNovel(title) {
  return invoke("create_novel_with_setup", {
    request: { title, description: "", creationExperience: "professional", estimatedChapterCount: 3 },
  });
}

async function openChapterWorkbench(novelId) {
  // 辅助初始化：直接打开章节执行页，避免本用例重复验证创建导航。
  await browser.execute((id) => { window.location.hash = `#/novels/${id}/edit?stage=chapter`; }, novelId);
  const button = await $(CREATE_BUTTON);
  await button.waitForExist({ timeout: 20000 });
  await button.waitForEnabled({ timeout: 20000 });
}

async function listChapters(novelId) {
  return invoke("list_chapters", { novelId });
}

async function waitForChapterCount(novelId, count) {
  await browser.waitUntil(
    async () => (await listChapters(novelId)).length === count,
    { timeout: 20000, timeoutMsg: `章节数量没有变成 ${count}` },
  );
}

async function waitForCreateEnabled() {
  await browser.waitUntil(
    async () => {
      const button = await $(CREATE_BUTTON);
      return Boolean(button && await button.isEnabled());
    },
    { timeout: 20000, timeoutMsg: "新建章节按钮没有恢复可用" },
  );
}

async function clickCreate() {
  const button = await $(CREATE_BUTTON);
  await button.waitForClickable({ timeout: 20000 });
  await button.click();
}

async function waitForSelectedChapter(chapterId) {
  await browser.waitUntil(
    async () => String(await browser.execute("window.location.hash")).includes(`chapterId=${chapterId}`),
    { timeout: 20000, timeoutMsg: "创建成功后没有选中新章节" },
  );
}

async function waitForWorkspaceText(text) {
  await browser.waitUntil(
    async () => String(await browser.execute("document.body.innerText")).includes(text),
    { timeout: 20000, timeoutMsg: `工作区没有刷新出 ${text}` },
  );
}

describe("P17.4.7 章节创建体验", () => {
  it("默认使用中文章节标题，并按现有最大顺序继续编号", async () => {
    const novel = await createNovel("P17.4.7 章节标题验收");
    await openChapterWorkbench(novel.id);

    await clickCreate();
    await waitForChapterCount(novel.id, 1);
    await waitForCreateEnabled();
    let chapters = await listChapters(novel.id);
    expect(chapters.map((item) => [item.sortOrder, item.title])).toEqual([[1, "第1章"]]);
    await waitForSelectedChapter(chapters[0].id);

    await clickCreate();
    await waitForChapterCount(novel.id, 2);
    await waitForCreateEnabled();
    chapters = await listChapters(novel.id);
    expect(chapters.map((item) => [item.sortOrder, item.title])).toEqual([
      [1, "第1章"],
      [2, "第2章"],
    ]);
    await waitForSelectedChapter(chapters[1].id);
    await waitForWorkspaceText("第2章");

    // 作者自己改过的标题不能被新建章节覆盖。
    const first = chapters.find((item) => item.sortOrder === 1);
    await invoke("save_chapter", {
      novelId: novel.id,
      chapterId: first.id,
      expectedVersion: first.officialVersion,
      title: "作者自己取的名字",
      content: "第一章正文。",
    });
    await clickCreate();
    await waitForChapterCount(novel.id, 3);
    await waitForCreateEnabled();
    chapters = await listChapters(novel.id);
    expect(chapters.find((item) => item.sortOrder === 1).title).toBe("作者自己取的名字");
    expect(chapters.find((item) => item.sortOrder === 3).title).toBe("第3章");

    // 删掉中间章节后，列表长度和最大顺序脱节；新章节必须继续最大顺序。
    const second = chapters.find((item) => item.sortOrder === 2);
    await invoke("delete_chapter", {
      novelId: novel.id,
      chapterId: second.id,
      expectedVersion: second.officialVersion,
    });
    await waitForChapterCount(novel.id, 2);
    await clickCreate();
    await waitForChapterCount(novel.id, 3);
    await waitForCreateEnabled();
    chapters = await listChapters(novel.id);
    const orders = chapters.map((item) => item.sortOrder);
    expect(new Set(orders).size).toBe(orders.length);
    expect(chapters.find((item) => item.sortOrder === 4)?.title).toBe("第4章");
  });

  it("连续点击只创建一个章节，按钮恢复后可以继续创建", async () => {
    const novel = await createNovel("P17.4.7 连续创建验收");
    await openChapterWorkbench(novel.id);

    // 同一个任务里连续点击两次：isPending 还没来得及渲染，必须由同步闸门拦住。
    const clickResult = await browser.execute((selector) => {
      const button = document.querySelector(selector);
      if (!button) return "missing";
      button.click();
      button.click();
      return "clicked";
    }, CREATE_BUTTON);
    expect(clickResult).toBe("clicked");

    await browser.waitUntil(
      async () => (await listChapters(novel.id)).length >= 1,
      { timeout: 20000, timeoutMsg: "连续点击后没有创建出章节" },
    );
    await waitForCreateEnabled();
    // 给可能仍在排队的第二次请求一个稳定的观察窗口，再确认没有多建。
    await browser.pause(1000);
    const first = await listChapters(novel.id);
    expect(first.length).toBe(1);
    await waitForSelectedChapter(first[0].id);

    await clickCreate();
    await waitForChapterCount(novel.id, 2);
    const chapters = await listChapters(novel.id);
    expect(chapters.map((item) => [item.sortOrder, item.title])).toEqual([
      [1, "第1章"],
      [2, "第2章"],
    ]);
    await waitForSelectedChapter(chapters[1].id);
  });

  it("数据库写入失败时显示原因，释放后重试可以恢复", async () => {
    const dataDir = process.env.BIZ_NOVEL_DATA_DIR;
    expect(dataDir).toBeTruthy();
    const dbPath = path.join(dataDir, "novel.sqlite");
    expect(fs.existsSync(dbPath)).toBe(true);
    const novel = await createNovel("P17.4.7 创建失败恢复验收");
    await openChapterWorkbench(novel.id);

    const signalPath = path.join(dataDir, "p1747-lock-ready");
    const lockScript = path.join(dataDir, "p1747-lock.py");
    fs.writeFileSync(lockScript, `
import pathlib
import sqlite3
import sys
import time

db, signal = sys.argv[1], sys.argv[2]
con = sqlite3.connect(db, timeout=10, isolation_level=None)
deadline = time.time() + 10
while True:
    try:
        con.execute("BEGIN EXCLUSIVE")
        break
    except sqlite3.OperationalError:
        con.rollback()
        if time.time() > deadline:
            raise
        time.sleep(0.2)
pathlib.Path(signal).write_text("locked")
time.sleep(8)
con.execute("ROLLBACK")
con.close()
pathlib.Path(signal).unlink(missing_ok=True)
`);

    const holder = spawn("python3", [lockScript, dbPath, signalPath], { stdio: "ignore" });
    const exited = new Promise((resolve, reject) => {
      holder.once("error", reject);
      holder.once("exit", resolve);
    });
    try {
      await browser.waitUntil(() => fs.existsSync(signalPath), {
        timeout: 15000,
        timeoutMsg: "没有拿到 SQLite 排他锁",
      });
      await clickCreate();
      const message = await $(OPERATION_MESSAGE);
      await message.waitForExist({ timeout: 20000 });
      const text = await message.getText();
      console.log("create failure message:", text);
      expect(text.length).toBeGreaterThan(0);
      expect(text).toContain("稍后重试");
      expect((await listChapters(novel.id)).length).toBe(0);
      await waitForCreateEnabled();

      await Promise.race([
        exited,
        browser.waitUntil(() => !fs.existsSync(signalPath), {
          timeout: 25000,
          timeoutMsg: "排他锁没有释放",
        }),
      ]);
      await clickCreate();
      await waitForChapterCount(novel.id, 1);
      const chapters = await listChapters(novel.id);
      expect(chapters[0].title).toBe("第1章");
      expect(await $(OPERATION_MESSAGE).isExisting()).toBe(false);
    } finally {
      holder.kill("SIGKILL");
      await exited.catch(() => {});
      fs.rmSync(signalPath, { force: true });
      fs.rmSync(lockScript, { force: true });
    }
  });
});
