import { useCallback, useEffect, useMemo, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { providerClient } from "@/platform/providerTransport";
import type { ModelBindingView, ModelView, ProviderView } from "@/platform/providerTransport";

const TASK_TYPE = "writer";

/** 模型没声明支持的强度时，按主流网关的常见档位给一组。 */
const FALLBACK_EFFORTS = ["low", "medium", "high"];

export default function DesktopModelSelector() {
  const navigate = useNavigate();
  const location = useLocation();
  const [providers, setProviders] = useState<ProviderView[]>([]);
  const [models, setModels] = useState<ModelView[]>([]);
  const [providerId, setProviderId] = useState("");
  const [modelId, setModelId] = useState("");
  const [effort, setEffort] = useState("");
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");
  const [loaded, setLoaded] = useState(false);

  const applyBinding = useCallback(
    (bindings: ModelBindingView[]) => {
      const binding = bindings.find((item) => item.taskType === TASK_TYPE);
      if (!binding) return;
      setProviderId(binding.providerId);
      setModelId(binding.modelId);
      setEffort(binding.reasoningEffort ?? "");
    },
    [],
  );

  const load = useCallback(async () => {
    try {
      const [nextProviders, bindings] = await Promise.all([
        providerClient.listProfiles(),
        providerClient.listBindings(),
      ]);
      setProviders(nextProviders);
      applyBinding(bindings);
      setLoaded(true);
    } catch {
      setMessage("模型来源读取失败");
      setLoaded(true);
    }
  }, [applyBinding]);

  // 切页或回到窗口时重新读取：在设置里刚加完模型，导航栏要立刻反映。
  useEffect(() => {
    void load();
  }, [load, location.pathname]);

  useEffect(() => {
    const onFocus = () => void load();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [load]);

  useEffect(() => {
    if (!providerId) {
      setModels([]);
      return;
    }
    void (async () => {
      try {
        const next = await providerClient.listModels(providerId);
        setModels(next);
      } catch {
        setModels([]);
      }
    })();
  }, [providerId]);

  const currentModel = useMemo(
    () => models.find((item) => item.modelId === modelId) ?? null,
    [models, modelId],
  );

  const effortOptions = useMemo(() => {
    const declared = currentModel?.capabilities.reasoningEfforts;
    return declared && declared.length ? declared : FALLBACK_EFFORTS;
  }, [currentModel]);

  // 模型声明了档位而用户选了未声明的档位时只做提示，不阻断保存。
  const protocolSupportsEffort = useMemo(() => {
    const provider = providers.find((item) => item.id === providerId);
    if (!provider) return true;
    return provider.protocol !== "ollama";
  }, [providers, providerId]);

  const effortUndeclared = useMemo(() => {
    const declared = currentModel?.capabilities.reasoningEfforts;
    if (!effort || !declared || !declared.length) return false;
    return !declared.includes(effort);
  }, [currentModel, effort]);

  const save = async (next: { providerId?: string; modelId?: string; effort?: string }) => {
    const nextProvider = next.providerId ?? providerId;
    const nextModel = next.modelId ?? modelId;
    if (!nextProvider || !nextModel) return;
    setSaving(true);
    setMessage("");
    try {
      await providerClient.upsertBinding({
        taskType: TASK_TYPE,
        providerId: nextProvider,
        modelId: nextModel,
        reasoningEffort: (next.effort ?? effort) || null,
      });
      setMessage("已保存");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "保存失败");
    } finally {
      setSaving(false);
    }
  };

  const selectClass =
    "h-8 max-w-[9.5rem] truncate rounded-md border border-border bg-background px-2 text-xs text-foreground";

  if (!loaded) {
    return <span className="hidden text-xs text-muted-foreground sm:inline">正在读取模型…</span>;
  }

  if (!providers.length) {
    return (
      <div className="hidden items-center gap-2 sm:flex">
        <span className="text-xs text-amber-600">尚未添加模型来源</span>
        <Button size="sm" variant="ghost" onClick={() => navigate("/settings")}>
          去添加
        </Button>
      </div>
    );
  }

  return (
    <div className="hidden items-center gap-1.5 sm:flex">
      <select
        aria-label="模型来源"
        className={selectClass}
        value={providerId}
        onChange={(event) => {
          const value = event.target.value;
          setProviderId(value);
          setModelId("");
          setEffort("");
        }}
      >
        <option value="">选择来源</option>
        {providers.map((provider) => (
          <option key={provider.id} value={provider.id}>
            {provider.displayName ?? provider.id}
          </option>
        ))}
      </select>

      <select
        aria-label="模型"
        className={selectClass}
        value={modelId}
        disabled={!providerId}
        onChange={(event) => {
          const value = event.target.value;
          setModelId(value);
          void save({ modelId: value });
        }}
      >
        <option value="">选择模型</option>
        {models.map((model) => (
          <option key={model.modelId} value={model.modelId}>
            {model.displayName ?? model.modelId}
          </option>
        ))}
      </select>

      <select
        aria-label="推理强度"
        className={selectClass}
        value={effort}
        disabled={!modelId}
        onChange={(event) => {
          const value = event.target.value;
          setEffort(value);
          void save({ effort: value });
        }}
      >
        <option value="">推理强度：默认</option>
        {effortOptions.map((item) => (
          <option key={item} value={item}>
            {item}
          </option>
        ))}
      </select>

      {!protocolSupportsEffort ? (
        <span className="text-xs text-muted-foreground" title="该协议没有推理强度参数">
          该协议不支持强度
        </span>
      ) : null}
      {effortUndeclared && protocolSupportsEffort ? (
        <span className="text-xs text-amber-600" title="模型没有声明这一档，仍会按所选发送">
          未声明档位
        </span>
      ) : null}
      {saving ? <span className="text-xs text-muted-foreground">保存中…</span> : null}
      {!saving && message ? (
        <span className="text-xs text-muted-foreground">{message}</span>
      ) : null}
    </div>
  );
}
