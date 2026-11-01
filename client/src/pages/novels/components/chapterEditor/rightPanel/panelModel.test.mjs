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

const candidate = { id: "cand-1", content: "候选正文", contentSha256: "sha-1" };

test("waiting approval maps to real director commands, including candidate CAS", () => {
  const actions = availableRunActions({
    legalActions: ["confirm", "reject", "regenerate", "confirm_after_edit"],
    status: "waiting_approval",
    gateId: "approval",
    candidate,
    controlVersion: 3,
  });
  assert.deepEqual(
    actions.map((action) => [action.key, action.command]),
    [
      ["confirm_candidate", "confirm_candidate_after_edit_run"],
      ["reject", "reject_run"],
      ["regenerate", "regenerate_run"],
    ],
  );
  const confirm = actions[0];
  assert.equal(confirm.needsCandidate, true, "章节确认必须带候选 CAS 参数");
  assert.equal(confirm.needsVersion, true);
});

test("planning gate without candidate uses plain approve, never the candidate command", () => {
  const actions = availableRunActions({
    legalActions: ["confirm", "reject", "regenerate", "confirm_after_edit"],
    status: "waiting_approval",
    gateId: "character_cast",
    candidate: null,
    controlVersion: 1,
  });
  assert.deepEqual(
    actions.map((action) => action.command),
    ["approve_run", "reject_run", "regenerate_run"],
  );
});

test("confirm_after_edit is not rendered when no candidate exists", () => {
  const actions = availableRunActions({
    legalActions: ["confirm_after_edit"],
    status: "waiting_approval",
    candidate: null,
  });
  assert.deepEqual(actions, []);
});

test("regenerate never falls back to resume", () => {
  const actions = availableRunActions({ legalActions: ["regenerate"], status: "waiting_approval" });
  assert.equal(actions.length, 1);
  assert.equal(actions[0].command, "regenerate_run");
});

test("paused states expose resume / quality debt / cancel with correct commands", () => {
  const plain = availableRunActions({ legalActions: ["resume", "cancel"], status: "paused" });
  assert.deepEqual(
    plain.map((action) => action.command),
    ["resume_run", "cancel_run"],
  );

  const safety = availableRunActions({ legalActions: ["cancel"], status: "paused" });
  assert.deepEqual(safety.map((action) => action.command), ["cancel_run"]);

  const debt = availableRunActions({ legalActions: ["cancel", "resume_quality_debt"], status: "paused" });
  const quality = debt.find((action) => action.key === "resume_quality_debt");
  assert.equal(quality.command, "resume_run_with_quality_debt");
  assert.equal(quality.needsReason, true, "质量债恢复必须带原因");
});

test("unknown backend actions are never rendered as buttons", () => {
  assert.deepEqual(availableRunActions({ legalActions: ["something_new"], status: "paused" }), []);
  assert.deepEqual(availableRunActions({ legalActions: null }), []);
});

test("duplicate keys collapse to one button", () => {
  const actions = availableRunActions({
    legalActions: ["confirm", "confirm_after_edit"],
    status: "waiting_approval",
    gateId: "approval",
    candidate,
  });
  assert.deepEqual(actions.map((action) => action.key), ["confirm_candidate"]);
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
  assert.equal(readStoredPanelTab("novel-2", storage), "revise");
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
