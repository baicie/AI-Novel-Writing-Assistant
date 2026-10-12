import { Button } from "@/components/ui/button";
import { useLocalCreationMode } from "@/hooks/useLocalCreationMode";
import { cn } from "@/lib/utils";
import { Cloud, Cpu } from "lucide-react";
import { toast } from "@/components/ui/toast";
import type { CreationMode } from "@/hooks/useLocalCreationMode";

/**
 * 创作工作台的模式选择器。
 * 用户可在本地创作和云创作之间切换。
 */
export function CreationModeSelector({ novelId }: { novelId: string }) {
  const { mode, setMode } = useLocalCreationMode(novelId);

  const handleModeChange = (newMode: CreationMode) => {
    if (newMode === mode) return;

    // 【提示信息】
    if (newMode === "local" && mode === "cloud") {
      toast.info("💡 已切换到本地创作。提示：首次使用需在设置中配置 API Key");
    } else if (newMode === "cloud" && mode === "local") {
      toast.info("✅ 已切换到云创作模式");
    }

    setMode(newMode);
  };

  return (
    <div className="flex items-center gap-2 rounded-lg bg-muted/50 p-1">
      <Button
        variant={mode === "local" ? "default" : "ghost"}
        size="sm"
        onClick={() => handleModeChange("local")}
        title="使用本地 Rust 内核，需配置 API Key（在设置中）"
        className="flex items-center gap-2"
      >
        <Cpu className="h-4 w-4" />
        <span className="hidden sm:inline">本地创作</span>
      </Button>

      <Button
        variant={mode === "cloud" ? "default" : "ghost"}
        size="sm"
        onClick={() => handleModeChange("cloud")}
        title="使用云端服务生成"
        className="flex items-center gap-2"
      >
        <Cloud className="h-4 w-4" />
        <span className="hidden sm:inline">云创作</span>
      </Button>
    </div>
  );
}
