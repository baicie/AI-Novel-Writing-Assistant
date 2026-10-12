const test = require("node:test");
const assert = require("node:assert/strict");
const os = require("node:os");
const path = require("node:path");

const appPaths = require("../dist/runtime/appPaths.js");

const runtimeEnvKeys = ["AI_NOVEL_RUNTIME", "AI_NOVEL_APP_DATA_DIR"];

function withRuntimeEnv(env, run) {
  const previous = Object.fromEntries(runtimeEnvKeys.map((key) => [key, process.env[key]]));
  for (const key of runtimeEnvKeys) {
    if (env[key] === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = env[key];
    }
  }

  try {
    return run();
  } finally {
    for (const key of runtimeEnvKeys) {
      if (previous[key] === undefined) {
        delete process.env[key];
      } else {
        process.env[key] = previous[key];
      }
    }
  }
}

test("self-hosted runtime keeps the database, logs and generated images under the app data directory", () => {
  const appDataDir = path.resolve(os.tmpdir(), "ai-novel-selfhost-data");

  withRuntimeEnv({ AI_NOVEL_RUNTIME: "selfhost", AI_NOVEL_APP_DATA_DIR: appDataDir }, () => {
    assert.equal(appPaths.resolveAppRuntimeMode(), "selfhost");
    assert.equal(appPaths.isSelfContainedRuntime(), true);
    assert.equal(appPaths.resolveDatabaseFilePath("./ai-novel.db"), path.join(appDataDir, "data", "ai-novel.db"));
    assert.equal(appPaths.resolveLogsRoot(), path.join(appDataDir, "logs"));
    assert.equal(appPaths.resolveGeneratedImagesRoot(), path.join(appDataDir, "storage", "generated-images"));
  });
});

test("web runtime keeps the source checkout layout even when an app data directory is configured", () => {
  withRuntimeEnv({ AI_NOVEL_RUNTIME: undefined, AI_NOVEL_APP_DATA_DIR: path.resolve(os.tmpdir(), "ignored") }, () => {
    assert.equal(appPaths.resolveAppRuntimeMode(), "web");
    assert.equal(appPaths.isSelfContainedRuntime(), false);
    assert.equal(appPaths.resolveDatabaseFilePath("./dev.db"), path.join(appPaths.resolveServerRoot(), "dev.db"));
    assert.equal(appPaths.resolveLogsRoot(), path.join(appPaths.resolveWorkspaceRoot(), ".logs"));
    assert.equal(
      appPaths.resolveGeneratedImagesRoot(),
      path.join(appPaths.resolveServerRoot(), "storage", "generated-images"),
    );
  });
});

test("runtime mode accepts known values case-insensitively and falls back to web", () => {
  withRuntimeEnv({ AI_NOVEL_RUNTIME: " SelfHost ", AI_NOVEL_APP_DATA_DIR: undefined }, () => {
    assert.equal(appPaths.resolveAppRuntimeMode(), "selfhost");
  });
  withRuntimeEnv({ AI_NOVEL_RUNTIME: "docker", AI_NOVEL_APP_DATA_DIR: undefined }, () => {
    assert.equal(appPaths.resolveAppRuntimeMode(), "web");
    assert.equal(appPaths.isSelfContainedRuntime(), false);
  });
});
