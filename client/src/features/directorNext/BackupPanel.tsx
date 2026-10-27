import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { createBackup, listBackups, verifyBackup, type BackupInfo } from "@/platform/desktopTransport";

export default function BackupPanel({ novelId }: { novelId: string }) {
  const [backups, setBackups] = useState<BackupInfo[]>([]);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = async () => {
    try { setBackups(await listBackups()); } catch { setBackups([]); }
  };
  useEffect(() => { void refresh(); }, [novelId]);

  const create = async () => {
    setBusy(true);
    try {
      const info = await createBackup();
      await refresh();
      setMessage(`已创建备份 ${info.fileName}（${(info.bytes / 1024 / 1024).toFixed(1)} MB）`);
    } catch (cause: unknown) {
      setMessage(cause instanceof Error ? cause.message : "创建备份失败。");
    } finally { setBusy(false); }
  };

  const verify = async (backup: BackupInfo) => {
    setBusy(true);
    try {
      const result = await verifyBackup(backup.fileName);
      setMessage(`备份完整性：${result}`);
    } catch (cause: unknown) {
      setMessage(cause instanceof Error ? cause.message : "备份校验失败。");
    } finally { setBusy(false); }
  };

  return (
    <section className="space-y-2 rounded-xl bg-muted/15 p-3" data-testid="backup-panel">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h3 className="text-sm font-semibold">本地备份</h3>
          <p className="mt-1 text-xs text-muted-foreground">为当前作品数据库创建一致性快照。备份保存在应用数据目录，不会上传。</p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={() => void refresh()} disabled={busy}>刷新</Button>
          <Button data-testid="backup-create" onClick={() => void create()} disabled={busy}>创建备份</Button>
        </div>
      </div>
      {message ? <p className="text-xs text-muted-foreground" role="status">{message}</p> : null}
      {backups.length ? <ul className="space-y-1">
        {backups.slice(0, 8).map((backup) => (
          <li key={backup.fileName} className="flex flex-wrap items-center justify-between gap-2 rounded-lg bg-background/60 px-3 py-2 text-xs">
            <span>{backup.fileName} · {(backup.bytes / 1024 / 1024).toFixed(1)} MB</span>
            <Button data-testid="backup-verify" variant="outline" onClick={() => void verify(backup)} disabled={busy}>校验</Button>
          </li>
        ))}
      </ul> : <p className="text-xs text-muted-foreground">还没有备份。</p>}
    </section>
  );
}
