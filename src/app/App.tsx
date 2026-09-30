import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import { api, type Status } from "../lib/api";
import { Tally } from "../tally/Tally";
import Onboarding from "./Onboarding";
import SettingsView from "./Settings";

export default function App() {
  const [status, setStatus] = useState<Status | null>(null);
  const [onboarding, setOnboarding] = useState<boolean | null>(null);

  const refresh = useCallback(async () => {
    try {
      const s = await api.status();
      setStatus(s);
      setOnboarding((prev) => (prev === null ? !s.settings.onboarded : prev));
    } catch (e) {
      console.error(e);
    }
  }, []);

  // Permissions and model state change outside the app (System Settings, downloads),
  // so keep the view fresh while the window is open.
  useEffect(() => {
    refresh();
    const t = setInterval(() => document.visibilityState === "visible" && refresh(), 1500);
    const off = listen("tauri://focus", refresh);
    return () => {
      clearInterval(t);
      off.then((f) => f());
    };
  }, [refresh]);

  if (!status || onboarding === null) {
    return (
      <div className="loading">
        <Tally size={56} mood="working" className="wiggle" />
      </div>
    );
  }

  return onboarding ? (
    <Onboarding status={status} refresh={refresh} onDone={() => setOnboarding(false)} />
  ) : (
    <SettingsView status={status} refresh={refresh} />
  );
}
