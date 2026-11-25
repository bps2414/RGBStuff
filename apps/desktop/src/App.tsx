import { useEffect } from "react";
import { useAppStore } from "./store/useAppStore";
import { StudioApp } from "./windows/StudioApp";
import { TrayPopover } from "./windows/TrayPopover";

export function App() {
  const params = new URLSearchParams(window.location.search);
  const windowKind = params.get("window") === "popover" ? "popover" : "studio";
  const bootstrap = useAppStore((s) => s.bootstrap);
  const loading = useAppStore((s) => s.loading);

  useEffect(() => {
    void bootstrap(windowKind === "studio");
  }, [bootstrap, windowKind]);

  if (loading) {
    return <div className="studio-shell h-full w-full rounded-xl" />;
  }

  return windowKind === "popover" ? <TrayPopover /> : <StudioApp />;
}

export default App;
