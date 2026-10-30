import type {
  CatalogProvider,
  CredentialKind,
  ModelView,
  ProviderError,
  ProviderProtocol,
  ProviderView,
} from "@/platform/providerTransport";

/** 任务种类必须与 shared/types/novel.ts 的 ModelRouteTaskType 一致，不能自造。 */
export const BINDING_TASK_TYPES = [
  { value: "planner", label: "故事规划" },
  { value: "writer", label: "章节正文" },
  { value: "review", label: "章节审校" },
  { value: "light_review", label: "轻量审校" },
  { value: "critical_review", label: "严格审校" },
  { value: "repair", label: "局部修复" },
  { value: "replan", label: "重新规划" },
  { value: "state_resolution", label: "状态整理" },
  { value: "summary", label: "摘要" },
  { value: "fact_extraction", label: "事实提取" },
  { value: "chat", label: "对话" },
] as const;

export type BindingTaskType = (typeof BINDING_TASK_TYPES)[number]["value"];

export interface ProviderDraft {
  providerId: string;
  displayName: string;
  protocol: ProviderProtocol;
  baseUrl: string;
  allowLocalhostHttp: boolean;
  credentialKind: CredentialKind;
  apiKey: string;
  /** null 表示新建；数字表示更新时的 CAS 版本。 */
  expectedRevision: number | null;
}

export function emptyDraft(): ProviderDraft {
  return {
    providerId: "",
    displayName: "",
    protocol: "openai-compatible",
    baseUrl: "",
    allowLocalhostHttp: false,
    credentialKind: "replace",
    apiKey: "",
    expectedRevision: null,
  };
}

export function draftFromProvider(view: ProviderView): ProviderDraft {
  return {
    providerId: view.id,
    displayName: view.displayName ?? "",
    protocol: (view.protocol as ProviderProtocol) ?? "openai-compatible",
    baseUrl: view.baseUrl,
    allowLocalhostHttp: view.allowLocalhostHttp,
    // 编辑已有配置默认保留密钥，避免误清空。
    credentialKind: "keep",
    apiKey: "",
    expectedRevision: view.revision,
  };
}

/** 选择厂商模板时只填充地址与协议，不动用户已经填的密钥。 */
export function applyTemplate(template: CatalogProvider, draft: ProviderDraft): ProviderDraft {
  return {
    ...draft,
    protocol: template.protocol as ProviderProtocol,
    baseUrl: template.baseUrl,
    allowLocalhostHttp: template.local || draft.allowLocalhostHttp,
    displayName: draft.displayName || template.displayName,
    providerId: draft.providerId || template.id,
  };
}

export function isDirty(a: ProviderDraft, b: ProviderDraft): boolean {
  return (
    a.providerId !== b.providerId ||
    a.displayName !== b.displayName ||
    a.protocol !== b.protocol ||
    a.baseUrl !== b.baseUrl ||
    a.allowLocalhostHttp !== b.allowLocalhostHttp ||
    a.credentialKind !== b.credentialKind ||
    a.apiKey !== b.apiKey
  );
}

export interface DraftValidation {
  ok: boolean;
  message?: string;
}

export function validateDraft(draft: ProviderDraft, hasSavedKey: boolean): DraftValidation {
  if (!/^[a-z0-9_-]{1,64}$/.test(draft.providerId)) {
    return { ok: false, message: "标识只能用小写字母、数字、- 和 _（最长 64）" };
  }
  if (!/^https?:\/\//i.test(draft.baseUrl.trim())) {
    return { ok: false, message: "接口地址需要以 http:// 或 https:// 开头" };
  }
  if (draft.credentialKind === "replace" && !draft.apiKey.trim()) {
    return { ok: false, message: "替换密钥时请填写 API Key" };
  }
  if (draft.credentialKind !== "replace" && draft.apiKey.trim()) {
    return { ok: false, message: "保留或清除密钥时不要填写 API Key" };
  }
  if (draft.credentialKind === "keep" && !hasSavedKey) {
    return { ok: false, message: "这个来源还没有保存过密钥，请选择替换并填写" };
  }
  return { ok: true };
}

export interface CandidateRow {
  modelId: string;
  displayName: string;
  selected: boolean;
  alreadySaved: boolean;
}

/** 发现候选与已保存模型合并：已保存的默认勾选，人工添加的保留显示。 */
export function mergeCandidates(
  candidates: ModelView[],
  saved: ModelView[],
  selectedIds: string[],
): CandidateRow[] {
  const savedIds = new Set(saved.map((model) => model.modelId));
  const seen = new Set<string>();
  const rows: CandidateRow[] = [];
  for (const model of candidates) {
    if (seen.has(model.modelId)) continue;
    seen.add(model.modelId);
    rows.push({
      modelId: model.modelId,
      displayName: model.displayName ?? model.modelId,
      selected: selectedIds.includes(model.modelId) || savedIds.has(model.modelId),
      alreadySaved: savedIds.has(model.modelId),
    });
  }
  return rows;
}

export function toggleSelection(selectedIds: string[], modelId: string): string[] {
  return selectedIds.includes(modelId)
    ? selectedIds.filter((id) => id !== modelId)
    : [...selectedIds, modelId];
}

/** 错误码 → 用户能懂的下一步动作。 */
export function describeProviderError(error: ProviderError): string {
  switch (error.code) {
    case "VALIDATION":
      return error.message || "填写的信息还不完整，检查后再试。";
    case "CONFLICT":
      return "配置已在别处被修改，请刷新后重新保存。";
    case "NOT_FOUND":
      return "找不到这个模型来源，可能已被删除。";
    case "IN_USE":
      return "还有创作任务在用这个模型，先改掉任务绑定再删除。";
    case "CREDENTIAL_MISSING":
      return "这个来源还没有可用的密钥，请先填写 API Key。";
    case "CREDENTIAL_UNAVAILABLE":
      return "系统凭据服务暂时不可用，稍后重试即可。";
    default:
      return "保存失败，请稍后重试。";
  }
}

/** 只有具备生成适配器的协议才允许绑定创作任务。 */
export const GENERATION_READY_PROTOCOLS: ProviderProtocol[] = ["openai-compatible", "ollama"];

export function canBindForGeneration(protocol: string): boolean {
  return GENERATION_READY_PROTOCOLS.includes(protocol as ProviderProtocol);
}
