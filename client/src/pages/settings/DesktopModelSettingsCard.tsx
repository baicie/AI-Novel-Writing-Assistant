import { useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  getModelSetup,
  hasApiKey,
  saveModelSetup,
  testModelSetup,
  type ModelSetupView,
} from "@/platform/desktopTransport";
import {
  DESKTOP_MODEL_PRESETS,
  presetById,
  presetForEndpoint,
  needsPlaintextHttpPermission,
  validateModelForm,
} from "./desktopModelPresets";

type Phase = "idle" | "saving" | "testing" | "ok" | "error";

export default function DesktopModelSettingsCard() {
  const [loaded, setLoaded] = useState<ModelSetupView | null>(null);
  const [keyPresent, setKeyPresent] = useState(false);
  const [presetId, setPresetId] = useState("deepseek");
  const [endpoint, setEndpoint] = useState("");
  const [model, setModel] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [phase, setPhase] = useState<Phase>("idle");
  const [message, setMessage] = useState("");

  useEffect(() => {
    let alive = true;
    void (async () => {
      const [setup, hasKey] = await Promise.all([getModelSetup(), hasApiKey()]);
      if (!alive) return;
      setLoaded(setup);
      setKeyPresent(hasKey);
      if (setup) {
        setPresetId(presetForEndpoint(setup.endpoint).id);
        setEndpoint(setup.endpoint);
        setModel(setup.model);
      }
    })();
    return () => {
      alive = false;
    };
  }, []);

  const preset = useMemo(() => presetById(presetId), [presetId]);
  const configured = loaded?.hasApiKey === true || keyPresent;
  const validation = validateModelForm({ endpoint, model, apiKey, hasSavedKey: keyPresent });

  const choosePreset = (id: string) => {
    setPresetId(id);
    setPhase("idle");
    setMessage("");
    const next = presetById(id);
    if (!next) return;
    setEndpoint(next.baseURL);
    setModel(next.model);
    setApiKey("");
  };

  const save = async (thenTest: boolean) => {
    if (validation) {
      setPhase("error");
      setMessage(validation);
      return;
    }
    setPhase("saving");
    setMessage("");
    try {
      const saved = await saveModelSetup({
        endpoint: endpoint.trim(),
        model: model.trim(),
        allowLocalhostHttp: needsPlaintextHttpPermission(endpoint),
        apiKey: apiKey.trim(),
      });
      setLoaded(saved);
      setKeyPresent(saved.hasApiKey);
      setApiKey("");
      if (!thenTest) {
        setPhase("ok");
        setMessage("已保存，可以开始创作。");
        return;
      }
      setPhase("testing");
      const result = await testModelSetup({
        endpoint: endpoint.trim(),
        model: model.trim(),
        apiKey: "",
        allowLocalhostHttp: needsPlaintextHttpPermission(endpoint),
      });
      setPhase(result.ok ? "ok" : "error");
      setMessage(result.ok ? `${result.message}（${result.latencyMs}ms）` : result.message);
    } catch (error) {
      setPhase("error");
      setMessage(error instanceof Error ? error.message : "保存失败，请稍后再试。");
    }
  };

  return (
    <Card data-testid="desktop-model-settings">
      <CardHeader>
        <CardTitle>模型</CardTitle>
        <CardDescription>
          {configured && loaded
            ? `当前使用 ${loaded.model}（${loaded.endpoint}）`
            : "选择一个厂商，填入 API Key 就能开始创作。"}
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex flex-wrap gap-2">
          {DESKTOP_MODEL_PRESETS.map((item) => (
            <Button
              key={item.id}
              type="button"
              size="sm"
              variant={presetId === item.id ? "default" : "outline"}
              onClick={() => choosePreset(item.id)}
            >
              {item.name}
            </Button>
          ))}
        </div>

        <div className="space-y-3">
          <label className="block space-y-1">
            <span className="text-xs text-muted-foreground">接口地址</span>
            <Input
              value={endpoint}
              onChange={(event) => setEndpoint(event.target.value)}
              placeholder="https://api.deepseek.com/v1"
            />
          </label>
          <label className="block space-y-1">
            <span className="text-xs text-muted-foreground">模型名称</span>
            <Input
              value={model}
              onChange={(event) => setModel(event.target.value)}
              placeholder="deepseek-chat"
            />
          </label>
          <label className="block space-y-1">
            <span className="text-xs text-muted-foreground">API Key</span>
            <Input
              type="password"
              value={apiKey}
              onChange={(event) => setApiKey(event.target.value)}
              placeholder={keyPresent ? "已保存，留空表示不修改" : "sk-..."}
            />
            {preset?.hint ? <span className="text-xs text-muted-foreground">{preset.hint}</span> : null}
          </label>
        </div>

        {message ? (
          <p className={phase === "error" ? "text-sm text-destructive" : "text-sm text-muted-foreground"}>{message}</p>
        ) : null}

        <p className="text-xs text-muted-foreground">
          「保存并测试」会向该网关发送一次真实请求（只要求回一个字），可能产生少量费用。
        </p>
        <div className="flex flex-wrap gap-2">
          <Button type="button" onClick={() => void save(true)} disabled={phase === "saving" || phase === "testing"}>
            {phase === "testing" ? "测试中…" : phase === "saving" ? "保存中…" : "保存并发送测试请求"}
          </Button>
          <Button type="button" variant="outline" onClick={() => void save(false)} disabled={phase === "saving" || phase === "testing"}>
            仅保存
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
