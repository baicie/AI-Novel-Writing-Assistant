import assert from "node:assert/strict";
import test from "node:test";
import {
  availableRunActions,
  formatRunStatus,
  isRecoverableStatus,
  readStoredPanelTab,
  storePanelTab,
  summarizeCast,
} from "./panelModel.ts";

test("legal actions map to real entries and drop unknown ones", () => {
  assert.deepEqual(
    availableRunActions(["confirm", "reject", "regenerate", "confirm_after_edit"]).map((item) => item.key),
    ["approve", "regenerate"],
  );
  assert.deepEqual(availableRunActions(["resume_with_quality_debt"]).map((item) => item.key), ["resume_debt"]);
  assert.deepEqual(availableRunActions(["something_new"]), []);
  assert.deepEqual(availableRunActions(null), []);
});

test("dedupes repeated actions and keeps first-seen order", () => {
  assert.deepEqual(availableRunActions(["confirm", "approve", "cancel"]).map((item) => item.key), ["approve", "cancel"]);
});

test("status text falls back to the raw value instead of inventing one", () => {
  assert.equal(formatRunStatus("waiting_approval"), "等待你的确认");
  assert.equal(formatRunStatus("weird_status"), "weird_status");
  assert.equal(formatRunStatus(null), "暂无运行");
  assert.equal(isRecoverableStatus("paused"), true);
  assert.equal(isRecoverableStatus("succeeded"), false);
});

test("cast summary counts roles without inventing characters", () => {
  const summary = summarizeCast([
    { role: "主角", storyFunction: "承载" },
    { role: "同伴", storyFunction: "协助" },
    { role: "对手", storyFunction: "施压" },
    { role: "村民" },
  ]);
  assert.deepEqual(summary, { total: 4, protagonists: 1, allies: 1, antagonists: 1 });
  assert.deepEqual(summarizeCast([]), { total: 0, protagonists: 0, allies: 0, antagonists: 0 });
});

test("panel tab is remembered per novel and falls back safely", () => {
  const store = new Map();
  const storage = {
    getItem: (key) => (store.has(key) ? store.get(key) : null),
    setItem: (key, value) => store.set(key, value),
  };
  assert.equal(readStoredPanelTab("novel-1", storage), "revise");
  storePanelTab("novel-1", "run", storage);
  assert.equal(readStoredPanelTab("novel-1", storage), "run");
  // 另一本书不受影响。
  assert.equal(readStoredPanelTab("novel-2", storage), "revise");
  // 脏数据回退到默认页签，而不是抛错。
  storage.setItem("chapter-workbench-tab:novel-3", "not-a-tab");
  assert.equal(readStoredPanelTab("novel-3", storage), "revise");
  assert.equal(readStoredPanelTab("novel-4", null), "revise");
});

test("storage failures never break the workbench", () => {
  const broken = {
    getItem: () => {
      throw new Error("blocked");
    },
    setItem: () => {
      throw new Error("quota");
    },
  };
  assert.equal(readStoredPanelTab("novel-x", broken), "revise");
  assert.doesNotThrow(() => storePanelTab("novel-x", "cast", broken));
});
