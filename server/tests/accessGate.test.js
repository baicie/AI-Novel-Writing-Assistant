const test = require("node:test");
const assert = require("node:assert/strict");
const http = require("node:http");
const { createApp } = require("../dist/app.js");
const { resetAccessAttemptLimiter } = require("../dist/platform/access/accessSession.js");

function listen(server) {
  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => resolve(server.address().port));
  });
}

function readCookie(response) {
  const header = response.headers.get("set-cookie") ?? "";
  const match = /ai_novel_access=([^;]+)/.exec(header);
  return match ? decodeURIComponent(match[1]) : "";
}

test("access password protects web APIs and leaves health and signed callbacks open", async () => {
  const previous = process.env.ACCESS_PASSWORD;
  process.env.ACCESS_PASSWORD = "shelf-secret";
  resetAccessAttemptLimiter();
  const server = http.createServer(createApp());
  const port = await listen(server);
  const base = `http://127.0.0.1:${port}`;
  try {
    const openStatus = await fetch(`${base}/api/access/status`);
    assert.equal(openStatus.status, 200);
    assert.deepEqual((await openStatus.json()).data, { required: true, authenticated: false });

    const blocked = await fetch(`${base}/api/novels`);
    assert.equal(blocked.status, 401);
    assert.equal((await blocked.json()).error, "ACCESS_REQUIRED");

    const health = await fetch(`${base}/api/health`);
    assert.equal(health.status, 200);

    const callback = await fetch(`${base}/api/auto-director/channel-callbacks/wecom/execute`);
    assert.notEqual(callback.status, 401);

    const rejected = await fetch(`${base}/api/access/login`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ password: "wrong" }),
    });
    assert.equal(rejected.status, 401);

    const accepted = await fetch(`${base}/api/access/login`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ password: "shelf-secret" }),
    });
    assert.equal(accepted.status, 200);
    const cookie = readCookie(accepted);
    assert.match(accepted.headers.get("set-cookie") ?? "", /HttpOnly/i);
    const session = await fetch(`${base}/api/access/status`, {
      headers: { cookie: `ai_novel_access=${cookie}` },
    });
    assert.equal((await session.json()).data.authenticated, true);

    const loggedOut = await fetch(`${base}/api/access/logout`, { method: "POST" });
    assert.match(loggedOut.headers.get("set-cookie") ?? "", /Max-Age=0/i);
  } finally {
    resetAccessAttemptLimiter();
    if (previous === undefined) {
      delete process.env.ACCESS_PASSWORD;
    } else {
      process.env.ACCESS_PASSWORD = previous;
    }
    await new Promise((resolve) => server.close(resolve));
  }
});
