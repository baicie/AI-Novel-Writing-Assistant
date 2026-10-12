import { Button } from "@/components/ui/button";
import { useLocalCreationMode } from "@/hooks/useLocalCreationMode";
import { cn } from "@/lib/utils";
import { Cloud, Cpu } from "lucide-react";

/**
 * 创作工作台的模式选择器。
 * 用户可在本地创作和云创作之间切换。
 */
export function CreationModeSelector({ novelId }: { novelId: string }) {
  const { mode, setMode } = useLocalCreationMode(novelId);

  return (
    <div className="flex items-center gap-2 rounded-lg bg-muted/50 p-1">
      <Button
        variant={mode === "local" ? "default" : "ghost"}
        size="sm"
        onClick={() => setMode("local")}
        className="flex items-center gap-2"
      >
        <Cpu className="h-4 w-4" />
        <span className="hidden sm:inline">本地创作</span>
      </Button>

      <Button
        variant={mode === "cloud" ? "default" : "ghost"}
        size="sm"
        onClick={() => setMode("cloud")}
        className="flex items-center gap-2"
      >
        <Cloud className="h-4 w-4" />
        <span className="hidden sm:inline">云创作</span>
      </Button>
    </div>
  );
}
