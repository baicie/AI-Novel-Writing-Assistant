import assert from "node:assert/strict";
import test from "node:test";
import { availableRunActions, formatRunStatus, isRecoverableStatus, summarizeCast } from "./panelModel.ts";

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
