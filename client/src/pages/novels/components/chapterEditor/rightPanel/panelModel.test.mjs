import assert from "node:assert/strict";
import test from "node:test";
import {
  availableRunActions,
  buildRunCommandRequest,
  formatRunStatus,
  isRecoverableStatus,
  readStoredPanelTab,
  storePanelTab,
  summarizeCast,
} from "./panelModel.ts";

const candidate = { id: "cand-1", content: "候选正文", contentSha256: "sha-1" };

const commandInput = {
  runId: "run-1",
  commandId: "cmd-1",
  controlVersion: 7,
  candidate,
  qualityDebtReason: "带质量提醒继续",
};

function requestFor(key, context, overrides = {}) {
  const action = availableRunActions(context).find((item) => item.key === key);
  assert.ok(action, `动作 ${key} 应该可用`);
  return buildRunCommandRequest(action, { ...commandInput, ...overrides });
}

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
  // 断言真实请求：候选确认必须带齐 CAS 三件套 + expectedVersion，少一个就可能覆盖别人改过的正文。
  const request = buildRunCommandRequest(actions[0], commandInput);
  assert.deepEqual(request, {
    command: "confirm_candidate_after_edit_run",
    payload: {
      runId: "run-1",
      commandId: "cmd-1",
      expectedVersion: 7,
      baseCandidateId: "cand-1",
      baseContentSha256: "sha-1",
      content: "候选正文",
    },
  });
});

test("candidate confirm refuses to send when CAS parts are missing", () => {
  const context = {
    legalActions: ["confirm"],
    status: "waiting_approval",
    gateId: "approval",
    candidate,
    controlVersion: 7,
  };
  // 候选在发起后失效（例如详情还没刷新到）：宁可报错，也不能发一个缺 CAS 的确认请求。
  for (const broken of [null, { id: "cand-1", content: "正文", contentSha256: null }, { id: "cand-1", content: "", contentSha256: "sha-1" }]) {
    assert.throws(() => requestFor("confirm_candidate", context, { candidate: broken }), /候选正文还没准备好/);
  }
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
  // 断言真实请求：质量债恢复必须把原因发出去，否则运行记录里查不到为什么放行。
  assert.deepEqual(buildRunCommandRequest(quality, commandInput), {
    command: "resume_run_with_quality_debt",
    payload: { runId: "run-1", commandId: "cmd-1", reason: "带质量提醒继续" },
  });
});

/**
 * 每个命令实际发出的参数键必须与 `src-tauri/src/commands/run.rs` 的签名一致。
 * 关键事实：resume_run 只接 accept_candidate、cancel_run 只接 run/command，两者都不接 expected_version；
 * 多发一个 Tauri 会直接拒绝请求，少发一个则丢掉并发保护。
 */
test("each command sends exactly the parameters its rust signature accepts", () => {
  const paused = { legalActions: ["resume", "cancel"], status: "paused" };
  const gate = {
    legalActions: ["confirm", "reject", "regenerate"],
    status: "waiting_approval",
    gateId: "character_cast",
    candidate: null,
  };

  const expected = [
    [requestFor("approve", gate), "approve_run", ["runId", "commandId", "expectedVersion"]],
    [requestFor("reject", gate), "reject_run", ["runId", "commandId", "expectedVersion"]],
    [requestFor("regenerate", gate), "regenerate_run", ["runId", "commandId", "expectedVersion"]],
    [requestFor("resume", paused), "resume_run", ["runId", "commandId", "acceptCandidate"]],
    [requestFor("cancel", paused), "cancel_run", ["runId", "commandId"]],
  ];

  for (const [request, command, keys] of expected) {
    assert.equal(request.command, command);
    assert.deepEqual(Object.keys(request.payload).sort(), [...keys].sort(), `${command} 的参数必须与命令签名一致`);
  }

  // resume / cancel 没有 expected_version 形参：带上去 Tauri 会拒绝整个请求。
  assert.equal("expectedVersion" in requestFor("resume", paused).payload, false);
  assert.equal("expectedVersion" in requestFor("cancel", paused).payload, false);
  // resume 当前只做「继续」，不顺手接受候选稿。
  assert.equal(requestFor("resume", paused).payload.acceptCandidate, false);
});

test("missing control version omits expectedVersion instead of sending null", () => {
  const gate = { legalActions: ["confirm"], status: "waiting_approval", gateId: "character_cast", candidate: null };
  const request = requestFor("approve", gate, { controlVersion: null });
  assert.equal(request.payload.expectedVersion, undefined);
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
