import { useMemo } from "react";

export type CreationMode = "local" | "cloud";

const STORAGE_KEY_PREFIX = "local-creation-mode:";

/**
 * 读取和设置本地创作模式的 hook。
 * 模式选择按小说存储，避免全局配置混乱。
 */
export function useLocalCreationMode(novelId: string) {
  const storageKey = useMemo(() => `${STORAGE_KEY_PREFIX}${novelId}`, [novelId]);

  const getMode = (): CreationMode => {
    try {
      const stored = localStorage.getItem(storageKey);
      return (stored === "local" || stored === "cloud") ? stored : "cloud";
    } catch (e) {
      console.warn("读取本地创作模式失败，默认为云创作", e);
      return "cloud";
    }
  };

  const setMode = (mode: CreationMode) => {
    try {
      localStorage.setItem(storageKey, mode);
    } catch (e) {
      console.error("保存本地创作模式失败", e);
    }
  };

  const mode = useMemo(() => getMode(), [storageKey]);

  return {
    mode,
    setMode,
    isLocalMode: mode === "local",
    isCloudMode: mode === "cloud",
  };
}
