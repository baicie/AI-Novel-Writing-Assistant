import test from "node:test";
import assert from "node:assert/strict";

import {
  describeUpdateState,
  initialUpdateState,
  progressPercent,
  updateReducer,
} from "../src/pages/settings/updateCheckState.ts";

test("检查更新：从空闲到已是最新", () => {
  let state = updateReducer(initialUpdateState, { type: "check-started" });
  assert.equal(state.phase, "checking");
  state = updateReducer(state, { type: "check-none" });
  assert.equal(state.phase, "up-to-date");
  assert.equal(describeUpdateState(state, { desktop: true }).label, "已经是最新版本");
});

test("检查更新：发现新版本时保留版本号与更新说明", () => {
  const state = updateReducer(
    updateReducer(initialUpdateState, { type: "check-started" }),
    { type: "check-found", version: "0.2.0", notes: "  修复若干问题  " },
  );
  assert.equal(state.phase, "available");
  assert.equal(state.availableVersion, "0.2.0");
  assert.equal(state.notes, "修复若干问题");
  const view = describeUpdateState(state, { desktop: true });
  assert.equal(view.action, "install");
  assert.match(view.label, /0\.2\.0/);
});

test("下载进度：按已下载字节计算百分比并夹在 0-100", () => {
  let state = updateReducer(initialUpdateState, { type: "check-found", version: "0.2.0" });
  state = updateReducer(state, { type: "install-started" });
  assert.equal(state.phase, "downloading");
  state = updateReducer(state, { type: "install-progress", downloadedBytes: 512, totalBytes: 2048 });
  assert.equal(progressPercent(state.progress), 25);
  state = updateReducer(state, { type: "install-progress", downloadedBytes: 4096, totalBytes: 2048 });
  assert.equal(progressPercent(state.progress), 100);
  assert.equal(progressPercent(undefined), null);
  assert.equal(progressPercent({ downloadedBytes: 10, totalBytes: 0 }), null);
});

test("下载完成进入待重启状态，失败进入错误状态", () => {
  let state = updateReducer(initialUpdateState, { type: "check-found", version: "0.2.0" });
  state = updateReducer(state, { type: "install-finished" });
  assert.equal(state.phase, "ready");
  assert.match(describeUpdateState(state, { desktop: true }).detail, /重新打开应用/);

  const failed = updateReducer(initialUpdateState, { type: "failed", message: "网络中断" });
  assert.equal(failed.phase, "error");
  const view = describeUpdateState(failed, { desktop: true });
  assert.equal(view.detail, "网络中断");
  assert.equal(view.action, "check");
});

test("浏览器版本只提示改用桌面版，不提供操作", () => {
  const view = describeUpdateState(initialUpdateState, { desktop: false });
  assert.equal(view.action, "none");
  assert.equal(view.busy, false);
  assert.match(view.detail, /桌面版/);
});

test("重置回到初始状态", () => {
  const state = updateReducer({ phase: "ready" }, { type: "reset" });
  assert.deepEqual(state, initialUpdateState);
});
