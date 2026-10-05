import { I18nProvider, resolveLanguage, useI18n } from "./i18n";
import { useEffect, useRef, useState } from "react";
import {
  Battery,
  FlaskConical,
  History,
  Info,
  LayoutDashboard,
  LockKeyhole,
  RotateCcw,
  Settings,
  X,
} from "lucide-react";
import { useBackend } from "./hooks/useBackend";
import { Dashboard } from "./pages/Dashboard";
import { SettingsPage } from "./pages/SettingsPage";
import { HistoryPage } from "./pages/HistoryPage";
import { AboutPage } from "./pages/AboutPage";
const pages = [
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { id: "history", label: "History", icon: History },
  { id: "settings", label: "Settings", icon: Settings },
  { id: "about", label: "About", icon: Info },
] as const;
type Page = (typeof pages)[number]["id"];
export default function App() {
  const backend = useBackend();
  return (
    <I18nProvider language={backend.data?.locale ?? resolveLanguage("system", navigator.languages)}>
      <AppView backend={backend} />
    </I18nProvider>
  );
}
function AppView({ backend }: { backend: ReturnType<typeof useBackend> }) {
  const { t, tr } = useI18n();
  const { data, error, busy, disconnected, perform, dismissError } = backend;
  const [page, setPage] = useState<Page>("dashboard");
  const main = useRef<HTMLElement>(null);
  useEffect(() => {
    document.documentElement.dataset.theme = data?.settings.theme ?? "system";
  }, [data?.settings.theme]);
  return (
    <div className="app-shell">
      <a className="skip-link" href="#main">
        {t("Skip to content")}
      </a>
      <aside className="sidebar">
        <div className="brand">
          <img src="/icon.svg" alt="" width="30" height="30" />
          <span>BatteryDeadline</span>
        </div>
        <nav aria-label={t("Main navigation")}>
          {pages.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              className={page === id ? "nav-item current" : "nav-item"}
              aria-current={page === id ? "page" : undefined}
              onClick={() => {
                setPage(id);
                requestAnimationFrame(() => main.current?.focus());
              }}
            >
              <Icon size={18} aria-hidden="true" />
              {t(label)}
            </button>
          ))}
        </nav>
        <div className="sidebar-footer">
          <LockKeyhole size={14} aria-hidden="true" />
          <span>{t("Private. Local. Yours.")}</span>
          <small>
            {t("Version")} {data?.version ?? "0.1.1"}
          </small>
        </div>
      </aside>
      <main id="main" ref={main} tabIndex={-1}>
        {data?.simulation && (
          <div className="simulation-banner">
            <FlaskConical size={16} aria-hidden="true" />
            <span>{t("Simulation mode")}</span>
            <small>{t("Synthetic battery \u00B7 Windows settings untouched")}</small>
          </div>
        )}
        {(error || disconnected) && (
          <div className="error-banner" role="alert">
            <p>
              {disconnected &&
                t("Latest readings are unavailable. Predictions may be out of date.")}
              {disconnected && error && " "}
              {tr(error)}
            </p>
            <button
              className="text-button"
              disabled={busy}
              onClick={() => void perform("get_snapshot")}
            >
              <RotateCcw size={14} aria-hidden="true" />
              {t("Retry")}
            </button>
            <button className="icon-button" aria-label={t("Dismiss error")} onClick={dismissError}>
              <X size={16} aria-hidden="true" />
            </button>
          </div>
        )}
        {data && data.pending_recovery > 0 && data.state === "error" && (
          <div className="recovery-banner" role="alert">
            <strong>{t("Some system settings still need to be restored.")}</strong>
            <p>
              {t(
                "Reconnect the original display and retry. A new session will remain blocked until recovery finishes.",
              )}
            </p>
            <button
              className="secondary"
              disabled={busy}
              onClick={() => void perform("restore_settings")}
            >
              {t("Restore now")}
            </button>
          </div>
        )}
        {!data ? (
          <div className="loading-state">
            <Battery size={40} aria-hidden="true" />
            <h1>{t("Getting to know your laptop.")}</h1>
            <p>{t("Checking battery readings and the settings your device supports.")}</p>
          </div>
        ) : page === "dashboard" ? (
          <Dashboard
            key={`${data.session?.id ?? "ready"}:${data.session?.ended_at ?? "active"}`}
            data={data}
            perform={perform}
            busy={busy || disconnected}
          />
        ) : page === "settings" ? (
          <SettingsPage data={data} perform={perform} busy={busy || disconnected} />
        ) : page === "history" ? (
          <HistoryPage perform={perform} busy={busy} />
        ) : (
          <AboutPage data={data} perform={perform} busy={busy} />
        )}
        <footer className="main-footer">
          <span>{t("Enough battery. A little more peace of mind.")}</span>
          <LockKeyhole size={12} aria-hidden="true" />
          <span>{t("100% local")}</span>
        </footer>
      </main>
    </div>
  );
}
