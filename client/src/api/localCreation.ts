import * as desktopTransport from "@/platform/desktopTransport";

/**
 * 本地创作模式的 Tauri 命令集合。
 * 通过这一层可以统一处理本地 vs 云 API 的差异。
 */

export interface ModelSetupView {
  endpoint: string;
  model: string;
  allowLocalhostHttp: boolean;
  hasApiKey: boolean;
}

export async function getLocalModelSetup(): Promise<ModelSetupView | null> {
  try {
    return await desktopTransport.getModelSetup();
  } catch (error) {
    console.error("读取模型配置失败", error);
    throw error;
  }
}

export async function saveLocalModelSetup(input: {
  endpoint: string;
  model: string;
  apiKey: string;
  allowLocalhostHttp?: boolean;
}): Promise<ModelSetupView> {
  try {
    return await desktopTransport.saveModelSetup({
      endpoint: input.endpoint,
      model: input.model,
      api_key: input.apiKey,
      allow_localhost_http: input.allowLocalhostHttp ?? false,
    });
  } catch (error) {
    console.error("保存模型配置失败", error);
    throw error;
  }
}

export async function listLocalChapters(novelId: string): Promise<desktopTransport.ChapterSnapshot[]> {
  try {
    return await desktopTransport.listChapters(novelId);
  } catch (error) {
    console.error(`读取本地章节列表失败（novelId: ${novelId}）`, error);
    throw error;
  }
}

export async function cancelLocalGeneration(runId: string): Promise<void> {
  try {
    await desktopTransport.cancelGeneration(runId);
  } catch (error) {
    console.error(`取消本地生成失败（runId: ${runId}）`, error);
    throw error;
  }
}

export async function startLocalGeneration(input: desktopTransport.StartGeneration): Promise<desktopTransport.CommandOutcome> {
  try {
    return await desktopTransport.openRun(input);
  } catch (error) {
    console.error("启动本地生成失败", error);
    throw error;
  }
}

export async function subscribeLocalGeneration(
  runId: string,
  onEvent: (event: desktopTransport.GenerationEvent) => void,
): Promise<void> {
  try {
    await desktopTransport.subscribeRun({ runId }, onEvent);
  } catch (error) {
    console.error(`订阅本地生成事件失败（runId: ${runId}）`, error);
    throw error;
  }
}

/**
 * 生成路由 - 根据模式自动选择本地或云 API。
 * 这是章节编辑器集成的关键入口。
 */

export interface ChapterGenerationMode {
  isLocal: boolean;
  novelId: string;
}

/**
 * 检查是否应该使用本地生成。
 * 仅当 localStorage 中明确设置为本地模式时返回 true。
 */
export function shouldUseLocalGeneration(novelId: string): boolean {
  try {
    const mode = localStorage.getItem(`local-creation-mode:${novelId}`);
    return mode === "local";
  } catch (e) {
    console.warn("检查本地模式设置失败，默认使用云模式", e);
    return false;
  }
}
