export type ActuatorKind = "brightness" | "refresh_rate" | "cpu_policy";
export type Preset = "comfort" | "balanced" | "aggressive";
export type ControllerState =
  | "idle"
  | "monitoring"
  | "adjusting"
  | "stable"
  | "at_risk"
  | "critical"
  | "paused_ac"
  | "restoring"
  | "error";
export interface Settings {
  reserve_percent: number;
  preset: Preset;
  min_brightness: number;
  min_refresh_hz: number;
  min_cpu_percent: number;
  allow_brightness: boolean;
  allow_refresh: boolean;
  allow_cpu: boolean;
  resume_on_battery: boolean;
  close_to_tray: boolean;
  theme: "system" | "light" | "dark";
  language: "system" | "en" | "zh-CN" | "ja";
}
export interface Telemetry {
  timestamp: string;
  battery_present: boolean;
  ac_connected: boolean | null;
  charging: boolean;
  percentage: number | null;
  remaining_wh: number | null;
  full_charge_wh: number | null;
  design_wh: number | null;
  discharge_w: number | null;
  voltage_v: number | null;
  os_remaining_minutes: number | null;
  quality: "excellent" | "good" | "estimated" | "poor";
  battery_identity: string;
  explanation: string;
}
export interface Prediction {
  estimated_power_w: number | null;
  power_budget_w: number | null;
  predicted_remaining_minutes: number | null;
  predicted_depletion_time: string | null;
  deadline_margin_minutes: number | null;
  need_to_save_w: number | null;
  confidence: "high" | "medium" | "low" | "warming_up";
  feasibility: "easy" | "possible" | "tight" | "unlikely" | "impossible" | "unknown";
  sample_seconds: number;
  coefficient_of_variation: number | null;
  observed_minimum_w: number | null;
  explanation: string;
}
export interface Capability {
  kind: ActuatorKind;
  target: string;
  supported: boolean;
  current: number | null;
  values: number[];
  explanation: string;
}
export interface Session {
  id: string;
  started_at: string;
  deadline: string;
  ended_at: string | null;
  start_percentage: number | null;
  end_percentage: number | null;
  reserve_percent: number;
  result: string;
  estimated_energy_saved_wh: number | null;
}
export interface Activity {
  id: number;
  timestamp: string;
  title: string;
  detail: string;
  kind: ActuatorKind | null;
  before: number | null;
  after: number | null;
  observed_saving_w: number | null;
}
export interface ChartPoint {
  timestamp: string;
  power_w: number | null;
  budget_w: number | null;
  percentage: number | null;
}
export interface AppSnapshot {
  version: string;
  locale: "en" | "zh-CN" | "ja";
  simulation: boolean;
  state: ControllerState;
  telemetry: Telemetry | null;
  prediction: Prediction;
  session: Session | null;
  settings: Settings;
  capabilities: Capability[];
  activity: Activity[];
  chart: ChartPoint[];
  pending_recovery: number;
  message: string;
}
export type SimulationEvent =
  | "workload_spike"
  | "normal_workload"
  | "plug_ac"
  | "unplug_ac"
  | "missing_rate"
  | "restore_rate"
  | "unsupported_display"
  | "actuator_failure"
  | "impossible_deadline"
  | "manual_brightness"
  | "crash_recovery";
export type CommandName =
  | "get_snapshot"
  | "get_history"
  | "start_session"
  | "stop_session"
  | "restore_settings"
  | "save_settings"
  | "export_diagnostics"
  | "simulation_event"
  | "advance_simulation";
export type ActionResult<T> = { ok: true; value: T } | { ok: false };
export type Perform = <T = null>(
  command: CommandName,
  args?: Record<string, unknown>,
) => Promise<ActionResult<T>>;
