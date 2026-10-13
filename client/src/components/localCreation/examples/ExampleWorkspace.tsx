/**
 * 本地创作工作台示例集成。
 * 展示如何在实际创作页面中使用模式选择器和密钥管理。
 * 
 * 注意：这是示例代码，实际集成时根据项目的页面结构进行调整。
 */

import { useState } from "react";
import { useLocalCreationMode } from "@/hooks/useLocalCreationMode";
import { CreationModeSelector, LocalCreationKeyManager } from "@/components/localCreation";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Settings2 } from "lucide-react";

/**
 * 创作工作台示例组件。
 * 集成模式选择和密钥管理。
 */
export function ExampleLocalCreationWorkspace({ novelId, novelTitle }: { novelId: string; novelTitle: string }) {
  const { mode, isLocalMode } = useLocalCreationMode(novelId);
  const [showKeyManager, setShowKeyManager] = useState(false);

  return (
    <div className="space-y-6">
      {/* 工作台头部：模式选择 + 密钥管理 */}
      <header className="flex items-center justify-between border-b pb-4">
        <div>
          <h1 className="text-2xl font-bold">{novelTitle}</h1>
          <p className="text-sm text-muted-foreground mt-1">
            当前模式：{isLocalMode ? "🖥️ 本地创作（离线）" : "☁️ 云创作（在线）"}
          </p>
        </div>

        <div className="flex items-center gap-3">
          <CreationModeSelector novelId={novelId} />

          {isLocalMode && (
            <Dialog open={showKeyManager} onOpenChange={setShowKeyManager}>
              <DialogTrigger asChild>
                <Button variant="outline" size="sm" className="gap-2">
                  <Settings2 className="h-4 w-4" />
                  密钥设置
                </Button>
              </DialogTrigger>
              <DialogContent className="max-w-md">
                <DialogHeader>
                  <DialogTitle>系统密钥管理</DialogTitle>
                  <DialogDescription>
                    保存 API Key 到系统凭据。本地生成时会自动读取，无需重复输入。
                  </DialogDescription>
                </DialogHeader>
                <LocalCreationKeyManager />
              </DialogContent>
            </Dialog>
          )}
        </div>
      </header>

      {/* 模式提示信息 */}
      <div className="rounded-lg border p-4 bg-muted/30">
        {isLocalMode ? (
          <div className="space-y-2">
            <h3 className="font-semibold flex items-center gap-2">
              <span className="text-lg">🖥️</span>
              本地创作模式已启用
            </h3>
            <ul className="text-sm text-muted-foreground space-y-1 ml-6">
              <li>✓ 生成不需要网络连接</li>
              <li>✓ API Key 保存在本地系统凭据中</li>
              <li>✓ 支持在中断后继续创作</li>
            </ul>
            <p className="text-xs text-amber-600 dark:text-amber-400 mt-2">
              💡 提示：首次使用前，请先在密钥设置中保存 API Key。
            </p>
          </div>
        ) : (
          <div className="space-y-2">
            <h3 className="font-semibold flex items-center gap-2">
              <span className="text-lg">☁️</span>
              云创作模式已启用
            </h3>
            <ul className="text-sm text-muted-foreground space-y-1 ml-6">
              <li>✓ 使用云端服务生成</li>
              <li>✓ 所有创作保存到服务器</li>
              <li>✓ 支持多设备同步</li>
            </ul>
          </div>
        )}
      </div>

      {/* 创作区域示例 */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        {/* 主编辑区 */}
        <div className="md:col-span-2 space-y-4">
          <div className="rounded-lg border p-6 bg-card">
            <h2 className="font-semibold mb-4">创作编辑区</h2>
            <div className="prose prose-sm max-w-none">
              <p className="text-muted-foreground">
                选择本地或云创作模式后，在此进行创作。系统会自动根据选择的模式调用对应的生成引擎。
              </p>
            </div>
          </div>
        </div>

        {/* 侧边栏 */}
        <div className="space-y-4">
          <div className="rounded-lg border p-4 bg-card">
            <h3 className="font-semibold text-sm mb-3">当前设置</h3>
            <dl className="space-y-2 text-sm">
              <div>
                <dt className="text-muted-foreground">创作模式</dt>
                <dd className="font-medium">{mode === "local" ? "本地" : "云"}</dd>
              </div>
              <div>
                <dt className="text-muted-foreground">小说 ID</dt>
                <dd className="font-mono text-xs break-all">{novelId}</dd>
              </div>
            </dl>
          </div>

          {isLocalMode && (
            <div className="rounded-lg border p-4 bg-blue-50 dark:bg-blue-950">
              <h4 className="font-semibold text-sm text-blue-900 dark:text-blue-100 mb-2">
                📌 本地模式提示
              </h4>
              <ul className="text-xs text-blue-800 dark:text-blue-200 space-y-1">
                <li>• 需要有效的 API Key</li>
                <li>• 首次使用需配置端点</li>
                <li>• 支持 localhost 调试</li>
              </ul>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

/**
 * 使用示例：在路由中使用本地创作工作台
 * 
 * import { ExampleLocalCreationWorkspace } from "@/components/localCreation/examples";
 * 
 * export function NovelEditPage({ params }: { params: { novelId: string } }) {
 *   const novel = useNovelQuery(params.novelId);
 *   
 *   return (
 *     <ExampleLocalCreationWorkspace
 *       novelId={params.novelId}
 *       novelTitle={novel?.title ?? "未命名小说"}
 *     />
 *   );
 * }
 */
