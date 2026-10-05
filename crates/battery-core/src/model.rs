use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryQuality {
    Excellent,
    Good,
    Estimated,
    Poor,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Telemetry {
    pub timestamp: DateTime<Utc>,
    pub battery_present: bool,
    pub ac_connected: Option<bool>,
    pub charging: bool,
    pub percentage: Option<f64>,
    pub remaining_wh: Option<f64>,
    pub full_charge_wh: Option<f64>,
    pub design_wh: Option<f64>,
    pub discharge_w: Option<f64>,
    pub voltage_v: Option<f64>,
    pub os_remaining_minutes: Option<f64>,
    pub quality: TelemetryQuality,
    pub battery_identity: String,
    pub explanation: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Medium,
    Low,
    WarmingUp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feasibility {
    Easy,
    Possible,
    Tight,
    Unlikely,
    Impossible,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Prediction {
    pub estimated_power_w: Option<f64>,
    pub power_budget_w: Option<f64>,
    pub predicted_remaining_minutes: Option<f64>,
    /// Time at which the configured reserve is reached, not time to zero.
    pub predicted_depletion_time: Option<DateTime<Utc>>,
    pub deadline_margin_minutes: Option<f64>,
    pub need_to_save_w: Option<f64>,
    pub confidence: Confidence,
    pub feasibility: Feasibility,
    pub sample_seconds: i64,
    pub coefficient_of_variation: Option<f64>,
    pub observed_minimum_w: Option<f64>,
    pub explanation: String,
}

impl Default for Prediction {
    fn default() -> Self {
        Self {
            estimated_power_w: None,
            power_budget_w: None,
            predicted_remaining_minutes: None,
            predicted_depletion_time: None,
            deadline_margin_minutes: None,
            need_to_save_w: None,
            confidence: Confidence::WarmingUp,
            feasibility: Feasibility::Unknown,
            sample_seconds: 0,
            coefficient_of_variation: None,
            observed_minimum_w: None,
            explanation: "Collecting a stable battery reading.".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    Comfort,
    Balanced,
    Aggressive,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub reserve_percent: u8,
    pub preset: Preset,
    pub min_brightness: u8,
    pub min_refresh_hz: u32,
    pub min_cpu_percent: u8,
    pub allow_brightness: bool,
    pub allow_refresh: bool,
    pub allow_cpu: bool,
    pub resume_on_battery: bool,
    pub close_to_tray: bool,
    pub theme: String,
    pub language: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            reserve_percent: 10,
            preset: Preset::Balanced,
            min_brightness: 45,
            min_refresh_hz: 60,
            min_cpu_percent: 80,
            allow_brightness: true,
            allow_refresh: true,
            allow_cpu: true,
            resume_on_battery: false,
            close_to_tray: true,
            theme: "system".into(),
            language: "system".into(),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> crate::Result<()> {
        if !(5..=30).contains(&self.reserve_percent)
            || !(20..=100).contains(&self.min_brightness)
            || !(30..=360).contains(&self.min_refresh_hz)
            || !(65..=100).contains(&self.min_cpu_percent)
            || !["system", "light", "dark"].contains(&self.theme.as_str())
            || !["system", "en", "zh-CN", "ja"].contains(&self.language.as_str())
        {
            return Err(crate::Error::InvalidInput("Check the reserve and comfort limits. Reserve must be 5–30%, brightness 20–100%, refresh 30–360 Hz, and CPU 65–100%.".into()));
        }
        Ok(())
    }
    pub fn apply_preset(&mut self, preset: Preset) {
        self.preset = preset;
        (
            self.min_brightness,
            self.min_refresh_hz,
            self.min_cpu_percent,
        ) = match preset {
            Preset::Comfort => (60, 60, 90),
            Preset::Balanced => (45, 60, 80),
            Preset::Aggressive => (30, 60, 65),
        };
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActuatorKind {
    Brightness,
    RefreshRate,
    CpuPolicy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Capability {
    pub kind: ActuatorKind,
    pub target: String,
    pub supported: bool,
    pub current: Option<u32>,
    pub values: Vec<u32>,
    pub explanation: String,
}

/// All identity and original state needed to undo a pending operation after a crash.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Snapshot {
    Brightness {
        target: String,
        original: u32,
    },
    RefreshRate {
        target: String,
        original: u32,
        monitor_id: String,
        width: u32,
        height: u32,
        bits_per_pixel: u32,
        orientation: u32,
    },
    CpuPolicy {
        target: String,
        original: u32,
        temporary_guid: String,
    },
}

impl Snapshot {
    pub fn kind(&self) -> ActuatorKind {
        match self {
            Self::Brightness { .. } => ActuatorKind::Brightness,
            Self::RefreshRate { .. } => ActuatorKind::RefreshRate,
            Self::CpuPolicy { .. } => ActuatorKind::CpuPolicy,
        }
    }
    pub fn target(&self) -> &str {
        match self {
            Self::Brightness { target, .. }
            | Self::RefreshRate { target, .. }
            | Self::CpuPolicy { target, .. } => target,
        }
    }
    pub fn original(&self) -> u32 {
        match self {
            Self::Brightness { original, .. }
            | Self::RefreshRate { original, .. }
            | Self::CpuPolicy { original, .. } => *original,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreOutcome {
    Restored,
    AlreadyOriginal,
    UserOverride,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControllerState {
    Idle,
    Monitoring,
    Adjusting,
    Stable,
    AtRisk,
    Critical,
    PausedAc,
    Restoring,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub started_at: DateTime<Utc>,
    pub deadline: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub start_percentage: Option<f64>,
    pub end_percentage: Option<f64>,
    pub reserve_percent: u8,
    pub result: String,
    /// Only populated when a defensible counterfactual exists. Never fabricated.
    pub estimated_energy_saved_wh: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Activity {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub title: String,
    pub detail: String,
    pub kind: Option<ActuatorKind>,
    pub before: Option<u32>,
    pub after: Option<u32>,
    pub observed_saving_w: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChartPoint {
    pub timestamp: DateTime<Utc>,
    pub power_w: Option<f64>,
    pub budget_w: Option<f64>,
    pub percentage: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppSnapshot {
    pub version: String,
    pub locale: String,
    pub simulation: bool,
    pub state: ControllerState,
    pub telemetry: Option<Telemetry>,
    pub prediction: Prediction,
    pub session: Option<Session>,
    pub settings: Settings,
    pub capabilities: Vec<Capability>,
    pub activity: Vec<Activity>,
    pub chart: Vec<ChartPoint>,
    pub pending_recovery: usize,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationEvent {
    WorkloadSpike,
    NormalWorkload,
    PlugAc,
    UnplugAc,
    MissingRate,
    RestoreRate,
    UnsupportedDisplay,
    ActuatorFailure,
    ImpossibleDeadline,
    ManualBrightness,
    CrashRecovery,
}
