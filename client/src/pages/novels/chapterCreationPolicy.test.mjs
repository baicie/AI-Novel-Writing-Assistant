import assert from "node:assert/strict";
import test from "node:test";

import {
  buildNewChapterDraft,
  defaultChapterTitle,
} from "./chapterCreationPolicy.ts";

const chapter = (order) => ({ order });

test("empty book starts at the first Chinese chapter title", () => {
  assert.deepEqual(buildNewChapterDraft([]), { order: 1, title: "第1章" });
  assert.equal(defaultChapterTitle(7), "第7章");
});

test("sequential chapters continue from the highest order", () => {
  assert.deepEqual(buildNewChapterDraft([chapter(1), chapter(2), chapter(3)]), {
    order: 4,
    title: "第4章",
  });
});

test("gaps left by deleting a middle chapter do not produce a duplicate order", () => {
  // 删掉第 3 章后列表长度是 3，但最大顺序仍是 4；用长度 + 1 会撞上已有的第 4 章。
  assert.deepEqual(buildNewChapterDraft([chapter(1), chapter(2), chapter(4)]), {
    order: 5,
    title: "第5章",
  });
});

test("reordered chapters still continue after the largest order", () => {
  assert.deepEqual(buildNewChapterDraft([chapter(3), chapter(1), chapter(2)]), {
    order: 4,
    title: "第4章",
  });
});

test("clearing every chapter starts numbering again", () => {
  assert.deepEqual(buildNewChapterDraft([]), { order: 1, title: "第1章" });
});

test("invalid orders are ignored instead of producing a broken title", () => {
  assert.deepEqual(
    buildNewChapterDraft([chapter(Number.NaN), chapter(0), chapter(-2), chapter(1.8)]),
    { order: 2, title: "第2章" },
  );
});

test("a just-issued order still blocks duplicates while the chapter list is refreshing", () => {
  // 创建成功到详情刷新完成之间，列表可能还是旧数据；刚发出的顺序必须继续生效。
  assert.deepEqual(buildNewChapterDraft([chapter(1)], 2), { order: 3, title: "第3章" });
  assert.deepEqual(buildNewChapterDraft([chapter(1), chapter(2)], 1), { order: 3, title: "第3章" });
});

test("the policy never writes back existing chapter names", () => {
  const existing = [{ order: 1, title: "作者自己取的名字" }];
  const draft = buildNewChapterDraft(existing);
  assert.equal(draft.title, "第2章");
  assert.equal(existing[0].title, "作者自己取的名字");
});
