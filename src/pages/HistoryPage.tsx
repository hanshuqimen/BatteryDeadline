import { useI18n } from "../i18n";
import { useEffect, useState } from "react";
import { CheckCircle2, Clock3 } from "lucide-react";

import type { Perform, Session } from "../types";
const results: Record<string, string> = {
  reached: "Reached",
  stopped: "Stopped",
  interrupted: "Interrupted",
  restarted: "Restarted",
  reserve_reached: "Reserve reached",
  running: "In progress",
  quit: "App closed",
};
export function HistoryPage({ perform, busy }: { perform: Perform; busy: boolean }) {
  const { t, tr, formatNumber, time, day } = useI18n();
  const [sessions, setSessions] = useState<Session[] | null>(null);
  useEffect(() => {
    let alive = true;
    void perform<Session[]>("get_history").then((result) => {
      if (alive && result.ok) setSessions(result.value);
    });
    return () => {
      alive = false;
    };
  }, [perform]);
  return (
    <>
      <header className="page-header">
        <div>
          <h1>{t("Time well spent.")}</h1>
          <p>{t("Your recent battery sessions, stored on this laptop.")}</p>
        </div>
        <Clock3 size={28} aria-hidden="true" />
      </header>
      {!sessions ? (
        <div className="empty-state">
          <h2>{t("Loading your sessions")}</h2>
          <p>{t("Reading local history.")}</p>
          <button
            className="secondary"
            disabled={busy}
            onClick={() =>
              void perform<Session[]>("get_history").then((r) => {
                if (r.ok) setSessions(r.value);
              })
            }
          >
            {t("Retry")}
          </button>
        </div>
      ) : sessions.length === 0 ? (
        <div className="empty-state">
          <Clock3 size={36} aria-hidden="true" />
          <h2>{t("Your next deadline is the first chapter.")}</h2>
          <p>{t("Start a session from Dashboard. Its result will appear here.")}</p>
        </div>
      ) : (
        <div className="history-list">
          {sessions.map((session) => (
            <article className="history-session" key={session.id}>
              <div>
                <strong>{day(session.started_at)}</strong>
                <span className={`result-pill ${session.result === "reached" ? "success" : ""}`}>
                  {session.result === "reached" && <CheckCircle2 size={14} aria-hidden="true" />}
                  {tr(results[session.result] ?? session.result)}
                </span>
              </div>
              <dl>
                <div>
                  <dt>{t("Started")}</dt>
                  <dd>{time(session.started_at)}</dd>
                </div>
                <div>
                  <dt>{t("Needed until")}</dt>
                  <dd>{time(session.deadline)}</dd>
                </div>
                <div>
                  <dt>{t("Battery")}</dt>
                  <dd>
                    {formatNumber(session.start_percentage, 0)}% →{" "}
                    {formatNumber(session.end_percentage, 0)}%
                  </dd>
                </div>
                <div>
                  <dt>{t("Reserve")}</dt>
                  <dd>{session.reserve_percent}%</dd>
                </div>
              </dl>
              {session.estimated_energy_saved_wh !== null && (
                <p>
                  {t("Estimated energy saved:")} {formatNumber(session.estimated_energy_saved_wh)}{" "}
                  {t("Wh")}
                </p>
              )}
            </article>
          ))}
        </div>
      )}
      <p className="small-note history-note">
        {t(
          "Power changes do not prove how much energy was saved. We leave savings blank when the estimate cannot be supported. Session history is retained for 180 days.",
        )}
      </p>
    </>
  );
}
