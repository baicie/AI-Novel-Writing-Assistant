import assert from "node:assert/strict";
import test from "node:test";

import {
  formatDiscoveryError,
  needsPlaintextHttpPermission,
} from "../src/platform/desktopModelRules.ts";

test("明文 http 需要显式允许，https 不需要", () => {
  assert.equal(needsPlaintextHttpPermission("http://82.156.234.84:7863/v1"), true);
  assert.equal(needsPlaintextHttpPermission("http://127.0.0.1:11434/v1"), true);
  assert.equal(needsPlaintextHttpPermission("  HTTP://api.example.com/v1  "), true);
  assert.equal(needsPlaintextHttpPermission("https://api.deepseek.com/v1"), false);
  assert.equal(needsPlaintextHttpPermission(""), false);
});

test("发现失败时优先显示原因与错误码", () => {
  assert.equal(
    formatDiscoveryError({ code: "AUTH_FAILED", message: "鉴权失败，请检查 API Key" }),
    "鉴权失败，请检查 API Key（AUTH_FAILED）",
  );
  assert.equal(formatDiscoveryError({ message: "只有原因" }), "只有原因");
  assert.equal(formatDiscoveryError(new Error("普通异常")), "普通异常");
  assert.equal(
    formatDiscoveryError(null),
    "获取模型列表失败，可以手动填写模型名称。",
  );
});
