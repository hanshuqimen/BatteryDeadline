import { useI18n } from "../i18n";
import { FlaskConical, FastForward } from "lucide-react";
import type { Perform, SimulationEvent } from "../types";
const scenarios: {
  event: SimulationEvent;
  label: string;
}[] = [
  { event: "workload_spike", label: "Heavy workload" },
  { event: "normal_workload", label: "Normal workload" },
  { event: "plug_ac", label: "Plug in" },
  { event: "unplug_ac", label: "Unplug" },
  { event: "missing_rate", label: "Power rate unavailable" },
  { event: "restore_rate", label: "Restore power rate" },
  { event: "manual_brightness", label: "Change brightness manually" },
  { event: "unsupported_display", label: "Toggle display availability" },
  { event: "actuator_failure", label: "Fail next adjustment" },
  { event: "impossible_deadline", label: "Battery below reserve" },
  { event: "crash_recovery", label: "Rehearse recovery" },
];
export function SimulationPanel({ perform, busy }: { perform: Perform; busy: boolean }) {
  const { t, tr } = useI18n();
  return (
    <details className="simulation-panel">
      <summary>
        <FlaskConical size={18} aria-hidden="true" />
        {t("Simulation controls")}
      </summary>
      <p>
        {t(
          "These scenarios run the real Rust controller on synthetic hardware. Windows settings stay untouched.",
        )}
      </p>
      <button
        className="secondary"
        disabled={busy}
        onClick={() => void perform("advance_simulation", { seconds: 300 })}
      >
        <FastForward size={16} aria-hidden="true" />
        {t("Advance 5 minutes")}
      </button>
      <div className="scenario-buttons">
        {scenarios.map(({ event, label }) => (
          <button
            className="quiet"
            key={event}
            disabled={busy}
            onClick={() => void perform("simulation_event", { event })}
          >
            {tr(label)}
          </button>
        ))}
      </div>
    </details>
  );
}
