import { hasDesktopShell } from "@/platform/desktopTransport";
import DirectorNovelPage from "@/pages/directorNext/DirectorNovelPage";
import TauriDirectorPage from "./TauriDirectorPage";

export default function DirectorRoute({ previewOnly = false }: { previewOnly?: boolean }) {
  if (!previewOnly && hasDesktopShell()) return <TauriDirectorPage />;
  return <DirectorNovelPage previewOnly={previewOnly} />;
}
