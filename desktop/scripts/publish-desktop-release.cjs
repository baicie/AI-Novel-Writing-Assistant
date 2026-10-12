const { execFileSync } = require("node:child_process");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "..", "..");
const platformTargetArgs = {
  win: ["--win", "--x64"],
  mac: ["--mac", "--arm64", "--x64"],
};

function resolvePlatform(argv) {
  const flagIndex = argv.indexOf("--platform");
  const platform = flagIndex >= 0 ? argv[flagIndex + 1] : "win";
  if (!platformTargetArgs[platform]) {
    throw new Error(`Unsupported desktop release platform: ${platform}`);
  }
  return platform;
}

try {
  const platform = resolvePlatform(process.argv.slice(2));
  execFileSync(process.execPath, [
    path.join("desktop", "scripts", "run-electron-builder.cjs"),
    ...platformTargetArgs[platform],
    "--publish",
    "always",
  ], {
    cwd: repoRoot,
    stdio: "inherit",
    env: {
      ...process.env,
      AI_NOVEL_RELEASE_CHANNEL: "release",
    },
  });
} catch (error) {
  console.error("[publish:desktop:release] failed.", error);
  process.exit(1);
}
