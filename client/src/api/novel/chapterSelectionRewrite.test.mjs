import assert from "node:assert/strict";
import test from "node:test";
import {
  applySelectedChapterEditorDiff,
  groupChapterEditorDiffChanges,
} from "@ai-novel/shared/types/chapterEditor/document";
import {
  buildRewriteCommandInput,
  resolveRewriteTargetRange,
  resolvedIntentFor,
  toEditorCandidates,
} from "./chapterSelectionRewriteModel.ts";

const CONTENT = "她抬头看向门口。\n\n风从缝里挤进来。";

function selectionRequest(overrides = {}) {
  return {
    source: "preset",
    scope: "selection",
    presetOperation: "polish",
    contentSnapshot: CONTENT,
    selection: { from: 3, to: 7, text: "看向门口" },
    constraints: { keepFacts: true, keepPov: true, noUnauthorizedSetting: true, preserveCoreInfo: true },
    ...overrides,
  };
}

test("选区坐标必须与当前正文逐字对得上，对不上要报错而不是猜", () => {
  assert.deepEqual(resolveRewriteTargetRange(selectionRequest()), {
    from: 3,
    to: 7,
    text: "看向门口",
  });

  // 正文已被改动：坐标还在，但那段文字已经不是作者选的了。
  assert.throws(
    () => resolveRewriteTargetRange(selectionRequest({ contentSnapshot: "她抬头望向远处。\n\n风。" })),
    /正文片段已变化/,
  );
  assert.throws(
    () => resolveRewriteTargetRange(selectionRequest({ selection: { from: 3, to: 999, text: "看向门口" } })),
    /选区超出正文范围/,
  );
  assert.throws(
    () => resolveRewriteTargetRange(selectionRequest({ selection: { from: 5, to: 5, text: "" } })),
    /选区超出正文范围/,
  );
  assert.throws(() => resolveRewriteTargetRange(selectionRequest({ selection: undefined })), /请先选中/);
});

test("整章改写把整段正文作为目标区间，不需要选区", () => {
  const range = resolveRewriteTargetRange(selectionRequest({ scope: "chapter", selection: undefined }));
  assert.deepEqual(range, { from: 0, to: CONTENT.length, text: CONTENT });
});

test("候选的 diff 分片必须能被采纳路径重建，否则采纳时会整段失败", () => {
  const candidates = toEditorCandidates(
    [{ label: " ", content: " 望向漆黑的门口 ", riskNotes: [" ", "语气更重"], semanticTags: [] }],
    "看向门口",
    (() => {
      let seq = 0;
      return () => `cand-${(seq += 1)}`;
    })(),
  );

  assert.equal(candidates.length, 1);
  assert.equal(candidates[0].id, "cand-1");
  assert.equal(candidates[0].label, "方案 1", "空 label 要有稳定回退");
  assert.equal(candidates[0].content, "望向漆黑的门口", "候选文本要去掉首尾空白");
  assert.deepEqual(candidates[0].riskNotes, ["语气更重"], "空风险项要去掉");

  // 这是和 applySelectedChapterEditorDiff 的硬契约：分片必须能重建原文与候选。
  const chunks = candidates[0].diffChunks;
  assert.equal(
    applySelectedChapterEditorDiff("看向门口", "望向漆黑的门口", chunks, new Set()),
    "看向门口",
    "不选任何修改应当回到原文",
  );
  // 变更 id 必须取自 groupChapterEditorDiffChanges：那是采纳路径认的唯一来源。
  const allChanges = new Set(groupChapterEditorDiffChanges(chunks).map((change) => change.id));
  assert.equal(
    applySelectedChapterEditorDiff("看向门口", "望向漆黑的门口", chunks, allChanges),
    "望向漆黑的门口",
    "全选修改应当得到候选全文",
  );
});

test("空白候选被丢弃，不编造可用候选", () => {
  assert.deepEqual(toEditorCandidates([{ content: "   " }], "原文", () => "x"), []);
});

test("发给内核的参数带上选区原文与前后文，缺上下文时传空串而不是字面量", () => {
  const request = selectionRequest({
    instruction: "  更压抑一点  ",
    presetOperation: "custom",
    source: "freeform",
    context: { beforeParagraphs: ["前一段"], afterParagraphs: ["后一段", "再后一段"] },
  });
  const input = buildRewriteCommandInput({
    requestId: "req-1",
    novelId: "n1",
    chapterId: "c1",
    chapterTitle: "第一章",
    request,
    targetRange: resolveRewriteTargetRange(request),
  });

  assert.equal(input.selectedText, "看向门口");
  assert.equal(input.selectionFrom, 3);
  assert.equal(input.selectionTo, 7);
  assert.equal(input.contentSnapshot, CONTENT, "快照必须是内核校验坐标用的同一份正文");
  assert.equal(input.instruction, "更压抑一点", "补充要求要去空白");
  assert.equal(input.operationLabel, "按作者要求修改");
  assert.equal(input.scopeLabel, "选中片段");
  assert.equal(input.beforeContext, "前一段");
  assert.equal(input.afterContext, "后一段\n\n再后一段");
  assert.equal(input.chapterTitle, "第一章");

  const bare = buildRewriteCommandInput({
    requestId: "req-2",
    novelId: "n1",
    chapterId: "c1",
    request: selectionRequest(),
    targetRange: resolveRewriteTargetRange(selectionRequest()),
  });
  assert.equal(bare.beforeContext, "", "没有上下文就传空串，交给内核按「无」渲染");
  assert.equal(bare.instruction, undefined);
  assert.equal(bare.chapterTitle, "");
});

test("改写意图沿用作者选的操作，不额外发一次模型请求去解析意图", () => {
  const preset = resolvedIntentFor(selectionRequest());
  assert.equal(preset.editGoal, "润色这段");
  assert.equal(preset.strength, "light");
  assert.ok(preset.mustPreserve.includes("剧情事实"));

  const freeform = resolvedIntentFor(
    selectionRequest({ source: "freeform", presetOperation: undefined, instruction: "更压抑" }),
  );
  assert.equal(freeform.editGoal, "更压抑", "有补充要求时以作者原话为目标");
  assert.equal(freeform.strength, "medium");
});
