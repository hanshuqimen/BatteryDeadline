import { useI18n } from "../i18n";
import { useState } from "react";
import { Copy, Download, LockKeyhole, ShieldCheck } from "lucide-react";
import { repositoryUrl } from "../lib";
import type { AppSnapshot, Perform } from "../types";
export function AboutPage({
  data,
  perform,
  busy,
}: {
  data: AppSnapshot;
  perform: Perform;
  busy: boolean;
}) {
  const { t } = useI18n();
  const [path, setPath] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [copyError, setCopyError] = useState(false);
  return (
    <>
      <header className="page-header">
        <div>
          <h1>BatteryDeadline</h1>
          <p>{t("Make your laptop last until you need it.")}</p>
        </div>
        <img
          className="about-icon"
          src="/icon.svg"
          alt={t("BatteryDeadline battery and clock mark")}
        />
      </header>
      <div className="about-intro">
        <p>
          {t(
            "Choose a time and a battery reserve. BatteryDeadline learns your recent usage, calculates what you can afford to use, and gently adjusts supported settings within your comfort limits.",
          )}
        </p>
        <p>
          {t("Version")} {data.version} {t("\u00B7 Windows 11 x64 \u00B7 MIT license")}
        </p>
      </div>
      <section className="about-row">
        <LockKeyhole size={24} aria-hidden="true" />
        <div>
          <h2>{t("Everything stays on your laptop.")}</h2>
          <p>
            {t(
              "No account, analytics, ads, cloud service, or uploaded battery data. Your history and recovery journal live in your Windows local application data folder.",
            )}
          </p>
        </div>
      </section>
      <section className="about-row">
        <ShieldCheck size={24} aria-hidden="true" />
        <div>
          <h2>{t("Temporary changes. A clear way back.")}</h2>
          <p>
            {t(
              "Original settings are saved before each change. Stop, plug in, reach your deadline, or quit from the tray to restore them. After a crash, restart the app to run recovery. Your later manual changes take priority.",
            )}
          </p>
          <p>
            {t(
              "Predictions depend on your workload and hardware. On track is an estimate, never a guarantee.",
            )}
          </p>
        </div>
      </section>
      <section className="surface diagnostics-section">
        <h2>{t("Help us make it better.")}</h2>
        <p>
          {t(
            "Export a local diagnostics archive with capabilities, version, and session activity. Review it before attaching it to an issue.",
          )}
        </p>
        <button
          className="secondary"
          disabled={busy}
          onClick={() =>
            void perform<string>("export_diagnostics").then((result) => {
              if (result.ok) setPath(result.value);
            })
          }
        >
          <Download size={16} aria-hidden="true" />
          {t("Export diagnostics")}
        </button>
        {path && (
          <p className="export-path" role="status">
            {t("Saved to:")} {path}
          </p>
        )}
      </section>
      <section className="repository-section">
        <h2>{t("Open source, and open to ideas.")}</h2>
        <p className="repository-url">{repositoryUrl}</p>
        <button
          className="quiet"
          onClick={() => {
            void navigator.clipboard
              .writeText(repositoryUrl)
              .then(() => {
                setCopied(true);
                setCopyError(false);
              })
              .catch(() => setCopyError(true));
          }}
        >
          <Copy size={15} aria-hidden="true" />
          {copied ? t("Link copied") : t("Copy repository link")}
        </button>
        {copyError && <p role="alert">{t("Select and copy the repository address above.")}</p>}
        <p className="small-note">
          {t(
            "Experimental background process optimization is planned for a later release. This version does not modify processes.",
          )}
        </p>
      </section>
    </>
  );
}
