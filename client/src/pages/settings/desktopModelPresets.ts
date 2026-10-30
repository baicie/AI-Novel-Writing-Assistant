import type { LLMProvider } from "@ai-novel/shared/types/llm";

export interface DesktopModelPreset {
  id: string;
  name: string;
  provider: LLMProvider;
  baseURL: string;
  model: string;
  hint: string;
}

// 桌面内核只有一套「地址 + 模型 + 密钥」配置，这里按厂商给预设，填完就能用。
export const DESKTOP_MODEL_PRESETS: DesktopModelPreset[] = [
  {
    id: "deepseek",
    name: "DeepSeek",
    provider: "deepseek",
    baseURL: "https://api.deepseek.com/v1",
    model: "deepseek-chat",
    hint: "在 platform.deepseek.com 申请 API Key",
  },
  {
    id: "openai",
    name: "OpenAI",
    provider: "openai",
    baseURL: "https://api.openai.com/v1",
    model: "gpt-4o-mini",
    hint: "在 platform.openai.com 申请 API Key",
  },
  {
    id: "moonshot",
    name: "月之暗面",
    provider: "moonshot",
    baseURL: "https://api.moonshot.cn/v1",
    model: "moonshot-v1-8k",
    hint: "在 platform.moonshot.cn 申请 API Key",
  },
  {
    id: "custom",
    name: "自定义接口",
    provider: "openai-compatible",
    baseURL: "",
    model: "",
    hint: "任何兼容 OpenAI 协议的地址都可以，例如自建网关",
  },
];

export function presetById(id: string): DesktopModelPreset | undefined {
  return DESKTOP_MODEL_PRESETS.find((preset) => preset.id === id);
}

/** 已保存的配置反推厂商：按地址前缀匹配，匹配不到归到自定义。 */
export function presetForEndpoint(endpoint: string): DesktopModelPreset {
  const normalized = endpoint.trim().replace(/\/+$/, "").toLowerCase();
  const matched = DESKTOP_MODEL_PRESETS.find(
    (preset) => preset.baseURL && normalized.startsWith(preset.baseURL.toLowerCase()),
  );
  return matched ?? DESKTOP_MODEL_PRESETS[DESKTOP_MODEL_PRESETS.length - 1]!;
}

/** 本地 http 地址需要显式允许，否则内核会拒绝明文请求。 */
export function requiresLocalhostPermission(endpoint: string): boolean {
  return /^http:\/\/(localhost|127\.0\.0\.1|\[::1\])(:\d+)?(\/|$)/i.test(endpoint.trim());
}

export function validateModelForm(form: { endpoint: string; model: string; apiKey: string; hasSavedKey: boolean }): string | null {
  if (!form.endpoint.trim()) return "请填写接口地址";
  if (!/^https?:\/\//i.test(form.endpoint.trim())) return "接口地址需要以 http:// 或 https:// 开头";
  if (!form.model.trim()) return "请填写模型名称";
  if (!form.apiKey.trim() && !form.hasSavedKey) return "请填写 API Key";
  return null;
}
