//! Documented Windows APIs only. Every mutation has a matching, verified restore operation.
mod battery;
mod brightness;
mod display;
mod power;

use crate::{
    ActuatorKind, Capability, Error, Hardware, RestoreOutcome, Result, Snapshot, Telemetry,
};
use chrono::{DateTime, Utc};

#[derive(Default)]
pub struct WindowsHardware {
    battery: battery::BatteryReader,
}

impl Hardware for WindowsHardware {
    fn is_simulation(&self) -> bool {
        false
    }
    fn telemetry(&mut self, now: DateTime<Utc>) -> Result<Telemetry> {
        self.battery.sample(now)
    }
    fn capabilities(&mut self) -> Vec<Capability> {
        vec![
            brightness::capability(),
            display::capability(),
            power::capability(),
        ]
    }
    fn snapshot(&mut self, cap: &Capability) -> Result<Snapshot> {
        if !cap.supported {
            return Err(Error::Hardware(
                "This system control is unavailable.".into(),
            ));
        }
        match cap.kind {
            ActuatorKind::Brightness => brightness::snapshot(&cap.target),
            ActuatorKind::RefreshRate => display::snapshot(&cap.target),
            ActuatorKind::CpuPolicy => power::snapshot(),
        }
    }
    fn apply(&mut self, s: &Snapshot, value: u32) -> Result<()> {
        // Recheck the source immediately before every system write, including after journaling.
        if battery::system_status()?.ACLineStatus != 0 {
            return Err(Error::Hardware(
                "The power source changed. Settings will be restored.".into(),
            ));
        }
        match s {
            Snapshot::Brightness { target, original } => {
                if brightness::current(target)? != *original {
                    return Err(Error::Hardware("Brightness was changed manually.".into()));
                }
                brightness::set(target, value)
            }
            Snapshot::RefreshRate { .. } => display::apply(s, value),
            Snapshot::CpuPolicy { .. } => power::apply(s, value),
        }
    }
    fn restore(&mut self, s: &Snapshot, applied: u32) -> Result<RestoreOutcome> {
        match s {
            Snapshot::Brightness { target, original } => {
                let current = brightness::current(target)?;
                if current == *original {
                    return Ok(RestoreOutcome::AlreadyOriginal);
                }
                if current != applied {
                    return Ok(RestoreOutcome::UserOverride);
                }
                brightness::set(target, *original)?;
                Ok(RestoreOutcome::Restored)
            }
            Snapshot::RefreshRate { .. } => display::restore(s, applied),
            Snapshot::CpuPolicy { .. } => power::restore(s, applied),
        }
    }
}

fn api_error(context: &str) -> Error {
    let error = std::io::Error::last_os_error();
    tracing::debug!(error=%error,operation=context,"Windows API failed");
    Error::Hardware(context.into())
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
fn from_wide(text: &[u16]) -> String {
    String::from_utf16_lossy(&text[..text.iter().position(|u| *u == 0).unwrap_or(text.len())])
}
