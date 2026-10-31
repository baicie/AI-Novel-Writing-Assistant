import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { ModelView } from "@/platform/providerTransport";

interface ProviderModelListProps {
  models: ModelView[];
  busy: boolean;
  effortDrafts: Record<string, string>;
  onEffortDraftChange: (modelId: string, value: string) => void;
  onSaveEfforts: (modelId: string, raw: string) => void;
  onDeleteModel: (modelId: string) => void;
}

/** 已保存的模型条目：可以登记支持的推理强度档位，也可以删除。 */
export default function ProviderModelList(props: ProviderModelListProps) {
  if (!props.models.length) return null;

  return (
    <div className="space-y-1">
      <span className="text-xs text-muted-foreground">已保存模型</span>
      <ul className="space-y-1">
        {props.models.map((model) => {
          const declared = (model.capabilities.reasoningEfforts ?? []).join(", ");
          const draftValue = props.effortDrafts[model.modelId] ?? declared;
          return (
            <li key={model.modelId} className="flex flex-wrap items-center justify-between gap-2 text-sm">
              <span>
                {model.displayName ?? model.modelId}
                <span className="ml-2 text-xs text-muted-foreground">{model.source}</span>
              </span>
              <span className="flex items-center gap-2">
                <Input
                  className="h-8 w-40"
                  aria-label={model.modelId + " 推理强度档位"}
                  placeholder="low, medium, high"
                  value={draftValue}
                  onChange={(event) => props.onEffortDraftChange(model.modelId, event.target.value)}
                />
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => props.onSaveEfforts(model.modelId, draftValue)}
                  disabled={props.busy}
                >
                  保存档位
                </Button>
              </span>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => props.onDeleteModel(model.modelId)}
                disabled={props.busy}
              >
                删除
              </Button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
