const path = require("node:path");
const { execFileSync } = require("node:child_process");
const { parseDevDesktopVersion } = require("./desktop-version.cjs");

const repoRoot = path.resolve(__dirname, "..");

function parseArgs(argv) {
  const options = {
    remote: "origin",
    dryRun: false,
    help: false,
    version: "",
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--help" || arg === "-h") {
      options.help = true;
      continue;
    }
    if (arg === "--dry-run") {
      options.dryRun = true;
      continue;
    }
    if (arg === "--remote" && argv[index + 1]) {
      options.remote = argv[index + 1].trim();
      index += 1;
      continue;
    }
    if (arg.startsWith("-")) {
      throw new Error(`Unknown argument: ${arg}`);
    }
    if (options.version) {
      throw new Error(`Unexpected extra version argument: ${arg}`);
    }
    options.version = arg.trim();
  }

  return options;
}

function printHelp() {
  console.log([
    "Usage: node scripts/trigger-desktop-dev-release.cjs [--dry-run] [--remote origin] X.Y.Z-dev.N",
    "",
    "Tags the current commit as vX.Y.Z-dev.N and pushes that tag to start the dev desktop workflow.",
    "desktop/package.json stays on the stable version. This does not publish a public vX.Y.Z release.",
  ].join("\n"));
}

function git(args, options = {}) {
  const output = execFileSync("git", args, {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: options.stdio ?? ["ignore", "pipe", "pipe"],
  });
  return typeof output === "string" ? output.trim() : "";
}

function gitOk(args) {
  try {
    git(args);
    return true;
  } catch (_error) {
    return false;
  }
}

function assertCleanWorkingTree() {
  const status = git(["status", "--porcelain"]);
  if (status) {
    throw new Error("Working tree is not clean. Commit the dev workflow before tagging a dev package.");
  }
}

function assertTagDoesNotExist(remote, tagName) {
  if (gitOk(["rev-parse", "--verify", "--quiet", `refs/tags/${tagName}`])) {
    throw new Error(`Local tag ${tagName} already exists.`);
  }
  const remoteTag = git(["ls-remote", "--tags", remote, tagName]);
  if (remoteTag) {
    throw new Error(`Remote tag ${tagName} already exists on ${remote}.`);
  }
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) {
    printHelp();
    return;
  }

  const version = parseDevDesktopVersion(options.version);
  const tagName = `v${version}`;
  const branch = git(["rev-parse", "--abbrev-ref", "HEAD"]);

  assertCleanWorkingTree();
  assertTagDoesNotExist(options.remote, tagName);

  console.log(`[desktop-dev] version=${version}`);
  console.log(`[desktop-dev] tag=${tagName}`);
  console.log(`[desktop-dev] branch=${branch}`);
  console.log(`[desktop-dev] remote=${options.remote}`);

  if (options.dryRun) {
    console.log("[desktop-dev] dry run passed; no tag or push was performed.");
    return;
  }

  git(["tag", "-a", tagName, "-m", `dev: ${tagName}`], { stdio: "inherit" });
  git(["push", options.remote, branch], { stdio: "inherit" });
  git(["push", options.remote, tagName], { stdio: "inherit" });
  console.log(`[desktop-dev] pushed ${tagName}; GitHub Actions will build the dev desktop package.`);
}

try {
  main();
} catch (error) {
  console.error(`[desktop-dev] ${error.message}`);
  process.exit(1);
}
