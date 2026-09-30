import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import { api, type Status } from "../lib/api";
import { Tally } from "../tally/Tally";
import SettingsView, { type Page } from "./Settings";
import Tour, { markToured } from "./Tour";

export default function App() {
  const [status, setStatus] = useState<Status | null>(null);
  const [page, setPage] = useState<Page>("general");
  const [touring, setTouring] = useState(false);
  const [practiced, setPracticed] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const s = await api.status();
      setStatus((prev) => {
        // First load: new users get Tally's tour (the app stays usable underneath).
        if (!prev && !s.settings.onboarded) setTouring(true);
        return s;
      });
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

  if (!status) {
    return (
      <div className="loading">
        <Tally size={56} mood="working" className="wiggle" />
      </div>
    );
  }

  return (
    <>
      <SettingsView
        status={status}
        refresh={refresh}
        page={page}
        setPage={setPage}
        onPracticed={() => setPracticed(true)}
        onTour={() => setTouring(true)}
      />
      {touring && (
        <Tour
          status={status}
          setPage={setPage}
          practiced={practiced}
          onClose={() => {
            setTouring(false);
            markToured().then(refresh);
          }}
        />
      )}
    </>
  );
}
