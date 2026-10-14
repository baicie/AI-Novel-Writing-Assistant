const fs = require("node:fs");
const path = require("node:path");
const { execFileSync, execSync } = require("node:child_process");

const repoRoot = path.resolve(__dirname, "..", "..");
const stageRoot = path.join(repoRoot, "target", "tauri-sidecar");
const serverStageDir = path.join(stageRoot, "server");
const binStageDir = path.join(stageRoot, "bin");
const sidecarEntryRelativePath = path.join("dist", "app", "desktop", "sidecarEntry.js");

const TARGET_PLATFORMS = {
  "aarch64-apple-darwin": { platform: "darwin", arch: "arm64" },
  "x86_64-apple-darwin": { platform: "darwin", arch: "x64" },
  "aarch64-pc-windows-msvc": { platform: "win32", arch: "arm64" },
  "x86_64-pc-windows-msvc": { platform: "win32", arch: "x64" },
  "aarch64-unknown-linux-gnu": { platform: "linux", arch: "arm64" },
  "x86_64-unknown-linux-gnu": { platform: "linux", arch: "x64" },
};

function log(message) {
  console.log(`[stage:tauri-sidecar] ${message}`);
}

function assertExists(targetPath, description) {
  if (!fs.existsSync(targetPath)) {
    throw new Error(`Expected ${description} at ${targetPath}, but it was not found.`);
  }
}

function resolveTargetTriple() {
  const fromTauri = process.env.TAURI_ENV_TARGET_TRIPLE?.trim();
  if (fromTauri) {
    return fromTauri;
  }
  const rustcInfo = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
  const hostLine = rustcInfo.split(/\r?\n/).find((line) => line.startsWith("host:"));
  if (!hostLine) {
    throw new Error("Unable to resolve the Rust host target triple from `rustc -vV`.");
  }
  return hostLine.slice("host:".length).trim();
}

function describeNodeBinary(nodeBinary) {
  const output = execFileSync(
    nodeBinary,
    ["-p", "JSON.stringify({ platform: process.platform, arch: process.arch, version: process.version, modules: process.versions.modules })"],
    { encoding: "utf8" },
  );
  return JSON.parse(output.trim());
}

function resolveNodeBinary(targetTriple) {
  const target = TARGET_PLATFORMS[targetTriple];
  if (!target) {
    throw new Error(`Target ${targetTriple} is not supported by the bundled local server.`);
  }

  const nodeBinary = path.resolve(process.env.AI_NOVEL_TAURI_NODE_BINARY?.trim() || process.execPath);
  assertExists(nodeBinary, "Node.js runtime for the sidecar");
  const nodeInfo = describeNodeBinary(nodeBinary);

  // Native modules (better-sqlite3, sharp) are copied from this workspace install, so the
  // bundled runtime, the workspace install and the build target must all share one platform.
  if (process.platform !== target.platform || process.arch !== target.arch) {
    throw new Error(
      `Target ${targetTriple} needs ${target.platform}/${target.arch} native modules, but this workspace was installed on ${process.platform}/${process.arch}. Package this target on a matching machine.`,
    );
  }
  if (nodeInfo.platform !== target.platform || nodeInfo.arch !== target.arch) {
    throw new Error(
      `Node.js runtime ${nodeBinary} is ${nodeInfo.platform}/${nodeInfo.arch}, but target ${targetTriple} needs ${target.platform}/${target.arch}.`,
    );
  }

  return { nodeBinary, nodeInfo, target };
}

function deployServer() {
  fs.rmSync(serverStageDir, { recursive: true, force: true });
  fs.mkdirSync(stageRoot, { recursive: true });
  execSync(
    `pnpm --filter @ai-novel/server deploy --prod --config.node-linker=hoisted "${serverStageDir}"`,
    { cwd: repoRoot, stdio: "inherit", env: process.env },
  );
  assertExists(path.join(serverStageDir, sidecarEntryRelativePath), "bundled sidecar entry");
  assertExists(path.join(serverStageDir, "src", "prisma", "migrations.sqlite"), "bundled SQLite migrations");
}

function syncGeneratedPrismaClient() {
  const stagedClientPackageJson = path.join(serverStageDir, "node_modules", "@prisma", "client", "package.json");
  assertExists(stagedClientPackageJson, "staged @prisma/client");
  const clientVersion = JSON.parse(fs.readFileSync(stagedClientPackageJson, "utf8")).version;

  const virtualStoreDir = path.join(repoRoot, "node_modules", ".pnpm");
  const storeEntryPrefix = `@prisma+client@${clientVersion}`;
  const generatedDir = fs
    .readdirSync(virtualStoreDir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory()
      && (entry.name === storeEntryPrefix || entry.name.startsWith(`${storeEntryPrefix}_`)))
    .map((entry) => path.join(virtualStoreDir, entry.name, "node_modules", ".prisma"))
    .find((candidate) => fs.existsSync(path.join(candidate, "client", "default.js")));
  if (!generatedDir) {
    throw new Error(`No generated Prisma client for @prisma/client@${clientVersion} was found. Run \`pnpm --filter @ai-novel/server prisma:generate\` first.`);
  }

  const generatedSchema = fs.readFileSync(path.join(generatedDir, "client", "schema.prisma"), "utf8");
  if (!/datasource\s+\w+\s*\{[^}]*provider\s*=\s*"sqlite"/.test(generatedSchema)) {
    throw new Error("The generated Prisma client targets a non-SQLite database. Regenerate it without DATABASE_URL pointing at PostgreSQL.");
  }

  fs.cpSync(generatedDir, path.join(serverStageDir, "node_modules", ".prisma"), { recursive: true });
}

function removeBinLinkDirectories(directory) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    if (!entry.isDirectory()) {
      continue;
    }
    const fullPath = path.join(directory, entry.name);
    if (entry.name === ".bin") {
      fs.rmSync(fullPath, { recursive: true, force: true });
      continue;
    }
    removeBinLinkDirectories(fullPath);
  }
}

function findSymlinks(directory, found = []) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const fullPath = path.join(directory, entry.name);
    if (entry.isSymbolicLink()) {
      found.push(fullPath);
    } else if (entry.isDirectory()) {
      findSymlinks(fullPath, found);
    }
  }
  return found;
}

function assertNoSymlinks() {
  // Tauri resource bundling skips symlinked directories, which would silently drop packages.
  const symlinks = findSymlinks(serverStageDir);
  if (symlinks.length > 0) {
    throw new Error(`The staged server still contains symlinks that Tauri cannot bundle:\n${symlinks.slice(0, 10).join("\n")}`);
  }
}

function stageNodeBinary(nodeBinary, targetTriple, target) {
  fs.rmSync(binStageDir, { recursive: true, force: true });
  fs.mkdirSync(binStageDir, { recursive: true });
  const extension = target.platform === "win32" ? ".exe" : "";
  const stagedBinary = path.join(binStageDir, `node-${targetTriple}${extension}`);
  fs.copyFileSync(nodeBinary, stagedBinary);
  fs.chmodSync(stagedBinary, 0o755);
  return stagedBinary;
}

function smokeTestStagedServer(stagedBinary) {
  const script = [
    "const Database = require('better-sqlite3');",
    "new Database(':memory:').prepare('select 1').get();",
    "require('sharp');",
    "require('@prisma/client');",
    `require.resolve('./${sidecarEntryRelativePath.split(path.sep).join("/")}');`,
  ].join(" ");
  execFileSync(stagedBinary, ["-e", script], { cwd: serverStageDir, stdio: "inherit" });
}

function directorySizeBytes(directory) {
  let total = 0;
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const fullPath = path.join(directory, entry.name);
    total += entry.isDirectory() ? directorySizeBytes(fullPath) : fs.statSync(fullPath).size;
  }
  return total;
}

function main() {
  const targetTriple = resolveTargetTriple();
  const { nodeBinary, nodeInfo, target } = resolveNodeBinary(targetTriple);
  log(`target ${targetTriple}, Node.js ${nodeInfo.version} (ABI ${nodeInfo.modules}) from ${nodeBinary}`);

  deployServer();
  syncGeneratedPrismaClient();
  removeBinLinkDirectories(serverStageDir);
  assertNoSymlinks();
  const stagedBinary = stageNodeBinary(nodeBinary, targetTriple, target);
  smokeTestStagedServer(stagedBinary);

  log(`server staged at ${serverStageDir} (${(directorySizeBytes(serverStageDir) / 1024 / 1024).toFixed(0)} MB)`);
  log(`runtime staged at ${stagedBinary}`);
}

try {
  main();
} catch (error) {
  console.error("[stage:tauri-sidecar] failed.", error instanceof Error ? error.message : error);
  process.exit(1);
}
