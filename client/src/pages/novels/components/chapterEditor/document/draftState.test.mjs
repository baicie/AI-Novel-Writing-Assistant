import assert from "node:assert/strict";
import test from "node:test";
import { acknowledgeChapterSave, receiveChapter, restoreChapterDraft } from "./draftState.ts";

const saved = { content: "甲\n\n乙", savedContent: "甲\n\n乙", savedUpdatedAt: "2026-10-11T00:00:00.000Z" };

test("chapter refresh keeps an unsaved author draft", () => {
  const draft = { ...saved, content: "甲\n\n作者刚改的乙" };
  assert.deepEqual(receiveChapter(draft, { content: "甲\n\n后台新版本", updatedAt: "2026-10-11T00:01:00.000Z" }, false), draft);
});

test("chapter refresh accepts a newer saved version when the editor is clean", () => {
  assert.deepEqual(receiveChapter(saved, { content: "甲\n\n后台新版本", updatedAt: "2026-10-11T00:01:00.000Z" }, false), {
    content: "甲\n\n后台新版本",
    savedContent: "甲\n\n后台新版本",
    savedUpdatedAt: "2026-10-11T00:01:00.000Z",
  });
});

test("save acknowledgement preserves newer local typing", () => {
  const draft = { ...saved, content: "甲\n\n本地继续输入" };
  assert.deepEqual(acknowledgeChapterSave(draft, "甲\n\n刚提交", { content: "甲\n\n刚提交", updatedAt: "2026-10-11T00:02:00.000Z" }), {
    content: "甲\n\n本地继续输入",
    savedContent: "甲\n\n刚提交",
    savedUpdatedAt: "2026-10-11T00:02:00.000Z",
  });
});

test("invalid draft storage falls back to the saved version", () => {
  assert.deepEqual(restoreChapterDraft("{broken", saved), saved);
});

test("busy generation refresh does not replace the author draft", () => {
  const draft = { ...saved, content: "甲\n\n作者正在写" };
  assert.equal(
    receiveChapter(draft, { content: "甲\n\n后台新版本", updatedAt: "2026-10-11T00:03:00.000Z" }, true),
    draft,
  );
});

test("save acknowledgement for older text never overwrites newer typing", () => {
  const draft = { ...saved, content: "甲\n\n继续输入的新正文" };
  const acknowledged = acknowledgeChapterSave(draft, "甲\n\n提交时的旧正文", {
    content: "甲\n\n提交时的旧正文",
    updatedAt: "2026-10-11T00:04:00.000Z",
  });
  assert.equal(acknowledged.content, "甲\n\n继续输入的新正文");
  assert.equal(acknowledged.savedContent, "甲\n\n提交时的旧正文");
});
