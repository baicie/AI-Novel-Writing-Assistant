import assert from "node:assert/strict";
import test from "node:test";
import {
  buildChapterGroups,
  buildRunAttention,
  filterChapters,
  formatChapterWordCount,
  isSaveShortcut,
  neighbourOf,
  runAttentionFor,
  shouldBlockChapterSwitch,
} from "./chapterNavigatorModel.ts";

const chapters = [
  { id: "c3", title: "第三章", order: 3, content: "有正文", officialVersion: 2, volumeId: "v2" },
  { id: "c1", title: "第一章", order: 1, content: "", officialVersion: 0, volumeId: "v1" },
  { id: "c2", title: "第二章", order: 2, content: "x", officialVersion: 1, volumeId: "v1" },
];

test("chapters are grouped by volume and ordered", () => {
  const groups = buildChapterGroups(chapters, [
    { id: "v1", title: "第一卷" },
    { id: "v2", title: "第二卷" },
  ]);
  assert.deepEqual(groups.map((group) => group.label), ["第一卷", "第二卷"]);
  assert.deepEqual(groups[0].chapters.map((chapter) => chapter.id), ["c1", "c2"]);
  assert.deepEqual(groups[1].chapters.map((chapter) => chapter.id), ["c3"]);
});

test("chapters without a known volume fall back to a safe group", () => {
  const groups = buildChapterGroups([{ id: "c9", title: "", order: 9 }], []);
  assert.equal(groups.length, 1);
  assert.equal(groups[0].label, "未分卷");
  assert.equal(groups[0].chapters[0].id, "c9");
});

test("search matches by order or title and ignores blank keyword", () => {
  assert.equal(filterChapters(chapters, "").length, 3);
  assert.deepEqual(filterChapters(chapters, "第三").map((chapter) => chapter.id), ["c3"]);
  assert.deepEqual(filterChapters(chapters, "2").map((chapter) => chapter.id), ["c2"]);
});

test("previous and next respect order boundaries", () => {
  assert.deepEqual(neighbourOf(chapters, "c2"), {
    previous: chapters.find((chapter) => chapter.id === "c1"),
    next: chapters.find((chapter) => chapter.id === "c3"),
  });
  assert.equal(neighbourOf(chapters, "c1").previous, null);
  assert.equal(neighbourOf(chapters, "c3").next, null);
  assert.deepEqual(neighbourOf(chapters, "missing"), { previous: null, next: null });
});

test("dirty chapter switch requires explicit confirmation", () => {
  const refused = shouldBlockChapterSwitch(true, () => false);
  assert.equal(refused, true, "用户拒绝时必须留在本章");

  const accepted = shouldBlockChapterSwitch(true, () => true);
  assert.equal(accepted, false, "用户同意后应当允许切换");

  let asked = false;
  const clean = shouldBlockChapterSwitch(false, () => {
    asked = true;
    return true;
  });
  assert.equal(clean, false);
  assert.equal(asked, false, "没有未保存修改时不应打扰用户");
});

test("run attention maps backend states to actionable levels", () => {
  assert.equal(runAttentionFor("waiting_approval"), "action");
  assert.equal(runAttentionFor("paused"), "action");
  assert.equal(runAttentionFor("failed"), "action");
  assert.equal(runAttentionFor("running"), "running");
  assert.equal(runAttentionFor("succeeded"), "done");
  assert.equal(runAttentionFor(null), null);
  assert.equal(runAttentionFor("something_new"), null);
});

test("latest run per chapter decides the badge", () => {
  const attention = buildRunAttention([
    { chapterId: "c1", state: "succeeded", updatedAt: 10 },
    { chapterId: "c1", state: "paused", updatedAt: 20 },
    { chapterId: "c2", state: "running", updatedAt: 5 },
    { chapterId: "c3", state: "succeeded", updatedAt: 7 },
  ]);
  assert.deepEqual(attention, { c1: "action", c2: "running", c3: "done" });
});

test("save shortcut only matches cmd/ctrl + s without alt", () => {
  assert.equal(isSaveShortcut({ key: "s", metaKey: true }), true);
  assert.equal(isSaveShortcut({ key: "S", ctrlKey: true }), true);
  assert.equal(isSaveShortcut({ key: "s" }), false);
  assert.equal(isSaveShortcut({ key: "s", metaKey: true, altKey: true }), false);
  assert.equal(isSaveShortcut({ key: "k", metaKey: true }), false);
});

test("chapter word count ignores blank content", () => {
  assert.equal(formatChapterWordCount(""), "空");
  assert.equal(formatChapterWordCount(null), "空");
  assert.equal(formatChapterWordCount("   "), "空");
  assert.equal(formatChapterWordCount("少年入山"), "4 字");
});
