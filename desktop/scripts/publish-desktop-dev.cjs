const { execFileSync } = require("node:child_process");
const path = require("node:path");
const { parseDevDesktopVersion } = require("../../scripts/desktop-version.cjs");

const repoRoot = path.resolve(__dirname, "..", "..");
const platformTargetArgs = {
  win: ["--win", "--x64"],
  mac: ["--mac", "--arm64", "--x64"],
};

function resolvePlatform(argv) {
  const flagIndex = argv.indexOf("--platform");
  const platform = flagIndex >= 0 ? argv[flagIndex + 1] : "win";
  if (!platformTargetArgs[platform]) {
    throw new Error(`Unsupported desktop dev platform: ${platform}`);
  }
  return platform;
}

try {
  const version = parseDevDesktopVersion(process.env.AI_NOVEL_DESKTOP_VERSION);
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
      AI_NOVEL_RELEASE_CHANNEL: "dev",
      AI_NOVEL_DESKTOP_VERSION: version,
    },
  });
} catch (error) {
  console.error("[publish:desktop:dev] failed.", error);
  process.exit(1);
}
