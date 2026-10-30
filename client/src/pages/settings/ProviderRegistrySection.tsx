import { useCallback, useEffect, useMemo, useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { providerClient } from "@/platform/providerTransport";
import type {
  CatalogProvider,
  ModelBindingView,
  ModelView,
  ProviderError,
  ProviderView,
} from "@/platform/providerTransport";
import {
  BINDING_TASK_TYPES,
  applyTemplate,
  canBindForGeneration,
  describeProviderError,
  draftFromProvider,
  emptyDraft,
  mergeCandidates,
  toggleSelection,
  validateDraft,
  type ProviderDraft,
} from "./providerRegistryModel";

function asProviderError(error: unknown): ProviderError {
  if (error && typeof error === "object" && "code" in error && "message" in error) {
    return error as ProviderError;
  }
  return { code: "STORAGE", message: "操作失败，请稍后重试。", retryable: true };
}

export default function ProviderRegistrySection() {
  const [templates, setTemplates] = useState<CatalogProvider[]>([]);
  const [providers, setProviders] = useState<ProviderView[]>([]);
  const [models, setModels] = useState<ModelView[]>([]);
  const [bindings, setBindings] = useState<ModelBindingView[]>([]);
  const [draft, setDraft] = useState<ProviderDraft>(emptyDraft());
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [previewToken, setPreviewToken] = useState<string | null>(null);
  const [candidates, setCandidates] = useState<ModelView[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [message, setMessage] = useState<string>("");

  const run = useCallback(async (label: string, action: () => Promise<void>) => {
    setBusy(label);
    setMessage("");
    try {
      await action();
    } catch (error) {
      setMessage(describeProviderError(asProviderError(error)));
    } finally {
      setBusy(null);
    }
  }, []);

  const refreshProviders = useCallback(async () => {
    const list = await providerClient.listProfiles();
    setProviders(list);
  }, []);

  const loadModels = useCallback(async (providerId: string) => {
    setModels(await providerClient.listModels(providerId));
  }, []);

  useEffect(() => {
    void run("load", async () => {
      const [templateList, providerList, bindingList] = await Promise.all([
        providerClient.listTemplates(),
        providerClient.listProfiles(),
        providerClient.listBindings(),
      ]);
      setTemplates(templateList);
      setProviders(providerList);
      setBindings(bindingList);
    });
  }, [run]);

  const selected = useMemo(
    () => providers.find((item) => item.id === draft.providerId) ?? null,
    [providers, draft.providerId],
  );

  const rows = useMemo(
    () => mergeCandidates(candidates, models, selectedIds),
    [candidates, models, selectedIds],
  );

  const chooseProvider = (view: ProviderView) => {
    setDraft(draftFromProvider(view));
    setCandidates([]);
    setPreviewToken(null);
    setSelectedIds([]);
    void run("models", async () => loadModels(view.id));
  };

  const startNew = () => {
    setDraft(emptyDraft());
    setModels([]);
    setCandidates([]);
    setPreviewToken(null);
    setSelectedIds([]);
  };

  const save = () =>
    run("save", async () => {
      const check = validateDraft(draft, selected?.credentialPresent ?? false);
      if (!check.ok) {
        setMessage(check.message ?? "请检查填写内容");
        return;
      }
      const saved = await providerClient.upsertProfile({
        providerId: draft.providerId,
        displayName: draft.displayName || null,
        protocol: draft.protocol,
        baseUrl: draft.baseUrl.trim(),
        allowLocalhostHttp: draft.allowLocalhostHttp,
        credential:
          draft.credentialKind === "replace"
            ? { kind: "replace", apiKey: draft.apiKey.trim() }
            : { kind: draft.credentialKind },
        expectedRevision: draft.expectedRevision,
      });
      await refreshProviders();
      setDraft(draftFromProvider(saved));
      setMessage("已保存。");
    });

  const discover = () =>
    run("discover", async () => {
      const preview = await providerClient.discover({
        providerId: targetProviderId(draft),
        protocol: draft.protocol,
        baseUrl: draft.baseUrl.trim(),
        apiKey: draft.apiKey.trim() ? draft.apiKey.trim() : null,
        allowLocalhostHttp: draft.allowLocalhostHttp,
      });
      setCandidates(preview.models);
      setPreviewToken(preview.previewToken);
      setSelectedIds(preview.models.map((model) => model.modelId));
      setMessage(
        preview.warnings.length > 0
          ? `取到 ${preview.models.length} 个模型：${preview.warnings.join("；")}`
          : `取到 ${preview.models.length} 个模型，勾选后保存。`,
      );
    });

  const applyDiscovery = () =>
    run("apply", async () => {
      if (!previewToken) {
        setMessage("请先获取模型列表。");
        return;
      }
      const outcome = await providerClient.applyDiscovery({
        providerId: draft.providerId,
        previewToken,
        selectedModelIds: selectedIds,
        expectedRevision: draft.expectedRevision,
      });
      setPreviewToken(null);
      setCandidates([]);
      await loadModels(draft.providerId);
      setMessage(
        `已加入 ${outcome.added} 个模型，更新 ${outcome.updated} 个，保留人工设置 ${outcome.preserved} 个。`,
      );
    });

  const saveBinding = (taskType: string, providerId: string, modelId: string) =>
    run(`bind-${taskType}`, async () => {
      const provider = providers.find((item) => item.id === providerId);
      if (!provider || !canBindForGeneration(provider.protocol)) {
        setMessage("这个来源暂时只能取模型，还不能用于创作生成。");
        return;
      }
      await providerClient.upsertBinding({ taskType, providerId, modelId, expectedRevision: provider.revision });
      setBindings(await providerClient.listBindings());
      setMessage("任务绑定已更新。");
    });

  return (
    <div className="space-y-6" data-testid="provider-registry">
      <div className="flex flex-wrap items-center gap-2">
        <h3 className="text-sm font-semibold">模型来源</h3>
        <span className="text-xs text-muted-foreground">{providers.length} 个</span>
        <Button size="sm" variant="outline" onClick={startNew} disabled={busy !== null}>
          新增来源
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => void run("providers", refreshProviders)}
          disabled={busy !== null}
        >
          刷新
        </Button>
      </div>

      <div className="flex flex-wrap gap-2">
        {providers.map((item) => (
          <Button
            key={item.id}
            size="sm"
            variant={item.id === draft.providerId ? "default" : "outline"}
            onClick={() => chooseProvider(item)}
            disabled={busy !== null}
          >
            {item.displayName ?? item.id}
            {item.credentialPresent ? "" : "（无密钥）"}
          </Button>
        ))}
      </div>

      <div className="space-y-3">
        <div className="flex flex-wrap gap-2">
          {templates.map((template) => (
            <Button
              key={template.id}
              size="sm"
              variant="ghost"
              onClick={() => setDraft(applyTemplate(template, draft))}
              disabled={busy !== null}
            >
              {template.displayName}
            </Button>
          ))}
        </div>

        <div className="grid gap-3 sm:grid-cols-2">
          <label className="space-y-1">
            <span className="text-xs text-muted-foreground">标识</span>
            <Input
              value={draft.providerId}
              onChange={(event) => setDraft({ ...draft, providerId: event.target.value })}
              placeholder="my-gateway"
            />
          </label>
          <label className="space-y-1">
            <span className="text-xs text-muted-foreground">接口地址</span>
            <Input
              value={draft.baseUrl}
              onChange={(event) => setDraft({ ...draft, baseUrl: event.target.value })}
              placeholder="https://api.deepseek.com/v1"
            />
          </label>
          <label className="space-y-1">
            <span className="text-xs text-muted-foreground">密钥处理</span>
            <select
              className="h-9 w-full rounded-md border border-input bg-transparent px-2 text-sm"
              value={draft.credentialKind}
              onChange={(event) =>
                setDraft({ ...draft, credentialKind: event.target.value as ProviderDraft["credentialKind"] })
              }
            >
              <option value="keep">保留已保存的密钥</option>
              <option value="replace">替换密钥</option>
              <option value="clear">清除密钥</option>
            </select>
          </label>
          <label className="space-y-1">
            <span className="text-xs text-muted-foreground">API Key</span>
            <Input
              type="password"
              value={draft.apiKey}
              onChange={(event) => setDraft({ ...draft, apiKey: event.target.value })}
              placeholder={draft.credentialKind === "replace" ? "sk-..." : "当前操作不需要填写"}
              disabled={draft.credentialKind !== "replace"}
            />
          </label>
        </div>

        <div className="flex flex-wrap gap-2">
          <Button size="sm" onClick={() => void save()} disabled={busy !== null}>
            保存
          </Button>
          <Button size="sm" variant="outline" onClick={() => void discover()} disabled={busy !== null}>
            获取模型
          </Button>
          <Button
            size="sm"
            variant="outline"
            onClick={() => void applyDiscovery()}
            disabled={busy !== null || !previewToken}
          >
            加入所选模型
          </Button>
        </div>

        {rows.length > 0 ? (
          <ul className="space-y-1">
            {rows.map((row) => (
              <li key={row.modelId} className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={row.selected}
                  onChange={() => setSelectedIds(toggleSelection(selectedIds, row.modelId))}
                />
                <span>{row.displayName}</span>
                {row.alreadySaved ? <span className="text-xs text-muted-foreground">已保存</span> : null}
              </li>
            ))}
          </ul>
        ) : null}

        {models.length > 0 ? (
          <div className="space-y-1">
            <span className="text-xs text-muted-foreground">已保存模型</span>
            <ul className="space-y-1">
              {models.map((model) => (
                <li key={model.modelId} className="flex items-center justify-between text-sm">
                  <span>
                    {model.displayName ?? model.modelId}
                    <span className="ml-2 text-xs text-muted-foreground">{model.source}</span>
                  </span>
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() =>
                      void run("delete-model", async () => {
                        await providerClient.deleteModel(draft.providerId, model.modelId, draft.expectedRevision);
                        await loadModels(draft.providerId);
                      })
                    }
                    disabled={busy !== null}
                  >
                    删除
                  </Button>
                </li>
              ))}
            </ul>
          </div>
        ) : null}
      </div>

      <div className="space-y-2">
        <h3 className="text-sm font-semibold">任务绑定</h3>
        <p className="text-xs text-muted-foreground">给每个创作环节指定模型，未绑定的环节使用默认模型。</p>
        <ul className="space-y-2">
          {BINDING_TASK_TYPES.map((task) => {
            const binding = bindings.find((item) => item.taskType === task.value);
            return (
              <li key={task.value} className="flex flex-wrap items-center gap-2 text-sm">
                <span className="w-24 text-muted-foreground">{task.label}</span>
                <select
                  className="h-8 rounded-md border border-input bg-transparent px-2 text-sm"
                  value={binding?.providerId ?? ""}
                  onChange={(event) => {
                    const providerId = event.target.value;
                    if (!providerId) return;
                    void run(`models-${providerId}`, async () => {
                      const list = await providerClient.listModels(providerId);
                      setModels(list);
                      const first = list[0]?.modelId;
                      if (first) await saveBinding(task.value, providerId, binding?.modelId ?? first);
                    });
                  }}
                >
                  <option value="">未绑定</option>
                  {providers.map((provider) => (
                    <option key={provider.id} value={provider.id}>
                      {provider.displayName ?? provider.id}
                    </option>
                  ))}
                </select>
                <span className="text-xs text-muted-foreground">{binding?.modelId ?? "默认模型"}</span>
              </li>
            );
          })}
        </ul>
      </div>

      {message ? <p className="text-sm text-muted-foreground">{message}</p> : null}
    </div>
  );
}

function targetProviderId(draft: ProviderDraft): string | null {
  return draft.expectedRevision === null ? null : draft.providerId;
}
