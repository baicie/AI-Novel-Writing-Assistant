const fs = require("node:fs");
const { execFileSync } = require("node:child_process");
const { createRequire } = require("node:module");
const path = require("node:path");

const desktopDir = path.resolve(__dirname, "..");
const repoRoot = path.resolve(desktopDir, "..");
const electronBuilderPackageJson = require.resolve("electron-builder/package.json", { paths: [desktopDir] });
const electronBuilderRequire = createRequire(electronBuilderPackageJson);
const asar = electronBuilderRequire("@electron/asar");
const buildDir = path.join(desktopDir, "build");
const distDir = path.join(buildDir, "dist");
const appDir = path.join(buildDir, "app");
const productName = "Biz Novel Studio";
const macUnpackedDirNames = {
  arm64: "mac-arm64",
  x64: "mac",
};
const builderWindowIcon = path.join(desktopDir, "builder", "app-icon.ico");
const appPackageJsonPath = path.join(appDir, "package.json");
const stagedServerEntry = path.join(appDir, "node_modules", "@ai-novel", "server", "dist", "app.js");
const stagedPrismaRuntimeEntry = path.join(appDir, "node_modules", ".prisma", "client", "default.js");
const stagedGeneratedPrismaClientEntry = path.join(
  appDir,
  "node_modules",
  ".pnpm",
  "node_modules",
  "@prisma",
  "client",
  "generated-client",
  "default.js",
);
const sourceServerSqliteMigrationsDir = path.join(repoRoot, "server", "src", "prisma", "migrations.sqlite");
const stagedServerSqliteMigrationsDir = path.join(appDir, "node_modules", "@ai-novel", "server", "src", "prisma", "migrations.sqlite");
const stagedAppUpdateConfig = path.join(buildDir, "resources", "app-update.yml");
const stagedClientIndex = path.join(buildDir, "resources", "client", "dist", "index.html");
const stagedRuntimeFile = path.join(appDir, "dist", "runtime", "server.js");

function assertExists(targetPath, description) {
  if (!fs.existsSync(targetPath)) {
    throw new Error(`Missing ${description}: ${targetPath}`);
  }
}

function assertNotExists(targetPath, description) {
  if (fs.existsSync(targetPath)) {
    throw new Error(`Unexpected ${description}: ${targetPath}`);
  }
}

function assertResolvesWithinDirectory(targetPath, expectedParentDir, description) {
  const resolvedPath = fs.realpathSync(targetPath);
  const normalizedParentDir = path.resolve(expectedParentDir);
  if (!resolvedPath.startsWith(normalizedParentDir)) {
    throw new Error(`${description} must resolve inside ${normalizedParentDir}, but resolved to ${resolvedPath}.`);
  }
}

function assertSomeMatch(entries, pattern, description) {
  const matchedEntry = entries.find((entry) => pattern.test(entry));
  if (!matchedEntry) {
    throw new Error(`Packaged app archive is missing ${description}.`);
  }
}

function listMigrationNames(migrationsDir) {
  return fs.readdirSync(migrationsDir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && fs.existsSync(path.join(migrationsDir, entry.name, "migration.sql")))
    .map((entry) => entry.name)
    .sort((left, right) => left.localeCompare(right));
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function parseArgs(argv) {
  const options = { platform: "win", arches: ["x64"] };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--platform") {
      options.platform = argv[index + 1];
      index += 1;
      continue;
    }
    if (arg === "--arch") {
      options.arches = String(argv[index + 1] || "").split(",").map((value) => value.trim()).filter(Boolean);
      index += 1;
      continue;
    }
    throw new Error(`Unknown argument: ${arg}`);
  }

  if (options.platform !== "win" && options.platform !== "mac") {
    throw new Error(`Unsupported platform: ${options.platform}`);
  }
  if (options.platform === "mac" && !argv.includes("--arch")) {
    options.arches = ["arm64", "x64"];
  }
  for (const arch of options.arches) {
    if (options.platform === "mac" ? !macUnpackedDirNames[arch] : arch !== "x64") {
      throw new Error(`Unsupported ${options.platform} architecture: ${arch}`);
    }
  }
  return options;
}

function resolvePackagedTargets(options) {
  if (options.platform === "win") {
    return [{
      label: "win-x64",
      arch: "x64",
      resourcesDir: path.join(distDir, "win-unpacked", "resources"),
      executablePath: null,
    }];
  }

  return options.arches.map((arch) => {
    const contentsDir = path.join(distDir, macUnpackedDirNames[arch], `${productName}.app`, "Contents");
    return {
      label: `mac-${arch}`,
      arch,
      resourcesDir: path.join(contentsDir, "Resources"),
      executablePath: path.join(contentsDir, "MacOS", productName),
    };
  });
}

function findFilesByName(rootDir, fileName) {
  if (!fs.existsSync(rootDir)) {
    return [];
  }
  const matches = [];
  for (const entry of fs.readdirSync(rootDir, { withFileTypes: true })) {
    const entryPath = path.join(rootDir, entry.name);
    if (entry.isDirectory()) {
      matches.push(...findFilesByName(entryPath, fileName));
    } else if (entry.name === fileName) {
      matches.push(entryPath);
    }
  }
  return matches;
}

function assertMachOArch(binaryPath, expectedArch, description) {
  const archs = execFileSync("lipo", ["-archs", binaryPath], { encoding: "utf8" }).trim().split(/\s+/);
  const expectedLipoArch = expectedArch === "x64" ? "x86_64" : expectedArch;
  if (!archs.includes(expectedLipoArch)) {
    throw new Error(`${description} is built for [${archs.join(", ")}], expected ${expectedLipoArch}: ${binaryPath}`);
  }
}

function verifyMacBinaries(target) {
  assertExists(target.executablePath, `${target.label} app executable`);
  assertMachOArch(target.executablePath, target.arch, `${target.label} app executable`);

  const nativeSqliteBinaries = findFilesByName(path.join(target.resourcesDir, "app.asar.unpacked"), "better_sqlite3.node");
  if (nativeSqliteBinaries.length === 0) {
    throw new Error(`${target.label} package is missing the unpacked better-sqlite3 native module.`);
  }
  for (const binaryPath of nativeSqliteBinaries) {
    assertMachOArch(binaryPath, target.arch, `${target.label} better-sqlite3 native module`);
  }
}

function verifyPackagedTarget(target, sourceSqliteMigrations) {
  const unpackedClientIndex = path.join(target.resourcesDir, "client", "dist", "index.html");
  const unpackedAppArchive = path.join(target.resourcesDir, "app.asar");
  const unpackedWindowIcon = path.join(target.resourcesDir, "icons", "app-icon.ico");
  const unpackedUpdateConfig = path.join(target.resourcesDir, "app-update.yml");

  assertExists(unpackedClientIndex, `${target.label} packaged renderer index`);
  assertExists(unpackedAppArchive, `${target.label} packaged app archive`);
  assertExists(unpackedWindowIcon, `${target.label} packaged desktop window icon`);
  assertExists(unpackedUpdateConfig, `${target.label} packaged updater feed configuration`);

  const packagedFiles = new Set(asar.listPackage(unpackedAppArchive).map((entry) => entry.replace(/^[\\/]/, "").replace(/\\/g, "/")));
  const packagedEntries = Array.from(packagedFiles);
  assertSomeMatch(
    packagedEntries,
    /^dist\/runtime\/server\.js$/,
    "desktop runtime server bundle inside app.asar",
  );
  assertSomeMatch(
    packagedEntries,
    /^node_modules\/(?:\.pnpm\/[^/]+\/node_modules\/)?@ai-novel\/server\/dist\/app\.js$/,
    "bundled server entry inside app.asar",
  );
  for (const migrationName of sourceSqliteMigrations) {
    assertSomeMatch(
      packagedEntries,
      new RegExp(`^node_modules/(?:\\.pnpm/[^/]+/node_modules/)?@ai-novel/server/src/prisma/migrations\\.sqlite/${escapeRegExp(migrationName)}/migration\\.sql$`),
      `SQLite migration ${migrationName} inside app.asar`,
    );
  }
  assertSomeMatch(
    packagedEntries,
    /^node_modules\/(?:\.pnpm\/[^/]+\/node_modules\/)?@prisma\/client\/generated-client\/default\.js$/,
    "embedded generated Prisma client inside app.asar",
  );
  assertSomeMatch(
    packagedEntries,
    /^node_modules\/(?:\.pnpm\/[^/]+\/node_modules\/)?@prisma\/client\/default\.js$/,
    "packaged Prisma client entrypoint inside app.asar",
  );

  if (target.executablePath) {
    verifyMacBinaries(target);
  }

  console.log(`[verify:desktop-package] ${target.label} package inspected at ${target.resourcesDir}`);
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  assertExists(appPackageJsonPath, "staged desktop package.json");
  assertExists(builderWindowIcon, "builder desktop window icon");
  assertExists(stagedAppUpdateConfig, "staged updater feed configuration");
  assertExists(stagedClientIndex, "staged renderer index");
  assertExists(stagedRuntimeFile, "desktop runtime server bundle");
  assertExists(sourceServerSqliteMigrationsDir, "source SQLite migrations");
  assertExists(stagedServerSqliteMigrationsDir, "staged SQLite migrations");
  assertNotExists(path.join(appDir, "src"), "desktop source directory inside staged app");
  assertNotExists(path.join(appDir, "node_modules", "electron"), "Electron runtime inside staged app node_modules");
  assertResolvesWithinDirectory(
    path.join(appDir, "node_modules", "@ai-novel", "server"),
    appDir,
    "Staged server package",
  );

  const appPackageJson = JSON.parse(fs.readFileSync(appPackageJsonPath, "utf8"));
  if (appPackageJson.dependencies?.electron) {
    throw new Error("Electron must not be bundled as an application dependency in the staged app.");
  }

  const runtimeSource = fs.readFileSync(stagedRuntimeFile, "utf8");
  if (runtimeSource.includes("pnpm --filter @ai-novel/server start")) {
    throw new Error("Packaged desktop runtime still references pnpm-based server startup.");
  }
  const stagedClientIndexSource = fs.readFileSync(stagedClientIndex, "utf8");
  if (stagedClientIndexSource.includes('src="/assets/') || stagedClientIndexSource.includes('href="/assets/')) {
    throw new Error("Packaged desktop renderer still references absolute /assets paths.");
  }
  const updaterConfigSource = fs.readFileSync(stagedAppUpdateConfig, "utf8");
  if (!updaterConfigSource.includes("provider: github")) {
    throw new Error("Desktop updater feed configuration is missing the GitHub provider.");
  }

  const sourceSqliteMigrations = listMigrationNames(sourceServerSqliteMigrationsDir);
  const stagedSqliteMigrations = listMigrationNames(stagedServerSqliteMigrationsDir);
  if (JSON.stringify(stagedSqliteMigrations) !== JSON.stringify(sourceSqliteMigrations)) {
    throw new Error("Staged SQLite migrations do not match the source migration set.");
  }

  for (const target of resolvePackagedTargets(options)) {
    verifyPackagedTarget(target, sourceSqliteMigrations);
  }

  console.log("[verify:desktop-package] staged package layout looks valid.");
}

try {
  main();
} catch (error) {
  console.error("[verify:desktop-package] failed.", error);
  process.exit(1);
}
