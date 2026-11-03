import assert from "node:assert/strict";
import test from "node:test";
import { isEditorSnapshotRequest } from "../src/platform/desktopPrimitives.ts";

// P17.2 RC1：桌面端没有整书快照能力。
// 编辑器内部的保护性调用可以降级为章节修订，但作者主动创建快照必须明确不可用。
test("编辑器内部的保护性快照请求才降级", () => {
  // 采纳候选：useCandidateAcceptance 生成的标签。
  assert.equal(isEditorSnapshotRequest("chapter-editor:3:selection:1700000000000"), true);
  // 载入历史版本前：ChapterEditorShell 生成的标签。
  assert.equal(isEditorSnapshotRequest("chapter-editor:before-version-load:2:1700000000000"), true);
  // 作者主动点「创建快照」：不能被降级成假成功。
  assert.equal(isEditorSnapshotRequest("manual-2026/11/03 10:00:00"), false);
  // 其它来源的标签同样不能降级。
  assert.equal(isEditorSnapshotRequest("before_pipeline"), false);
  assert.equal(isEditorSnapshotRequest(""), false);
  // 缺失或非字符串标签按「不能降级」处理，宁可报未迁移也不伪造。
  assert.equal(isEditorSnapshotRequest(undefined), false);
  assert.equal(isEditorSnapshotRequest(null), false);
  assert.equal(isEditorSnapshotRequest(42), false);
  // 前缀相近但不是编辑器标签的情况也不能误判。
  assert.equal(isEditorSnapshotRequest("chapter-editor"), false);
  assert.equal(isEditorSnapshotRequest("xchapter-editor:1"), false);
});
