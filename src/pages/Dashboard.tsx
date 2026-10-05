import { useI18n } from "../i18n";
import { useState } from "react";
import {
  Battery,
  CheckCircle2,
  CirclePause,
  Clock3,
  Play,
  PlugZap,
  RotateCcw,
  ShieldCheck,
  Square,
  TriangleAlert,
} from "lucide-react";
import {
  active,
  deadlineError,
  defaultDeadline,
  localInput,
  presetSettings,
  stateLabels,
} from "../lib";
import type { AppSnapshot, Perform, Preset } from "../types";
import { ActivityList } from "../components/ActivityList";
import { PowerChart } from "../components/PowerChart";
import { CapabilityScan } from "../components/CapabilityScan";
import { SimulationPanel } from "../components/SimulationPanel";
export function Dashboard({
  data,
  perform,
  busy,
}: {
  data: AppSnapshot;
  perform: Perform;
  busy: boolean;
}) {
  const { t, tr, formatNumber, time } = useI18n();
  const running = active(data);
  const paused = data.state === "paused_ac";
  const editable = !running || paused;
  const [deadline, setDeadline] = useState(() =>
    running && data.session
      ? localInput(new Date(data.session.deadline))
      : defaultDeadline(data.telemetry?.timestamp),
  );
  const [reserve, setReserve] = useState(data.settings.reserve_percent);
  const [inputError, setInputError] = useState<string | null>(null);
  const [initialNow] = useState(() => Date.now());
  const prediction = data.prediction;
  const margin = prediction.deadline_margin_minutes;
  const warning =
    data.state === "error" ||
    prediction.feasibility === "impossible" ||
    (running && margin !== null && margin < 0);
  const plugged = data.telemetry?.ac_connected === true;
  const finished = !running && data.session?.result === "reached";
  const title = finished
    ? "Goal reached"
    : paused
      ? "Paused and restored"
      : running
        ? tr(stateLabels[data.state])
        : plugged
          ? "Plugged in"
          : "Ready when you are";
  const StatusIcon = warning
    ? TriangleAlert
    : paused
      ? CirclePause
      : finished || data.state === "stable"
        ? CheckCircle2
        : Clock3;
  const now = data.telemetry ? new Date(data.telemetry.timestamp).getTime() : initialNow;
  const draw = prediction.estimated_power_w ?? data.telemetry?.discharge_w;
  const saving = prediction.need_to_save_w;
  const start = async () => {
    const error = deadlineError(deadline, now);
    setInputError(error);
    if (error) return;
    if (!running && reserve !== data.settings.reserve_percent) {
      const saved = await perform("save_settings", {
        settings: { ...data.settings, reserve_percent: reserve },
      });
      if (!saved.ok) return;
    }
    await perform("start_session", { deadline: new Date(deadline).toISOString() });
  };
  const progress =
    running && data.session && prediction.predicted_depletion_time
      ? Math.max(
          5,
          Math.min(
            100,
            ((new Date(prediction.predicted_depletion_time).getTime() - now) /
              Math.max(1, new Date(data.session.deadline).getTime() - now)) *
              85,
          ),
        )
      : 0;
  return (
    <>
      <header className="page-header">
        <div>
          <h1>{t("Make time for your day.")}</h1>
          <p>{t("Enough battery for the time you choose.")}</p>
        </div>
        <div className="battery-reading">
          <Battery size={25} aria-hidden="true" />
          <strong>
            {formatNumber(data.telemetry?.percentage, 0)}
            <span>%</span>
          </strong>
          <small>
            {plugged ? (
              <>
                <PlugZap size={13} aria-hidden="true" />
                {t("Plugged in")}
              </>
            ) : data.telemetry?.ac_connected === false ? (
              t("On battery")
            ) : (
              t("Power status unknown")
            )}
          </small>
        </div>
      </header>
      <section className="deadline-panel" aria-label={t("Battery deadline")}>
        <form
          className="deadline-editor"
          onSubmit={(event) => {
            event.preventDefault();
            void start();
          }}
        >
          <label htmlFor="deadline">{t("Need this laptop until")}</label>
          <input
            id="deadline"
            type="datetime-local"
            value={deadline}
            onChange={(e) => {
              setDeadline(e.target.value);
              setInputError(null);
            }}
            disabled={busy || !editable}
            required
            aria-invalid={Boolean(inputError)}
            aria-describedby="deadline-hint"
          />
          <p id="deadline-hint" className={inputError ? "field-error" : "muted"}>
            {inputError ? tr(inputError) : t("Local time. Choose today or tomorrow.")}
          </p>
          <label className="reserve-control" htmlFor="reserve">
            <ShieldCheck size={16} aria-hidden="true" />
            <span>{t("Keep in reserve")}</span>
            <input
              id="reserve"
              type="number"
              min="5"
              max="30"
              required
              value={running ? data.settings.reserve_percent : reserve}
              onChange={(e) => setReserve(Number(e.target.value))}
              disabled={busy || running}
            />
            <span>%</span>
          </label>
          <p className="reserve-hint">
            {t("Your estimate keeps this battery available at the deadline.")}
          </p>
          {running && !paused ? (
            <button
              className="primary"
              type="button"
              disabled={busy}
              onClick={() => void perform("stop_session")}
            >
              <Square size={15} aria-hidden="true" />
              {busy ? t("Working\u2026") : t("Stop and restore")}
            </button>
          ) : (
            <button
              className="primary"
              type="submit"
              disabled={busy || data.pending_recovery > 0 || !data.telemetry?.battery_present}
            >
              <Play size={16} aria-hidden="true" />
              {busy ? t("Working\u2026") : paused ? t("Start again") : t("Start session")}
            </button>
          )}
          <span className="small-note">{t("Changes are temporary and reversible.")}</span>
        </form>
        <div className={`deadline-outlook ${warning ? "outlook-warning" : ""}`}>
          <div className="status-label" role="status">
            <StatusIcon size={21} aria-hidden="true" />
            {tr(title)}
          </div>
          <div className="expected-time">
            <span>{t("Expected until")}</span>
            <strong>
              {prediction.predicted_depletion_time
                ? time(prediction.predicted_depletion_time)
                : running && !paused
                  ? t("Learning\u2026")
                  : "—"}
            </strong>
          </div>
          <p className="margin">
            {running && margin !== null ? (
              <>
                <strong>
                  {t(
                    margin < 0
                      ? "{minutes} minutes short of your deadline"
                      : "{minutes} minutes ahead of your deadline",
                    { minutes: formatNumber(Math.abs(margin), 0) },
                  )}
                </strong>
              </>
            ) : finished ? (
              t("You made it. Your settings are restored.")
            ) : paused ? (
              t("Power is restored. Start again when you're ready.")
            ) : running ? (
              t("Learning recent usage to estimate your margin.")
            ) : (
              t("Start a session to see whether you're on track.")
            )}
          </p>
          <div className="journey" aria-hidden="true">
            <div className="journey-fill" style={{ width: `${progress}%` }} />
            <span className="journey-start" />
            <span className="journey-deadline" />
          </div>
          <div className="journey-labels">
            <span>{running && data.session ? time(data.session.started_at) : t("Now")}</span>
            <span>
              {running && data.session ? time(data.session.deadline) : t("Your deadline")}
            </span>
          </div>
          <p className="outlook-message">
            {running
              ? tr(data.message)
              : data.telemetry?.battery_present
                ? t("We gently adjust what your device supports, within your comfort limits.")
                : t("No battery was found. You can explore the simulator with --simulate.")}
          </p>
        </div>
      </section>
      <section className="usage-readings" aria-label={t("Power readings")}>
        <div>
          <span>{t("Current usage")}</span>
          <strong>
            {formatNumber(draw)} <small>{t("W")}</small>
          </strong>
        </div>
        <div>
          <span>{t("Required average")}</span>
          <strong>
            {formatNumber(prediction.power_budget_w)} <small>{t("W")}</small>
          </strong>
        </div>
        <div>
          <span>{saving !== null && saving <= 0 ? t("Power headroom") : t("Need to save")}</span>
          <strong className={saving !== null && saving > 0 ? "warning-text" : "positive-text"}>
            {formatNumber(saving === null ? null : Math.abs(saving))} <small>{t("W")}</small>
          </strong>
        </div>
        <div className="confidence">
          <span>{t("Prediction confidence")}</span>
          <strong>
            {prediction.confidence === "warming_up"
              ? t("Learning")
              : tr(prediction.confidence[0]?.toUpperCase() + prediction.confidence.slice(1))}
          </strong>
          <small>{tr(prediction.explanation)}</small>
        </div>
      </section>
      <div className="section-heading">
        <h2>{t("Your comfort comes first.")}</h2>
        <span>
          {running ? t("Stop the session to change limits") : t("Choose how much to adjust")}
        </span>
      </div>
      <div className="preset-options">
        {(["comfort", "balanced", "aggressive"] as Preset[]).map((preset) => (
          <button
            className={data.settings.preset === preset ? "preset selected" : "preset"}
            key={preset}
            aria-pressed={data.settings.preset === preset}
            disabled={busy || running}
            onClick={() =>
              void perform("save_settings", { settings: presetSettings(data.settings, preset) })
            }
          >
            <strong>{tr(preset[0]?.toUpperCase() + preset.slice(1))}</strong>
            <span>
              {preset === "comfort"
                ? t("Light adjustments")
                : preset === "balanced"
                  ? t("An everyday balance")
                  : t("More time, less performance")}
            </span>
          </button>
        ))}
      </div>
      <section className="surface power-section">
        <div className="section-heading">
          <h2>{t("A little less power. A little more time.")}</h2>
          <span>{t("Watts \u00B7 recent measurements")}</span>
        </div>
        <PowerChart points={data.chart} />
      </section>
      <section className="surface activity-section">
        <div className="section-heading">
          <h2>{t("What we're doing")}</h2>
          {data.pending_recovery > 0 && (
            <button
              className="text-button"
              disabled={busy}
              onClick={() => void perform("restore_settings")}
            >
              <RotateCcw size={15} aria-hidden="true" />
              {t("Restore now")}
            </button>
          )}
        </div>
        <ActivityList items={data.activity} />
      </section>
      <CapabilityScan data={data} />
      {data.simulation && <SimulationPanel perform={perform} busy={busy} />}
    </>
  );
}
