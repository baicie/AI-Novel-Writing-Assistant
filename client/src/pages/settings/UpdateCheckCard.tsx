import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  checkForUpdate,
  installAvailableUpdate,
  isUpdateCheckAvailable,
  readCurrentVersion,
  resetPendingUpdate,
} from "@/platform/desktopUpdater";
import {
  describeUpdateState,
  initialUpdateState,
  progressPercent,
  updateReducer,
  type UpdateState,
} from "./updateCheckState";

export default function UpdateCheckCard() {
  const [state, setState] = useState<UpdateState>(initialUpdateState);
  const [currentVersion, setCurrentVersion] = useState("");
  const desktop = isUpdateCheckAvailable();

  useEffect(() => {
    let alive = true;
    void readCurrentVersion().then((version) => {
      if (alive && version) setCurrentVersion(version);
    });
    return () => {
      alive = false;
    };
  }, []);
  const view = describeUpdateState(state, { desktop });
  const percent = progressPercent(state.progress);

  const handleCheck = async () => {
    setState((prev) => updateReducer(prev, { type: "reset" }));
    setState((prev) => updateReducer(prev, { type: "check-started" }));
    resetPendingUpdate();
    try {
      const update = await checkForUpdate();
      if (!update) {
        setState((prev) => updateReducer(prev, { type: "check-none" }));
        return;
      }
      setState((prev) =>
        updateReducer(prev, { type: "check-found", version: update.version, notes: update.notes }),
      );
    } catch (error) {
      setState((prev) =>
        updateReducer(prev, { type: "failed", message: error instanceof Error ? error.message : "" }),
      );
    }
  };

  const handleInstall = async () => {
    setState((prev) => updateReducer(prev, { type: "install-started" }));
    try {
      const installed = await installAvailableUpdate((progress) => {
        setState((prev) => updateReducer(prev, { type: "install-progress", ...progress }));
      });
      if (!installed) {
        setState((prev) => updateReducer(prev, { type: "failed", message: "没有可安装的更新。" }));
        return;
      }
      setState((prev) => updateReducer(prev, { type: "install-finished" }));
    } catch (error) {
      setState((prev) =>
        updateReducer(prev, { type: "failed", message: error instanceof Error ? error.message : "" }),
      );
    }
  };

  return (
    <Card data-testid="settings-update-check">
      <CardHeader>
        <CardTitle>版本更新</CardTitle>
        <CardDescription>
          {currentVersion ? `当前版本 ${currentVersion}` : "在这里查看是否有新版本并下载。"}
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="space-y-1">
          <p className="text-sm font-medium" data-testid="settings-update-label">
            {view.label}
          </p>
          <p className="text-sm text-muted-foreground" data-testid="settings-update-detail">
            {view.detail}
          </p>
          {state.phase === "downloading" && percent != null ? (
            <div className="h-1.5 w-full overflow-hidden rounded-full bg-muted">
              <div className="h-full rounded-full bg-primary transition-all" style={{ width: `${percent}%` }} />
            </div>
          ) : null}
        </div>
        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            variant="outline"
            disabled={view.busy || view.action !== "check"}
            onClick={handleCheck}
            data-testid="settings-update-check-button"
          >
            {state.phase === "checking" ? "检查中…" : "检查更新"}
          </Button>
          {view.action === "install" ? (
            <Button type="button" onClick={handleInstall} data-testid="settings-update-install-button">
              下载并安装
            </Button>
          ) : null}
        </div>
      </CardContent>
    </Card>
  );
}
