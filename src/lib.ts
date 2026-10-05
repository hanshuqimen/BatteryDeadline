import type {
  ActuatorKind,
  AppSnapshot,
  ChartPoint,
  ControllerState,
  Preset,
  Settings,
} from "./types";

export const repositoryUrl = "https://github.com/hanshuqimen/BatteryDeadline";
export const actuatorLabels: Record<ActuatorKind, string> = {
  brightness: "Screen brightness",
  refresh_rate: "Display smoothness",
  cpu_policy: "Processor limit",
};
export const stateLabels: Record<ControllerState, string> = {
  idle: "Ready when you are",
  monitoring: "Learning your usage",
  adjusting: "Gently adjusting",
  stable: "On track",
  at_risk: "Deadline at risk",
  critical: "Deadline unlikely",
  paused_ac: "Paused",
  restoring: "Restoring your settings",
  error: "Restore needed",
};
export function formatNumber(value: number | null | undefined, digits = 1): string {
  return value == null || !Number.isFinite(value)
    ? "—"
    : value.toLocaleString(undefined, {
        minimumFractionDigits: digits,
        maximumFractionDigits: digits,
      });
}
export function time(value: string | null | undefined): string {
  return value
    ? new Date(value).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })
    : "—";
}
export function day(value: string): string {
  return new Date(value).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}
export function active(data: AppSnapshot): boolean {
  return data.session !== null && data.session.ended_at === null;
}
export function localInput(date: Date): string {
  const part = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${part(date.getMonth() + 1)}-${part(date.getDate())}T${part(date.getHours())}:${part(date.getMinutes())}`;
}
export function defaultDeadline(timestamp?: string): string {
  return localInput(new Date(new Date(timestamp ?? Date.now()).getTime() + 4 * 3_600_000));
}
export function deadlineError(input: string, now: number): string | null {
  const timestamp = new Date(input).getTime();
  if (!Number.isFinite(timestamp)) return "Choose a valid date and time.";
  if (timestamp < now + 60_000 || timestamp > now + 48 * 3_600_000)
    return "Choose a time between one minute and 48 hours from now.";
  return null;
}
export function presetSettings(settings: Settings, preset: Preset): Settings {
  const limits = {
    comfort: [60, 60, 90],
    balanced: [45, 60, 80],
    aggressive: [30, 60, 65],
  } as const;
  const [min_brightness, min_refresh_hz, min_cpu_percent] = limits[preset];
  return { ...settings, preset, min_brightness, min_refresh_hz, min_cpu_percent };
}
/** Missing measurements split the path instead of fabricating a continuous line. */
export function chartPath(
  points: ChartPoint[],
  field: "power_w" | "budget_w",
  maximum: number,
): string {
  const first = points[0];
  const last = points.at(-1);
  if (!first || !last) return "";
  const start = new Date(first.timestamp).getTime();
  const span = Math.max(1, new Date(last.timestamp).getTime() - start);
  let drawing = false;
  return points
    .map((point) => {
      const value = point[field];
      if (value === null || !Number.isFinite(value)) {
        drawing = false;
        return "";
      }
      const x = 30 + ((new Date(point.timestamp).getTime() - start) / span) * 640;
      const y = 140 - (Math.min(maximum, Math.max(0, value)) / maximum) * 110;
      const result = `${drawing ? "L" : "M"}${x.toFixed(1)},${y.toFixed(1)}`;
      drawing = true;
      return result;
    })
    .join(" ");
}
