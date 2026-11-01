import assert from "node:assert/strict";
import test from "node:test";
import {
  applySelectedChapterEditorDiff,
  buildChapterEditorDiffChunks,
  chapterParagraphs,
  groupChapterEditorDiffChanges,
  insertChapterContinuation,
  normalizeChapterText,
  replaceChapterSelection,
} from "./document.ts";

test("chapter document coordinates preserve authored whitespace", () => {
  const content = normalizeChapterText("  第一段\r\n\r\n第二段\n");
  assert.equal(content, "  第一段\n\n第二段\n");
  assert.deepEqual(chapterParagraphs(content), [
    { index: 1, text: "  第一段", from: 0, to: 5 },
    { index: 2, text: "第二段\n", from: 7, to: 11 },
  ]);
});

test("chapter selection replacement rejects stale text", () => {
  assert.equal(replaceChapterSelection("甲\n乙", { from: 2, to: 3, text: "乙" }, "丙"), "甲\n丙");
  assert.throws(
    () => replaceChapterSelection("甲\n新乙", { from: 2, to: 3, text: "乙" }, "丙"),
    /正文片段已变化/,
  );
});

test("chapter continuation inserts at an exact cursor offset", () => {
  assert.equal(insertChapterContinuation("甲\n乙", 2, "，然后转身"), "甲\n，然后转身乙");
  assert.throws(
    () => insertChapterContinuation("甲", 2, "乙"),
    /正文光标位置已变化/,
  );
});

test("chapter diff groups replacements so partial acceptance cannot duplicate source text", () => {
  const chunks = [
    { id: "equal-1", type: "equal", text: "她抬头" },
    { id: "delete-1", type: "delete", text: "看向门口" },
    { id: "insert-1", type: "insert", text: "望向漆黑的门口" },
    { id: "equal-2", type: "equal", text: "。" },
    { id: "insert-2", type: "insert", text: "\n" },
  ];
  const changes = groupChapterEditorDiffChanges(chunks);
  assert.deepEqual(changes.map(({ kind, originalText, candidateText }) => ({ kind, originalText, candidateText })), [
    { kind: "replace", originalText: "看向门口", candidateText: "望向漆黑的门口" },
    { kind: "insert", originalText: "", candidateText: "\n" },
  ]);
  const candidate = "她抬头望向漆黑的门口。\n";
  assert.equal(applySelectedChapterEditorDiff("她抬头看向门口。", candidate, chunks, new Set()), "她抬头看向门口。");
  assert.equal(applySelectedChapterEditorDiff("她抬头看向门口。", candidate, chunks, new Set(["change-1"])), "她抬头望向漆黑的门口。");
  assert.equal(applySelectedChapterEditorDiff("她抬头看向门口。", candidate, chunks, new Set(["change-2"])), "她抬头看向门口。\n");
});

test("built diff chunks always satisfy the apply path's reconstruction check", () => {
  // buildChapterEditorDiffChunks 的产物必须能被 applySelectedChapterEditorDiff 接受：
  // 这两个函数一个在生成侧、一个在采纳侧，分片口径不一致会让作者采纳时直接报错。
  const cases = [
    ["她抬头看向门口。", "她抬头望向漆黑的门口。"],
    ["", "全新的一段正文"],
    ["要删掉的旧段落", ""],
    ["完全一致的文本", "完全一致的文本"],
    ["第一段\n\n第二段", "第一段\n\n改写后的第二段\n\n新增第三段"],
    ["Mixed 中英 text 42", "Mixed 中英文 text 7"],
  ];
  for (const [original, candidate] of cases) {
    const chunks = buildChapterEditorDiffChunks(original, candidate);
    assert.equal(
      chunks.filter((chunk) => chunk.type !== "insert").map((chunk) => chunk.text).join(""),
      original,
      `删除+相同必须还原原文：${JSON.stringify(original)}`,
    );
    assert.equal(
      chunks.filter((chunk) => chunk.type !== "delete").map((chunk) => chunk.text).join(""),
      candidate,
      `插入+相同必须还原候选：${JSON.stringify(candidate)}`,
    );
    // 全不选 = 原文，全选 = 候选。这是作者在 Diff 面板里的两个端点。
    assert.equal(applySelectedChapterEditorDiff(original, candidate, chunks, new Set()), original);
    const allChanges = new Set(groupChapterEditorDiffChanges(chunks).map((change) => change.id));
    assert.equal(applySelectedChapterEditorDiff(original, candidate, chunks, allChanges), candidate);
  }
});

test("diff chunk ids are unique so partial acceptance can address each change", () => {
  const chunks = buildChapterEditorDiffChunks("她抬头看向门口。", "她抬头望向漆黑的门口。\n");
  assert.equal(new Set(chunks.map((chunk) => chunk.id)).size, chunks.length, "分片 id 不能重复");
  assert.ok(chunks.every((chunk) => ["equal", "insert", "delete"].includes(chunk.type)));
});
