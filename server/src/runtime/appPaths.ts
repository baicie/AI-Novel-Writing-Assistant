import os from "node:os";
import path from "node:path";

export type AppRuntimeMode = "web" | "desktop" | "selfhost";

const APP_NAME = "AI-Novel-Writing-Assistant-v2";
const SERVER_ROOT = path.resolve(__dirname, "..", "..");
const WORKSPACE_ROOT = path.resolve(SERVER_ROOT, "..");

function resolveConfiguredAppDataDir(): string | null {
  const configuredDir = process.env.AI_NOVEL_APP_DATA_DIR?.trim();
  return configuredDir ? path.resolve(configuredDir) : null;
}

function resolveDefaultDesktopAppDataDir(): string {
  const localAppData = process.env.LOCALAPPDATA?.trim();
  if (localAppData) {
    return path.join(localAppData, APP_NAME);
  }

  const appData = process.env.APPDATA?.trim();
  if (appData) {
    return path.join(appData, APP_NAME);
  }

  if (process.platform === "darwin") {
    return path.join(os.homedir(), "Library", "Application Support", APP_NAME);
  }

  return path.join(os.homedir(), `.${APP_NAME}`);
}

export function resolveAppRuntimeMode(): AppRuntimeMode {
  const configuredMode = process.env.AI_NOVEL_RUNTIME?.trim().toLowerCase();
  return configuredMode === "desktop" || configuredMode === "selfhost" ? configuredMode : "web";
}

// Desktop and self-hosted servers own one app data directory for the database, logs and generated
// images, and upgrade their SQLite database on startup. Electron-only behavior must not key on this.
export function isSelfContainedRuntime(mode: AppRuntimeMode = resolveAppRuntimeMode()): boolean {
  return mode !== "web";
}

export function resolveAppDataRoot(): string {
  return resolveConfiguredAppDataDir() ?? resolveDefaultDesktopAppDataDir();
}

export function resolveServerRoot(): string {
  return SERVER_ROOT;
}

export function resolveWorkspaceRoot(): string {
  return WORKSPACE_ROOT;
}

export function resolveDataRoot(): string {
  return isSelfContainedRuntime()
    ? path.join(resolveAppDataRoot(), "data")
    : resolveServerRoot();
}

export function resolveLogsRoot(): string {
  return isSelfContainedRuntime()
    ? path.join(resolveAppDataRoot(), "logs")
    : path.join(resolveWorkspaceRoot(), ".logs");
}

export function resolveGeneratedImagesRoot(): string {
  return isSelfContainedRuntime()
    ? path.join(resolveAppDataRoot(), "storage", "generated-images")
    : path.join(resolveServerRoot(), "storage", "generated-images");
}

export function resolveDatabaseFilePath(filePath: string): string {
  const baseDir = resolveDataRoot();
  return path.isAbsolute(filePath) ? filePath : path.resolve(baseDir, filePath);
}
