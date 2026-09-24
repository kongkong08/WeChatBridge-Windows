import { useEffect, useState } from "react";
import "./App.css";
import RecordsPage from "./pages/RecordsPage";
import SettingsPage from "./pages/SettingsPage";
import FloatBall from "./FloatBall";
import { getCurrentWindow } from "@tauri-apps/api/window";

type Tab = "records" | "settings";

function MainApp() {
  const [tab, setTab] = useState<Tab>("records");

  return (
    <div className="app-shell">
      <header className="app-header">
        <div className="brand">
          <span className="brand-dot" />
          <h1>微信流</h1>
          <span className="brand-sub">WeChatBridge</span>
        </div>
        <nav className="tabs">
          <button
            className={tab === "records" ? "tab active" : "tab"}
            onClick={() => setTab("records")}
          >
            记录
          </button>
          <button
            className={tab === "settings" ? "tab active" : "tab"}
            onClick={() => setTab("settings")}
          >
            设置
          </button>
        </nav>
      </header>
      <main className="app-main">
        {tab === "records" ? <RecordsPage /> : <SettingsPage />}
      </main>
    </div>
  );
}

function App() {
  const [label, setLabel] = useState<string>("main");
  const [ready, setReady] = useState(false);

  useEffect(() => {
    const w = getCurrentWindow();
    setLabel(w.label);
    setReady(true);
  }, []);

  if (!ready) return null;
  return label === "float-ball" ? <FloatBall /> : <MainApp />;
}

export default App;
