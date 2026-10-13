import { useState, useEffect } from "react";
import { useMutation } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { toast } from "@/components/ui/toast";
import { getLocalModelSetup, saveLocalModelSetup, type ModelSetupView } from "@/api/localCreation";
import { LucideIcon, Check, AlertCircle, Loader2 } from "lucide-react";
import { cn } from "@/lib/utils";

/**
 * 本地创作模式的系统密钥管理界面。
 * 允许用户保存 API Key 到系统凭据，后续无需重复输入。
 */
export function LocalCreationKeyManager() {
  const [apiKey, setApiKey] = useState("");
  const [setup, setSetup] = useState<ModelSetupView | null>(null);
  const [isLoadingSetup, setIsLoadingSetup] = useState(true);

  // 加载现有配置
  useEffect(() => {
    const loadSetup = async () => {
      try {
        setIsLoadingSetup(true);
        const result = await getLocalModelSetup();
        setSetup(result);
      } catch (error) {
        console.error("加载密钥配置失败", error);
        toast.error("无法读取系统凭据配置");
      } finally {
        setIsLoadingSetup(false);
      }
    };

    loadSetup();
  }, []);

  const saveMutation = useMutation({
    mutationFn: async () => {
      if (!apiKey.trim()) {
        throw new Error("请输入 API Key");
      }
      return saveLocalModelSetup({
        endpoint: setup?.endpoint || "https://api.opencode-go.com/v1",
        model: setup?.model || "gpt-4",
        apiKey: apiKey.trim(),
        allowLocalhostHttp: setup?.allowLocalhostHttp ?? false,
      });
    },
    onSuccess: (result) => {
      setSetup(result);
      setApiKey("");
      toast.success("✓ 密钥已保存到系统凭据");
    },
    onError: (error) => {
      toast.error(`保存失败: ${error instanceof Error ? error.message : "未知错误"}`);
    },
  });

  const testConnectionMutation = useMutation({
    mutationFn: async () => {
      const result = await getLocalModelSetup();
      if (!result?.hasApiKey) {
        throw new Error("未找到已保存的密钥，请先保存 API Key");
      }
      return result;
    },
    onSuccess: () => {
      toast.success("✓ 连接成功，密钥有效");
    },
    onError: (error) => {
      toast.error(`连接失败: ${error instanceof Error ? error.message : "未知错误"}`);
    },
  });

  if (isLoadingSetup) {
    return (
      <div className="flex items-center justify-center py-8">
        <Loader2 className="h-4 w-4 animate-spin" />
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div>
        <label className="block text-sm font-medium mb-2">API Key</label>
        <div className="flex gap-2">
          <Input
            type="password"
            placeholder="输入你的 API Key"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            disabled={saveMutation.isPending}
            className="flex-1"
          />
        </div>
        <p className="mt-1 text-xs text-muted-foreground">
          密钥将保存到系统凭据中（macOS Keychain），后续使用无需重复输入。
        </p>
      </div>

      {setup?.hasApiKey && (
        <div className="flex items-center gap-2 px-3 py-2 rounded bg-emerald-500/10 border border-emerald-500/20">
          <Check className="h-4 w-4 text-emerald-600" />
          <span className="text-sm text-emerald-900 dark:text-emerald-100">
            密钥已在系统凭据中保存
          </span>
        </div>
      )}

      {setup?.endpoint && (
        <div className="text-xs text-muted-foreground space-y-1">
          <p>
            <strong>端点：</strong> {setup.endpoint}
          </p>
          <p>
            <strong>模型：</strong> {setup.model}
          </p>
        </div>
      )}

      <div className="flex gap-2 pt-4">
        <Button
          onClick={() => saveMutation.mutate()}
          disabled={saveMutation.isPending || !apiKey.trim()}
          variant="default"
          size="sm"
        >
          {saveMutation.isPending ? (
            <>
              <Loader2 className="h-4 w-4 mr-2 animate-spin" />
              保存中...
            </>
          ) : (
            "保存到系统凭据"
          )}
        </Button>

        <Button
          onClick={() => testConnectionMutation.mutate()}
          disabled={testConnectionMutation.isPending}
          variant="outline"
          size="sm"
        >
          {testConnectionMutation.isPending ? (
            <>
              <Loader2 className="h-4 w-4 mr-2 animate-spin" />
              测试中...
            </>
          ) : (
            "测试连接"
          )}
        </Button>
      </div>
    </div>
  );
}
