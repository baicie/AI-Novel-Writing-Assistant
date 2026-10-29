import { hasDesktopShell } from "@/platform/desktopTransport";
import DirectorRunHistoryPage from "@/pages/directorNext/DirectorRunHistoryPage";
import TauriDirectorHistoryPage from "./TauriDirectorHistoryPage";

export default function DirectorHistoryRoute() {
  if (hasDesktopShell()) return <TauriDirectorHistoryPage />;
  return <DirectorRunHistoryPage />;
}
