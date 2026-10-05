use crate::{Capability, RestoreOutcome, Result, SimulationEvent, Snapshot, Telemetry};
use chrono::{DateTime, Utc};

#[cfg(windows)]
pub mod windows;

/// Platform adapters own device discovery, support checks, and post-write verification.
/// The engine owns the write-ahead journal and never calls apply before it is durable.
pub trait Hardware: Send {
    fn is_simulation(&self) -> bool;
    fn telemetry(&mut self, now: DateTime<Utc>) -> Result<Telemetry>;
    fn capabilities(&mut self) -> Vec<Capability>;
    fn snapshot(&mut self, capability: &Capability) -> Result<Snapshot>;
    fn apply(&mut self, snapshot: &Snapshot, value: u32) -> Result<()>;
    fn restore(&mut self, snapshot: &Snapshot, applied: u32) -> Result<RestoreOutcome>;
    fn simulation_event(&mut self, _event: SimulationEvent) -> Result<()> {
        Err(crate::Error::InvalidInput(
            "Simulation controls are available only in simulation mode.".into(),
        ))
    }
}
