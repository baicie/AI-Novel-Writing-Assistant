import assert from "node:assert/strict";
import test from "node:test";

import {
  appError,
  asRecord,
  iso,
  ok,
  taskStatusForRun,
} from "../src/platform/desktopPrimitives.ts";

test("asRecord 把 JSON 字符串与对象都转成记录，其它输入给空对象", () => {
  assert.deepEqual(asRecord('{"a":1}'), { a: 1 });
  assert.deepEqual(asRecord({ a: 1 }), { a: 1 });
  assert.deepEqual(asRecord("not-json"), {});
  assert.deepEqual(asRecord("42"), {});
  assert.deepEqual(asRecord(null), {});
  assert.deepEqual(asRecord(undefined), {});
});

test("appError 保留机器码与可重试标记，缺失时给兜底文案", () => {
  assert.deepEqual(appError({ code: "AUTH_FAILED", message: "鉴权失败", retryable: true }), {
    code: "AUTH_FAILED",
    message: "鉴权失败",
    retryable: true,
  });
  assert.deepEqual(appError({ code: "TIMEOUT" }), {
    code: "TIMEOUT",
    message: "本地创作内核没有完成这次操作。",
    retryable: false,
  });
  assert.deepEqual(appError("boom"), {
    code: "INTEGRITY_ERROR",
    message: "本地创作内核没有完成这次操作。",
    retryable: false,
  });
});

test("运行状态映射覆盖暂停、成功、失败、取消与未知", () => {
  assert.equal(taskStatusForRun("running"), "running");
  assert.equal(taskStatusForRun("paused"), "waiting_approval");
  assert.equal(taskStatusForRun("succeeded"), "succeeded");
  assert.equal(taskStatusForRun("failed"), "failed");
  assert.equal(taskStatusForRun("cancelled"), "cancelled");
  assert.equal(taskStatusForRun("canceled"), "cancelled");
  assert.equal(taskStatusForRun("mystery"), "queued");
});

test("ok 与 iso 的形状稳定", () => {
  assert.deepEqual(ok({ value: 1 }), { success: true, data: { value: 1 } });
  assert.equal(iso(0), "1970-01-01T00:00:00.000Z");
});
