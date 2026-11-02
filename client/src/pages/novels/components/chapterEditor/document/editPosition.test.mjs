import assert from "node:assert/strict";
import test from "node:test";
import {
  buildEditPositionRecord,
  clearEditPosition,
  readEditPosition,
  readRecentEditTarget,
  resolveEditPositionRestore,
  startupEditRoute,
  writeEditPosition,
  writeRecentEditTarget,
} from "./editPosition.ts";

/** 内存版 Storage：位置恢复是纯偏好逻辑，用最小实现即可确定性地验证。 */
function memoryStorage() {
  const data = new Map();
  return {
    getItem: (key) => (data.has(key) ? data.get(key) : null),
    setItem: (key, value) => { data.set(key, String(value)); },
    removeItem: (key) => { data.delete(key); },
    size: () => data.size,
  };
}

const base = {
  novelId: "n1",
  chapterId: "c1",
  cursorOffset: 12,
  scrollTop: 240,
  officialVersion: 3,
  contentLength: 100,
  now: 1_000,
};

test("编辑位置按小说隔离，且不保存正文", () => {
  const storage = memoryStorage();
  writeEditPosition(buildEditPositionRecord(base), storage);
  writeEditPosition(buildEditPositionRecord({ ...base, novelId: "n2", chapterId: "c9" }), storage);

  const first = readEditPosition("n1", storage);
  assert.equal(first.chapterId, "c1");
  assert.equal(readEditPosition("n2", storage).chapterId, "c9");
  assert.equal(readEditPosition("n3", storage), null, "别的书不能读到这本书的位置");

  // 记录里只有坐标与标识：字段名必须正好是这些，不能夹带正文或密钥。
  const parsed = JSON.parse(storage.getItem("ai-novel:edit-position:v1:n1"));
  assert.deepEqual(
    Object.keys(parsed).sort(),
    ["chapterId", "contentLength", "cursorOffset", "novelId", "officialVersion", "scrollTop", "updatedAt"],
  );
});

test("损坏的位置记录不会阻断打开章节", () => {
  const storage = memoryStorage();
  storage.setItem("ai-novel:edit-position:v1:n1", "{not json");
  assert.equal(readEditPosition("n1", storage), null);

  storage.setItem("ai-novel:edit-position:v1:n1", JSON.stringify({ novelId: "n1", chapterId: "c1" }));
  assert.equal(readEditPosition("n1", storage), null, "缺字段的记录必须丢弃而不是猜");
});

test("只有同一章、正文已恢复且坐标在范围内才恢复", () => {
  const record = buildEditPositionRecord(base);
  assert.deepEqual(
    resolveEditPositionRestore({ record, chapterId: "c1", officialVersion: 3, contentLength: 100 }),
    { cursorOffset: 12, scrollTop: 240 },
  );
  // 切到别的章：不能把光标放到不相干的位置。
  assert.equal(
    resolveEditPositionRestore({ record, chapterId: "c2", officialVersion: 3, contentLength: 100 }),
    null,
  );
  // 正文还没恢复完成。
  assert.equal(
    resolveEditPositionRestore({ record, chapterId: "c1", officialVersion: 3, contentLength: 0 }),
    null,
  );
});

test("版本变化后长度不足时放弃恢复，而不是把光标夹到末尾", () => {
  const record = buildEditPositionRecord({ ...base, contentLength: 100, cursorOffset: 80 });
  // 新版本短了很多：原坐标已无意义。
  assert.equal(
    resolveEditPositionRestore({ record, chapterId: "c1", officialVersion: 4, contentLength: 40 }),
    null,
  );
  // 新版本仍能容纳原坐标：保留光标位置。
  assert.deepEqual(
    resolveEditPositionRestore({ record, chapterId: "c1", officialVersion: 4, contentLength: 120 }),
    { cursorOffset: 80, scrollTop: 240 },
  );
  // 坐标越界同样放弃。
  assert.equal(
    resolveEditPositionRestore({ record, chapterId: "c1", officialVersion: 4, contentLength: 50 }),
    null,
  );
});

test("越界或负数值在记录时就被收敛", () => {
  const record = buildEditPositionRecord({ ...base, cursorOffset: -5, scrollTop: -10, contentLength: 30 });
  assert.equal(record.cursorOffset, 0);
  assert.equal(record.scrollTop, 0);
  assert.equal(record.contentLength, 30);
});

test("清除后不再恢复", () => {
  const storage = memoryStorage();
  writeEditPosition(buildEditPositionRecord(base), storage);
  clearEditPosition("n1", storage);
  assert.equal(readEditPosition("n1", storage), null);
  assert.equal(storage.size(), 0);
});

// ---- 桌面版重新打开时回到上次写作位置 ----

test("最近编辑章节按书隔离存储，重启后能读回", () => {
  const storage = memoryStorage();
  writeRecentEditTarget({ novelId: "n1", chapterId: "c3", updatedAt: 100 }, storage);
  assert.deepEqual(readRecentEditTarget(storage), { novelId: "n1", chapterId: "c3", updatedAt: 100 });

  // 后一次编辑覆盖前一次，只保留最新章节。
  writeRecentEditTarget({ novelId: "n2", chapterId: "c9", updatedAt: 200 }, storage);
  assert.deepEqual(readRecentEditTarget(storage), { novelId: "n2", chapterId: "c9", updatedAt: 200 });

  // 损坏记录不能把作者带到错误位置。
  storage.setItem("ai-novel:edit-position:v1:recent", "{\"novelId\":\"n1\"}");
  assert.equal(readRecentEditTarget(storage), null);
});

test("只有没有显式启动路由时才回到上次章节", () => {
  const recent = { novelId: "n1", chapterId: "c3", updatedAt: 100 };
  assert.equal(
    startupEditRoute({ startRoute: null, recent }),
    "/novels/n1/chapters/c3",
  );
  // 显式路由（例如从任务中心深链进入）优先，不能被最近记录覆盖。
  assert.equal(startupEditRoute({ startRoute: "/tasks", recent }), null);
  // 没有历史记录时留在默认首页。
  assert.equal(startupEditRoute({ startRoute: null, recent: null }), null);
});

// ---- 光标偏移 ↔ 编辑器坐标必须互逆 ----

test("光标偏移能还原成编辑器坐标，且与正向计算互逆", async () => {
  const { toPlateValue, toPlainText, getCursorOffsetFromValue, pointFromCursorOffset } = await import("./editorDocument.ts");
  const text = "他推开门，屋里没有人。\n\n灯还亮着，桌上摊着半张地图。\n\n他走到窗前，听见远处有人敲门。";
  const value = toPlateValue(text);
  assert.equal(toPlainText(value), text, "编辑器值必须能还原正文");

  // 对每个合法偏移做一次往返：offset → point → offset 必须相等。
  for (let offset = 0; offset <= text.length; offset += 1) {
    const point = pointFromCursorOffset(value, offset);
    if (!point) {
      // 段落分隔处的中间位置允许归到下一段段首或上一段段尾。
      continue;
    }
    const back = getCursorOffsetFromValue(value, point);
    assert.equal(typeof back, "number", `偏移 ${offset} 必须能算出坐标`);
    if (offset !== back) {
      // 只有落在 \n\n 分隔符中间时才允许偏移归位到段落边界。
      const gapStart = back;
      assert.ok(
        text.slice(Math.min(offset, gapStart), Math.max(offset, gapStart)).replace(/\n/g, "") === "",
        `偏移 ${offset} 还原成 ${back} 时只能跨过换行分隔符`,
      );
    }
  }

  // 文末偏移是合法光标位置；真正越界与负数必须拒绝，不能夹到末尾。
  assert.notEqual(pointFromCursorOffset(value, text.length), null, "文末是合法光标位置");
  assert.equal(pointFromCursorOffset(value, text.length + 5), null);
  assert.equal(pointFromCursorOffset(value, -1), null);
});
