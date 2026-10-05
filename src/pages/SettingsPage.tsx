import { useI18n } from "../i18n";
import { useState } from "react";
import { Check, ShieldCheck } from "lucide-react";
import { active, presetSettings } from "../lib";
import type { AppSnapshot, Perform, Preset, Settings } from "../types";
export function SettingsPage({
  data,
  perform,
  busy,
}: {
  data: AppSnapshot;
  perform: Perform;
  busy: boolean;
}) {
  const { t, tr } = useI18n();
  const [draft, setDraft] = useState<Settings>(data.settings);
  const [saved, setSaved] = useState(false);
  const running = active(data);
  const available = (kind: "brightness" | "refresh_rate" | "cpu_policy") =>
    data.capabilities.some((c) => c.kind === kind && c.supported);
  const update = <K extends keyof Settings>(key: K, value: Settings[K]) => {
    setDraft({ ...draft, [key]: value });
    setSaved(false);
  };
  return (
    <>
      <header className="page-header">
        <div>
          <h1>{t("Comfort, on your terms.")}</h1>
          <p>{t("Set the limits BatteryDeadline should respect.")}</p>
        </div>
        <ShieldCheck size={28} aria-hidden="true" />
      </header>
      {running && (
        <p className="notice">
          {t(
            "Stop your current session to change its limits. Your current settings remain in effect.",
          )}
        </p>
      )}
      <section className="settings-fieldset">
        <label className="theme-label">
          {t("Language")}
          <select
            value={data.settings.language}
            disabled={busy}
            onChange={(event) => {
              const language = event.target.value as Settings["language"];
              void perform("save_settings", { settings: { ...data.settings, language } }).then(
                (result) => {
                  if (result.ok) setDraft((current) => ({ ...current, language }));
                },
              );
            }}
          >
            <option value="system">{t("Follow Windows language")}</option>
            <option value="zh-CN">简体中文</option>
            <option value="en">English</option>
            <option value="ja">日本語</option>
          </select>
        </label>
        <p className="small-note">
          {t("Language changes apply immediately and do not interrupt your session.")}
        </p>
      </section>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void perform("save_settings", { settings: draft }).then((result) => setSaved(result.ok));
        }}
      >
        <fieldset disabled={busy || running} className="settings-fieldset">
          <legend>{t("Comfort limits")}</legend>
          <div className="preset-options">
            {(["comfort", "balanced", "aggressive"] as Preset[]).map((preset) => (
              <button
                type="button"
                key={preset}
                aria-pressed={draft.preset === preset}
                className={draft.preset === preset ? "preset selected" : "preset"}
                onClick={() => {
                  setDraft(presetSettings(draft, preset));
                  setSaved(false);
                }}
              >
                <strong>{tr(preset[0]?.toUpperCase() + preset.slice(1))}</strong>
                <span>
                  {preset === "comfort"
                    ? t("Brightness \u226560%, CPU \u226590%")
                    : preset === "balanced"
                      ? t("Brightness \u226545%, CPU \u226580%")
                      : t("Brightness \u226530%, CPU \u226565%")}
                </span>
              </button>
            ))}
          </div>
          <div className="settings-grid">
            <label>
              {t("Battery reserve")}{" "}
              <div className="number-unit">
                <input
                  type="number"
                  min="5"
                  max="30"
                  required
                  value={draft.reserve_percent}
                  onChange={(e) => update("reserve_percent", Number(e.target.value))}
                />
                <span>%</span>
              </div>
              <small>{t("Keep 5\u201330% available at your deadline.")}</small>
            </label>
            <label>
              {t("Minimum brightness")}{" "}
              <div className="number-unit">
                <input
                  type="number"
                  min="20"
                  max="100"
                  required
                  value={draft.min_brightness}
                  onChange={(e) => update("min_brightness", Number(e.target.value))}
                />
                <span>%</span>
              </div>
              <small>{t("Never dim below this while adjusting.")}</small>
            </label>
            <label>
              {t("Minimum refresh rate")}{" "}
              <div className="number-unit">
                <input
                  type="number"
                  min="30"
                  max="360"
                  required
                  value={draft.min_refresh_hz}
                  onChange={(e) => update("min_refresh_hz", Number(e.target.value))}
                />
                <span>Hz</span>
              </div>
              <small>{t("Uses only legal rates your display supports.")}</small>
            </label>
            <label>
              {t("Minimum processor limit")}{" "}
              <div className="number-unit">
                <input
                  type="number"
                  min="65"
                  max="100"
                  required
                  value={draft.min_cpu_percent}
                  onChange={(e) => update("min_cpu_percent", Number(e.target.value))}
                />
                <span>%</span>
              </div>
              <small>{t("Applies to battery power on a temporary plan.")}</small>
            </label>
          </div>
          <div className="toggle-list">
            <label>
              <input
                type="checkbox"
                checked={draft.allow_brightness}
                disabled={!available("brightness")}
                onChange={(e) => update("allow_brightness", e.target.checked)}
              />
              <span>
                {t("Allow screen brightness adjustments")}
                <small>
                  {available("brightness")
                    ? t("Your manual changes always take priority.")
                    : t("Unavailable on this display.")}
                </small>
              </span>
            </label>
            <label>
              <input
                type="checkbox"
                checked={draft.allow_refresh}
                disabled={!available("refresh_rate")}
                onChange={(e) => update("allow_refresh", e.target.checked)}
              />
              <span>
                {t("Allow refresh rate adjustments")}
                <small>
                  {available("refresh_rate")
                    ? t("Internal display only. HDR displays are excluded.")
                    : t("Unavailable in the current display configuration.")}
                </small>
              </span>
            </label>
            <label>
              <input
                type="checkbox"
                checked={draft.allow_cpu}
                disabled={!available("cpu_policy")}
                onChange={(e) => update("allow_cpu", e.target.checked)}
              />
              <span>
                {t("Allow processor power limits")}
                <small>
                  {available("cpu_policy")
                    ? t("Your original power plan is preserved.")
                    : t("Unavailable under the current Windows power policy.")}
                </small>
              </span>
            </label>
          </div>
        </fieldset>
        <fieldset disabled={busy || running} className="settings-fieldset">
          <legend>{t("When plans change")}</legend>
          <div className="toggle-list">
            <label>
              <input
                type="checkbox"
                checked={draft.resume_on_battery}
                onChange={(e) => update("resume_on_battery", e.target.checked)}
              />
              <span>
                {t("Resume automatically after unplugging")}
                <small>
                  {t("Off by default. A new estimate warms up before adjustments resume.")}
                </small>
              </span>
            </label>
            <label>
              <input
                type="checkbox"
                checked={draft.close_to_tray}
                onChange={(e) => update("close_to_tray", e.target.checked)}
              />
              <span>
                {t("Keep running when the window is closed")}
                <small>{t("Use Quit in the system tray to restore settings and exit.")}</small>
              </span>
            </label>
          </div>
          <p className="safety-note">
            {t(
              "Settings always restore when you stop, plug in, or reach the deadline. Recovery runs automatically after an interrupted session.",
            )}
          </p>
        </fieldset>
        <fieldset disabled={busy || running} className="settings-fieldset">
          <legend>{t("Appearance")}</legend>
          <label className="theme-label">
            {t("Theme")}
            <select
              value={draft.theme}
              onChange={(e) => update("theme", e.target.value as Settings["theme"])}
            >
              <option value="system">{t("Follow Windows")}</option>
              <option value="light">{t("Light")}</option>
              <option value="dark">{t("Dark")}</option>
            </select>
          </label>
        </fieldset>
        <div className="form-actions">
          <button className="primary" type="submit" disabled={busy || running}>
            {busy ? t("Saving\u2026") : t("Save settings")}
          </button>
          {saved && (
            <span className="saved-feedback" role="status">
              <Check size={16} aria-hidden="true" />
              {t("Settings saved")}
            </span>
          )}
        </div>
      </form>
    </>
  );
}
