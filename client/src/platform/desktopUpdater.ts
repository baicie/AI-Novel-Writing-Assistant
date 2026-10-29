import { isTauri } from "@tauri-apps/api/core";
import type { UpdateProgress } from "@/pages/settings/updateCheckState";

export interface AvailableUpdate {
  version: string;
  notes?: string;
}

type UpdaterModule = typeof import("@tauri-apps/plugin-updater");

export function isUpdateCheckAvailable(): boolean {
  return typeof window !== "undefined" && isTauri();
}

async function loadUpdater(): Promise<UpdaterModule | null> {
  if (!isUpdateCheckAvailable()) return null;
  try {
    return await import("@tauri-apps/plugin-updater");
  } catch {
    // 桌面端未打包更新插件时（例如本地 dev 构建），这里退化为"不支持更新"。
    return null;
  }
}

export async function readCurrentVersion(): Promise<string | null> {
  if (!isUpdateCheckAvailable()) return null;
  try {
    const { getVersion } = await import("@tauri-apps/api/app");
    return await getVersion();
  } catch {
    return null;
  }
}

let pending: Awaited<ReturnType<UpdaterModule["check"]>> | null = null;

export async function checkForUpdate(): Promise<AvailableUpdate | null> {
  const updater = await loadUpdater();
  if (!updater) return null;
  const update = await updater.check();
  pending = update;
  if (!update) return null;
  return { version: update.version, notes: update.body ?? undefined };
}

export async function installAvailableUpdate(
  onProgress: (progress: UpdateProgress) => void,
): Promise<boolean> {
  if (!pending) return false;
  let downloadedBytes = 0;
  let totalBytes = 0;
  await pending.downloadAndInstall((event) => {
    if (event.event === "Started") {
      totalBytes = event.data.contentLength ?? 0;
      downloadedBytes = 0;
      onProgress({ downloadedBytes, totalBytes });
      return;
    }
    if (event.event === "Progress") {
      downloadedBytes += event.data.chunkLength ?? 0;
      onProgress({ downloadedBytes, totalBytes });
      return;
    }
    onProgress({ downloadedBytes: totalBytes, totalBytes });
  });
  pending = null;
  return true;
}

export function resetPendingUpdate() {
  pending = null;
}
