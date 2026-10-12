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

/**
 * 事件处理器 - 处理本地生成事件流
 * 累积文本并识别候选方案
 */
function createEventProcessor() {
  const textBuffer: Record<string, string> = {};
  const events: unknown[] = [];

  return {
    process(event: any) {
      events.push(event);

      if (event.kind === "token_delta" && event.stepId && event.text) {
        // 累积 token 为完整文本
        if (!textBuffer[event.stepId]) {
          textBuffer[event.stepId] = "";
        }
        textBuffer[event.stepId] += event.text;
      }
    },

    getText(stepId: string): string {
      return textBuffer[stepId] || "";
    },

    getCheckpoints(): Array<{
      candidateId: string;
      stepId: string;
      text: string;
    }> {
      const checkpoints: Array<{
        candidateId: string;
        stepId: string;
        text: string;
      }> = [];
      
      for (const event of events) {
        const e = event as any;
        if (e.kind === "checkpoint_saved" && e.candidateId && e.stepId) {
          checkpoints.push({
            candidateId: e.candidateId,
            stepId: e.stepId,
            text: this.getText(e.stepId),
          });
        }
      }
      
      return checkpoints;
    },
  };
}

/**
 * 本地模式 AI 修正生成 - 真实实现
 */
export async function handleLocalAiRevision(
  novelId: string,
  chapterId: string,
  request: unknown,
): Promise<unknown> {
  const aiRequest = request as {
    source?: string;
    scope?: string;
    instruction?: string;
    selection?: { from: number; to: number; text: string };
    provider?: string;
    model?: string;
    temperature?: number;
    [key: string]: unknown;
  };

  // 获取本地模型配置
  const modelSetup = await getLocalModelSetup();
  if (!modelSetup || !modelSetup.hasApiKey) {
    throw new Error(
      "本地模型配置未找到或 API Key 未配置。请在设置中配置 API Key。"
    );
  }

  const processor = createEventProcessor();

  try {
    // 启动本地生成
    const outcome = await startLocalGeneration({
      commandId: `local-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
      novelId,
      targetChapterId: chapterId,
      provider: aiRequest.provider || "opencode-go",
      model: aiRequest.model || "gpt-4",
      promptVersions: {
        source: aiRequest.source || "freeform",
        scope: aiRequest.scope || "selection",
        instruction: aiRequest.instruction || "",
      },
      maxAutomaticRetries: 3,
      endpoint: modelSetup.endpoint,
      allowLocalhostHttp: modelSetup.allowLocalhostHttp,
    });

    // 订阅生成事件
    await subscribeLocalGeneration(outcome.runId, (event) =>
      processor.process(event)
    );
  } catch (error) {
    const message = error instanceof Error ? error.message : "本地生成启动失败";
    console.error("[Local Generation Error]", message, error);
    throw new Error(message);
  }

  // 提取生成的候选方案
  const checkpoints = processor.getCheckpoints();
  if (checkpoints.length === 0) {
    throw new Error("生成未产生任何候选方案。请检查配置并重试。");
  }

  // 返回 ChapterEditorResponse 格式
  return {
    data: {
      sessionId: `local-${Date.now()}`,
      candidates: checkpoints.map((cp) => ({
        id: cp.candidateId,
        source: aiRequest.source || "freeform",
        scope: aiRequest.scope || "selection",
        content: cp.text,
        applied: false,
      })),
      activeCandidateId: checkpoints[0]?.candidateId ?? null,
    },
  };
}

/**
 * 本地模式光标续写生成 - 真实实现
 */
export async function handleLocalCursorContinuation(
  novelId: string,
  chapterId: string,
  request: unknown,
): Promise<unknown> {
  const contRequest = request as {
    operation?: string;
    cursorOffset?: number;
    provider?: string;
    model?: string;
    temperature?: number;
    [key: string]: unknown;
  };

  // 获取本地模型配置
  const modelSetup = await getLocalModelSetup();
  if (!modelSetup || !modelSetup.hasApiKey) {
    throw new Error(
      "本地模型配置未找到或 API Key 未配置。请在设置中配置 API Key。"
    );
  }

  const processor = createEventProcessor();

  try {
    // 启动本地生成
    const outcome = await startLocalGeneration({
      commandId: `local-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
      novelId,
      targetChapterId: chapterId,
      provider: contRequest.provider || "opencode-go",
      model: contRequest.model || "gpt-4",
      promptVersions: {
        source: "continuation",
        operation: contRequest.operation || "continue",
        cursorOffset: String(contRequest.cursorOffset ?? 0),
      },
      maxAutomaticRetries: 3,
      endpoint: modelSetup.endpoint,
      allowLocalhostHttp: modelSetup.allowLocalhostHttp,
    });

    // 订阅生成事件
    await subscribeLocalGeneration(outcome.runId, (event) =>
      processor.process(event)
    );
  } catch (error) {
    const message = error instanceof Error ? error.message : "本地续写启动失败";
    console.error("[Local Continuation Error]", message, error);
    throw new Error(message);
  }

  // 提取生成的候选方案
  const checkpoints = processor.getCheckpoints();
  if (checkpoints.length === 0) {
    throw new Error("续写未产生任何候选方案。请检查配置并重试。");
  }

  // 返回 ChapterEditorResponse 格式
  return {
    data: {
      sessionId: `local-${Date.now()}`,
      operation: contRequest.operation || "continue",
      candidates: checkpoints.map((cp) => ({
        id: cp.candidateId,
        operation: contRequest.operation || "continue",
        content: cp.text,
        applied: false,
      })),
      activeCandidateId: checkpoints[0]?.candidateId ?? null,
    },
  };
}
