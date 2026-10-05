//! Explicit synthetic hardware using the same engine and recovery journal as production.
use crate::{
    ActuatorKind, Capability, Error, Hardware, RestoreOutcome, Result, SimulationEvent, Snapshot,
    Telemetry, TelemetryQuality,
};
use chrono::{DateTime, Utc};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Debug)]
pub struct FakeState {
    pub remaining_wh: f64,
    pub full_wh: f64,
    pub brightness: u32,
    pub refresh: u32,
    pub cpu: u32,
    pub ac: bool,
    pub missing_rate: bool,
    pub relative_capacity: bool,
    pub unsupported_display: bool,
    pub fail_next_action: bool,
    pub workload_extra_w: f64,
    pub active_plan: String,
    pub plans: HashMap<String, u32>,
    last_sample: Option<DateTime<Utc>>,
}

impl Default for FakeState {
    fn default() -> Self {
        Self {
            remaining_wh: 39.0,
            full_wh: 60.0,
            brightness: 75,
            refresh: 120,
            cpu: 100,
            ac: false,
            missing_rate: false,
            relative_capacity: false,
            unsupported_display: false,
            fail_next_action: false,
            workload_extra_w: 0.0,
            active_plan: "simulation-original-plan".into(),
            plans: HashMap::from([("simulation-original-plan".into(), 100)]),
            last_sample: None,
        }
    }
}

impl FakeState {
    pub fn power(&self) -> f64 {
        (13.0 + self.workload_extra_w
            - (75.0 - self.brightness as f64) * 0.08
            - (120.0 - self.refresh as f64) * 0.02
            - (100.0 - self.cpu as f64) * 0.14)
            .max(3.0)
    }
}

#[derive(Clone, Default)]
pub struct FakeHardware {
    pub state: Arc<Mutex<FakeState>>,
}

impl FakeHardware {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, FakeState>> {
        self.state
            .lock()
            .map_err(|_| Error::Hardware("Simulation hardware is unavailable.".into()))
    }
}

impl Hardware for FakeHardware {
    fn is_simulation(&self) -> bool {
        true
    }
    fn telemetry(&mut self, now: DateTime<Utc>) -> Result<Telemetry> {
        let mut s = self.lock()?;
        if let Some(last) = s.last_sample {
            let seconds = (now - last).num_milliseconds().max(0) as f64 / 1000.0;
            if !s.ac {
                s.remaining_wh = (s.remaining_wh - s.power() * seconds / 3600.0).max(0.0);
            }
        }
        s.last_sample = Some(now);
        Ok(Telemetry {
            timestamp: now,
            battery_present: true,
            ac_connected: Some(s.ac),
            charging: s.ac,
            percentage: Some(s.remaining_wh / s.full_wh * 100.0),
            remaining_wh: (!s.relative_capacity).then_some(s.remaining_wh),
            full_charge_wh: (!s.relative_capacity).then_some(s.full_wh),
            design_wh: Some(68.0),
            discharge_w: (!s.ac && !s.missing_rate && !s.relative_capacity).then_some(s.power()),
            voltage_v: Some(11.8),
            os_remaining_minutes: None,
            quality: if s.relative_capacity {
                TelemetryQuality::Poor
            } else if s.missing_rate {
                TelemetryQuality::Good
            } else {
                TelemetryQuality::Excellent
            },
            battery_identity: "simulation-battery-v1".into(),
            explanation: "Synthetic battery. No Windows settings are changed.".into(),
        })
    }
    fn capabilities(&mut self) -> Vec<Capability> {
        let Ok(s) = self.lock() else { return vec![] };
        vec![
            Capability {
                kind: ActuatorKind::Brightness,
                target: "simulation-panel".into(),
                supported: true,
                current: Some(s.brightness),
                values: vec![],
                explanation: "Simulated internal panel".into(),
            },
            Capability {
                kind: ActuatorKind::RefreshRate,
                target: "simulation-display".into(),
                supported: !s.unsupported_display,
                current: Some(s.refresh),
                values: if s.unsupported_display {
                    vec![]
                } else {
                    vec![60, 120]
                },
                explanation: if s.unsupported_display {
                    "No supported refresh-rate control"
                } else {
                    "Simulated 60 / 120 Hz display"
                }
                .into(),
            },
            Capability {
                kind: ActuatorKind::CpuPolicy,
                target: s.active_plan.clone(),
                supported: true,
                current: Some(s.cpu),
                values: vec![],
                explanation: "Simulated reversible plan".into(),
            },
        ]
    }
    fn snapshot(&mut self, cap: &Capability) -> Result<Snapshot> {
        let s = self.lock()?;
        Ok(match cap.kind {
            ActuatorKind::Brightness => Snapshot::Brightness {
                target: cap.target.clone(),
                original: s.brightness,
            },
            ActuatorKind::RefreshRate => Snapshot::RefreshRate {
                target: cap.target.clone(),
                original: s.refresh,
                monitor_id: "simulation-monitor".into(),
                width: 1920,
                height: 1080,
                bits_per_pixel: 32,
                orientation: 0,
            },
            ActuatorKind::CpuPolicy => Snapshot::CpuPolicy {
                target: s.active_plan.clone(),
                original: s.cpu,
                temporary_guid: uuid::Uuid::new_v4().to_string(),
            },
        })
    }
    fn apply(&mut self, snapshot: &Snapshot, value: u32) -> Result<()> {
        let mut s = self.lock()?;
        if s.fail_next_action {
            s.fail_next_action = false;
            return Err(Error::Hardware(
                "The simulated actuator refused this change.".into(),
            ));
        }
        if s.ac {
            return Err(Error::Hardware(
                "Connected to power. No adjustment was applied.".into(),
            ));
        }
        match snapshot {
            Snapshot::Brightness { original, .. } => {
                if s.brightness != *original || value > 100 {
                    return Err(Error::Hardware(
                        "Brightness changed outside the controller.".into(),
                    ));
                }
                s.brightness = value;
            }
            Snapshot::RefreshRate { original, .. } => {
                if s.unsupported_display || s.refresh != *original || ![60, 120].contains(&value) {
                    return Err(Error::Hardware("The display mode is unavailable.".into()));
                }
                s.refresh = value;
            }
            Snapshot::CpuPolicy {
                target,
                temporary_guid,
                ..
            } => {
                if &s.active_plan != target {
                    return Err(Error::Hardware(
                        "The power plan was changed manually.".into(),
                    ));
                }
                s.plans.insert(temporary_guid.clone(), value);
                s.active_plan = temporary_guid.clone();
                s.cpu = value;
            }
        }
        Ok(())
    }
    fn restore(&mut self, snapshot: &Snapshot, applied: u32) -> Result<RestoreOutcome> {
        let mut s = self.lock()?;
        match snapshot {
            Snapshot::Brightness { original, .. } => {
                if s.brightness == *original {
                    return Ok(RestoreOutcome::AlreadyOriginal);
                }
                if s.brightness != applied {
                    return Ok(RestoreOutcome::UserOverride);
                }
                s.brightness = *original;
            }
            Snapshot::RefreshRate { original, .. } => {
                if s.unsupported_display {
                    return Err(Error::Hardware(
                        "Reconnect the simulated display to restore its refresh rate.".into(),
                    ));
                }
                if s.refresh == *original {
                    return Ok(RestoreOutcome::AlreadyOriginal);
                }
                if s.refresh != applied {
                    return Ok(RestoreOutcome::UserOverride);
                }
                s.refresh = *original;
            }
            Snapshot::CpuPolicy {
                target,
                original,
                temporary_guid,
            } => {
                let outcome = if s.active_plan == *temporary_guid {
                    s.active_plan = target.clone();
                    s.cpu = *original;
                    RestoreOutcome::Restored
                } else if s.active_plan == *target {
                    RestoreOutcome::AlreadyOriginal
                } else {
                    RestoreOutcome::UserOverride
                };
                s.plans.remove(temporary_guid);
                return Ok(outcome);
            }
        }
        Ok(RestoreOutcome::Restored)
    }
    fn simulation_event(&mut self, event: SimulationEvent) -> Result<()> {
        let mut s = self.lock()?;
        match event {
            SimulationEvent::WorkloadSpike => s.workload_extra_w = 8.0,
            SimulationEvent::NormalWorkload => s.workload_extra_w = 0.0,
            SimulationEvent::PlugAc => s.ac = true,
            SimulationEvent::UnplugAc => s.ac = false,
            SimulationEvent::MissingRate => s.missing_rate = true,
            SimulationEvent::RestoreRate => s.missing_rate = false,
            SimulationEvent::UnsupportedDisplay => s.unsupported_display = !s.unsupported_display,
            SimulationEvent::ActuatorFailure => s.fail_next_action = true,
            SimulationEvent::ImpossibleDeadline => s.remaining_wh = 4.0,
            SimulationEvent::ManualBrightness => s.brightness = 85,
            SimulationEvent::CrashRecovery => {}
        }
        Ok(())
    }
}
