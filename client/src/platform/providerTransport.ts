import { invoke } from "@tauri-apps/api/core";

/** 与 Rust DTO 一一对应的类型。字段名保持 camelCase，与 serde 序列化一致。 */

export type ProviderProtocol = "openai-compatible" | "anthropic-messages" | "ollama";

export type CredentialKind = "keep" | "replace" | "clear";

export interface CredentialActionDto {
  kind: CredentialKind;
  /** 只在 kind=replace 时出现；keep/clear 带它会被告知参数错误。 */
  apiKey?: string;
}

export interface ProviderView {
  id: string;
  displayName: string | null;
  protocol: string;
  baseUrl: string;
  allowLocalhostHttp: boolean;
  isBuiltin: boolean;
  sortOrder: number;
  revision: number;
  credentialPresent: boolean;
}

export interface CatalogModel {
  id: string;
  displayName: string;
  contextWindow: number | null;
  maxOutputTokens: number | null;
  supportsTools: boolean | null;
  supportsReasoning: boolean | null;
}

export interface CatalogProvider {
  id: string;
  displayName: string;
  protocol: string;
  baseUrl: string;
  requiresApiKey: boolean;
  local: boolean;
  models: CatalogModel[];
}

export type CapabilityField =
  | "context-window"
  | "max-output-tokens"
  | "supports-streaming"
  | "supports-tools"
  | "supports-reasoning"
  | "input-modalities"
  | "reasoning-efforts";

export interface ModelCapabilitiesView {
  contextWindow: number | null;
  maxOutputTokens: number | null;
  supportsStreaming: boolean | null;
  supportsTools: boolean | null;
  supportsReasoning: boolean | null;
  inputModalities: string[] | null;
  reasoningEfforts: string[] | null;
}

export interface ModelView {
  providerId: string;
  modelId: string;
  displayName: string | null;
  capabilities: ModelCapabilitiesView;
  source: "builtin" | "discovered" | "supplemental" | "user";
  overridden: CapabilityField[];
}

export interface ModelBindingView {
  taskType: string;
  providerId: string;
  modelId: string;
  reasoningEffort: string | null;
}

export interface DiscoveryPreview {
  previewToken: string;
  providerId: string;
  models: ModelView[];
  warnings: string[];
}

export interface ApplyDiscoveredOutcome {
  added: number;
  updated: number;
  preserved: number;
  revision: number;
}

export interface ResolvedModelRoute {
  taskType: string;
  providerId: string;
  modelId: string;
  protocol: string;
  endpoint: string;
  allowLocalhostHttp: boolean;
  credentialRef: string;
  reasoningEffort: string | null;
  source: "binding" | "default";
}

export type FrozenRouteStatus =
  | { kind: "resolved"; protocol: string; credentialRef: string }
  | { kind: "legacy" };

export interface CredentialReconcileReport {
  completed: number;
  /** 需要用户重新输入密钥的 provider id（密钥正文从不落库，无法自动重放）。 */
  needsUserAction: string[];
}

export interface UpsertProviderInput {
  providerId: string;
  displayName?: string | null;
  protocol: ProviderProtocol;
  baseUrl: string;
  allowLocalhostHttp?: boolean;
  sortOrder?: number;
  isBuiltin?: boolean;
  credential: CredentialActionDto;
  /** 新建留空；更新必填（CAS）。 */
  expectedRevision?: number | null;
}

export interface UpsertModelInput {
  providerId: string;
  modelId: string;
  displayName?: string | null;
  contextWindow?: number | null;
  maxOutputTokens?: number | null;
  supportsStreaming?: boolean | null;
  supportsTools?: boolean | null;
  supportsReasoning?: boolean | null;
  expectedRevision?: number | null;
}

export interface DiscoveryDraftInput {
  providerId?: string | null;
  protocol: ProviderProtocol;
  baseUrl: string;
  apiKey?: string | null;
  allowLocalhostHttp?: boolean;
}

export interface ApplyDiscoveryInput {
  providerId: string;
  previewToken: string;
  selectedModelIds: string[];
  expectedRevision?: number | null;
}

export interface UpsertBindingInput {
  taskType: string;
  providerId: string;
  modelId: string;
  reasoningEffort?: string | null;
  expectedRevision?: number | null;
}

export type ProviderErrorCode =
  | "VALIDATION"
  | "NOT_FOUND"
  | "CONFLICT"
  | "IN_USE"
  | "CREDENTIAL_MISSING"
  | "CREDENTIAL_UNAVAILABLE"
  | "NOT_SUPPORTED"
  | "STORAGE";

export interface ProviderError {
  code: ProviderErrorCode;
  message: string;
  retryable: boolean;
  details?: unknown;
}

export type Invoker = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

/**
 * 所有 Provider IPC 的唯一出口：组件不直接写 invoke 字符串。
 * 传入 invoker 是为了让契约测试可以替换掉真实 Tauri 通道。
 */
export function createProviderClient(call: Invoker) {
  return {
    listTemplates: () => call<CatalogProvider[]>("list_provider_templates"),
    listProfiles: () => call<ProviderView[]>("list_provider_profiles"),
    getProfile: (providerId: string) =>
      call<ProviderView>("get_provider_profile", { providerId }),
    upsertProfile: (request: UpsertProviderInput) =>
      call<ProviderView>("upsert_provider_profile", { request }),
    deleteProfile: (providerId: string, force = false) =>
      call<void>("delete_provider_profile", { providerId, force }),
    listModels: (providerId: string) =>
      call<ModelView[]>("list_provider_models", { providerId }),
    upsertModel: (request: UpsertModelInput) =>
      call<void>("upsert_provider_model", { request }),
    deleteModel: (providerId: string, modelId: string, expectedRevision?: number | null) =>
      call<void>("delete_provider_model", { providerId, modelId, expectedRevision }),
    discover: (request: DiscoveryDraftInput) =>
      call<DiscoveryPreview>("discover_provider_models", { request }),
    applyDiscovery: (request: ApplyDiscoveryInput) =>
      call<ApplyDiscoveredOutcome>("apply_provider_discovery", { request }),
    listBindings: () => call<ModelBindingView[]>("list_model_bindings"),
    upsertBinding: (request: UpsertBindingInput) =>
      call<void>("upsert_model_binding", { request }),
    reconcileCredentialIntents: () =>
      call<CredentialReconcileReport>("reconcile_credential_intents"),
    getEffectiveRoute: (taskType: string) =>
      call<ResolvedModelRoute | null>("get_effective_model_route", { taskType }),
    checkRunRoute: (providerId: string, modelId: string) =>
      call<FrozenRouteStatus>("check_run_route", { providerId, modelId }),
  };
}

export type ProviderClient = ReturnType<typeof createProviderClient>;

/** 真实桌面运行时使用的客户端。 */
export const providerClient: ProviderClient = createProviderClient((command, args) =>
  invoke(command, args),
);