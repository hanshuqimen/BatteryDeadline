use crate::{
    Activity, ActuatorKind, AppSnapshot, ChartPoint, ControllerState, Error, Hardware, Prediction,
    RestoreOutcome, Result, Session, Settings, SimulationEvent,
};
use crate::{controller::Controller, estimator::Estimator, storage::Storage};
use chrono::{DateTime, Duration, Utc};
use std::{
    collections::{HashMap, VecDeque},
    io::Write,
    path::{Path, PathBuf},
};

struct Observation {
    kind: ActuatorKind,
    target: String,
    at: DateTime<Utc>,
    before: f64,
    before_value: u32,
    after_value: u32,
}

pub struct Engine {
    hardware: Box<dyn Hardware>,
    storage: Storage,
    estimator: Estimator,
    controller: Controller,
    settings: Settings,
    state: ControllerState,
    telemetry: Option<crate::Telemetry>,
    prediction: Prediction,
    session: Option<Session>,
    capabilities: Vec<crate::Capability>,
    baselines: HashMap<ActuatorKind, u32>,
    expected: HashMap<ActuatorKind, (String, u32)>,
    effects: HashMap<ActuatorKind, f64>,
    activity: VecDeque<Activity>,
    chart: VecDeque<ChartPoint>,
    last_tick: Option<DateTime<Utc>>,
    last_scan: Option<DateTime<Utc>>,
    last_sample: Option<DateTime<Utc>>,
    last_retention: DateTime<Utc>,
    observation: Option<Observation>,
    message: String,
}

impl Engine {
    pub fn open(
        mut hardware: Box<dyn Hardware>,
        directory: &Path,
        now: DateTime<Utc>,
    ) -> Result<Self> {
        let storage = Storage::open(directory, hardware.is_simulation())?;
        let settings = storage.settings()?;
        let capabilities = hardware.capabilities();
        let effects = storage.effects(&capabilities)?;
        let mut engine = Self {
            hardware,
            storage,
            estimator: Estimator::default(),
            controller: Controller::default(),
            settings,
            state: ControllerState::Idle,
            telemetry: None,
            prediction: Prediction::default(),
            session: None,
            capabilities,
            baselines: HashMap::new(),
            expected: HashMap::new(),
            effects,
            activity: VecDeque::new(),
            chart: VecDeque::new(),
            last_tick: None,
            last_scan: None,
            last_sample: None,
            last_retention: now,
            observation: None,
            message: "Choose when you need your laptop until.".into(),
        };
        if !engine.storage.pending()?.is_empty() {
            engine.message = "Recovering settings from an interrupted session.".into();
            if let Err(error) = engine.restore_all(now) {
                engine.state = ControllerState::Error;
                engine.message = error.to_string();
            } else {
                engine.message = "Recovered the previous session's settings.".into();
            }
        }
        engine.storage.close_interrupted_sessions(now)?;
        engine.telemetry = Some(engine.hardware.telemetry(now)?);
        engine.capabilities = engine.hardware.capabilities();
        engine.activity = engine.storage.recent_activity(None)?.into();
        Ok(engine)
    }

    pub fn snapshot(&self) -> Result<AppSnapshot> {
        Ok(AppSnapshot {
            version: env!("CARGO_PKG_VERSION").into(),
            locale: crate::locale::resolve(&self.settings.language).into(),
            simulation: self.hardware.is_simulation(),
            state: self.state,
            telemetry: self.telemetry.clone(),
            prediction: self.prediction.clone(),
            session: self.session.clone(),
            settings: self.settings.clone(),
            capabilities: self.capabilities.clone(),
            activity: self.activity.iter().cloned().collect(),
            chart: self.chart.iter().cloned().collect(),
            pending_recovery: self.storage.pending()?.len(),
            message: self.message.clone(),
        })
    }
    pub fn history(&self) -> Result<Vec<Session>> {
        self.storage.history()
    }
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn save_settings(&mut self, settings: Settings) -> Result<()> {
        settings.validate()?;
        // Presentation preferences do not alter a running session's control contract.
        let mut session_contract = self.settings.clone();
        session_contract.language = settings.language.clone();
        session_contract.theme = settings.theme.clone();
        session_contract.close_to_tray = settings.close_to_tray;
        if self.session.as_ref().is_some_and(|s| s.ended_at.is_none())
            && session_contract != settings
        {
            return Err(Error::InvalidInput(
                "Stop the session before changing its comfort limits.".into(),
            ));
        }
        self.storage.save_settings(&settings)?;
        self.settings = settings;
        Ok(())
    }

    pub fn start(&mut self, deadline: DateTime<Utc>, now: DateTime<Utc>) -> Result<()> {
        if deadline < now + Duration::minutes(1) || deadline > now + Duration::hours(48) {
            return Err(Error::InvalidInput(
                "Choose a deadline between one minute and 48 hours from now.".into(),
            ));
        }
        if !self.storage.pending()?.is_empty() && self.state == ControllerState::Error {
            self.restore_all(now)?;
        }
        if self.session.as_ref().is_some_and(|s| s.ended_at.is_none()) {
            if self.state == ControllerState::PausedAc {
                self.stop("restarted", now)?;
            } else {
                return Err(Error::InvalidInput(
                    "A session is already running. Stop it before choosing another deadline."
                        .into(),
                ));
            }
        }
        if !self.storage.pending()?.is_empty() {
            return Err(Error::Hardware(
                "Restore pending system settings before starting another session.".into(),
            ));
        }
        let t = self.hardware.telemetry(now)?;
        if !t.battery_present {
            return Err(Error::InvalidInput(
                "No laptop battery was found. Simulation is available with --simulate.".into(),
            ));
        }
        if t.ac_connected.is_none() {
            return Err(Error::InvalidInput(
                "The power source is unknown. Wait for a reliable reading before starting.".into(),
            ));
        }
        if t.percentage
            .is_some_and(|p| p <= self.settings.reserve_percent as f64)
        {
            return Err(Error::InvalidInput("The battery is already at or below your reserve. Connect power or lower the reserve.".into()));
        }
        self.estimator.reset();
        self.controller.reset();
        self.chart.clear();
        self.activity.clear();
        self.observation = None;
        self.expected.clear();
        self.capabilities = self.hardware.capabilities();
        self.baselines = self
            .capabilities
            .iter()
            .filter_map(|c| c.current.map(|v| (c.kind, v)))
            .collect();
        let s = Session {
            id: uuid::Uuid::new_v4().to_string(),
            started_at: now,
            deadline,
            ended_at: None,
            start_percentage: t.percentage,
            end_percentage: None,
            reserve_percent: self.settings.reserve_percent,
            result: "running".into(),
            estimated_energy_saved_wh: None,
        };
        self.storage.save_session(&s)?;
        self.session = Some(s);
        self.telemetry = Some(t);
        self.state = if self
            .telemetry
            .as_ref()
            .is_some_and(|t| t.ac_connected == Some(true))
        {
            ControllerState::PausedAc
        } else {
            ControllerState::Monitoring
        };
        self.message = if self.state == ControllerState::PausedAc {
            "Plugged in. Unplug and start to begin a battery session."
        } else {
            "Learning your recent usage before making adjustments."
        }
        .into();
        self.log(
            now,
            "Session started",
            &format!(
                "Target: {} UTC, keeping {}% in reserve.",
                deadline.format("%Y-%m-%d %H:%M"),
                self.settings.reserve_percent
            ),
            None,
            None,
            None,
            None,
        )?;
        Ok(())
    }

    pub fn stop(&mut self, result: &str, now: DateTime<Utc>) -> Result<()> {
        self.state = ControllerState::Restoring;
        if let Err(error) = self.restore_all(now) {
            self.state = ControllerState::Error;
            self.message = error.to_string();
            return Err(error);
        }
        if let Some(s) = self.session.as_mut().filter(|s| s.ended_at.is_none()) {
            s.ended_at = Some(now);
            s.end_percentage = self.telemetry.as_ref().and_then(|t| t.percentage);
            s.result = result.into();
            self.storage.save_session(s)?;
        }
        self.state = ControllerState::Idle;
        self.controller.reset();
        self.observation = None;
        self.message = if result == "reached" {
            "Goal reached. Your system settings are restored."
        } else if result == "reserve_reached" {
            "Your reserve was reached before the deadline. Settings are restored; connect power."
        } else {
            "Session stopped. Your system settings are restored."
        }
        .into();
        let message = self.message.clone();
        self.log(
            now,
            if result == "reached" {
                "Goal reached"
            } else {
                "Settings restored"
            },
            &message,
            None,
            None,
            None,
            None,
        )?;
        Ok(())
    }

    pub fn restore_now(&mut self, now: DateTime<Utc>) -> Result<()> {
        self.stop("stopped", now)
    }

    fn restore_all(&mut self, now: DateTime<Utc>) -> Result<()> {
        let entries = self.storage.pending()?;
        let mut failed = Vec::new();
        let mut overridden = Vec::new();
        for entry in entries {
            if overridden.contains(&(entry.snapshot.kind(), entry.snapshot.target().to_string())) {
                continue;
            }
            match self.hardware.restore(&entry.snapshot, entry.desired) {
                Ok(RestoreOutcome::UserOverride) => {
                    // Power-plan clones still need cleanup even after a user changes the active scheme.
                    if entry.snapshot.kind() == ActuatorKind::CpuPolicy {
                        self.storage.mark_restored(entry.id, "user_override")?;
                    } else {
                        self.storage
                            .supersede(entry.snapshot.kind(), entry.snapshot.target())?;
                        overridden.push((entry.snapshot.kind(), entry.snapshot.target().into()));
                    }
                    self.log(
                        now,
                        "Kept your manual change",
                        "A newer system setting takes priority over the saved baseline.",
                        Some(entry.snapshot.kind()),
                        None,
                        None,
                        None,
                    )?;
                }
                Ok(outcome) => {
                    self.storage.mark_restored(
                        entry.id,
                        if outcome == RestoreOutcome::Restored {
                            "restored"
                        } else {
                            "already_original"
                        },
                    )?;
                }
                Err(error) => {
                    tracing::warn!(actuator=?entry.snapshot.kind(),error=%error,"Recovery will be retried");
                    failed.push(error.to_string());
                }
            }
        }
        self.expected.clear();
        self.observation = None;
        self.capabilities = self.hardware.capabilities();
        if !failed.is_empty() || !self.storage.pending()?.is_empty() {
            self.state = ControllerState::Error;
            return Err(Error::Hardware(format!(
                "Some settings could not be restored. Reconnect the display or retry Restore. {}",
                failed.join(" ")
            )));
        }
        Ok(())
    }

    pub fn tick(&mut self, now: DateTime<Utc>) -> Result<()> {
        let result = self.tick_inner(now);
        if let Err(error) = &result {
            tracing::error!(error=%error,"Controller tick failed; entering recovery");
            self.state = ControllerState::Error;
            self.message = error.to_string();
            // A storage/telemetry error must never cause further active writes.
            if let Err(recovery) = self.restore_all(now) {
                tracing::error!(error=%recovery,"Recovery remains pending");
            }
            self.state = ControllerState::Error;
        }
        result
    }

    fn tick_inner(&mut self, now: DateTime<Utc>) -> Result<()> {
        let previous = self.last_tick.replace(now);
        let gap = previous.is_some_and(|t| {
            let d = (now - t).num_seconds();
            !(0..=15).contains(&d)
        });
        if gap {
            self.restore_all(now)?;
            self.estimator.reset();
            self.controller.reset();
            self.last_scan = None;
            self.log(now,"Usage estimate restarted","A sleep, resume, or clock change was detected. Settings were restored before taking fresh readings.",None,None,None,None)?;
        }
        let telemetry = self.hardware.telemetry(now)?;
        self.telemetry = Some(telemetry.clone());
        let active = self.session.as_ref().is_some_and(|s| s.ended_at.is_none());
        if active && self.session.as_ref().is_some_and(|s| now >= s.deadline) {
            self.stop("reached", now)?;
            self.estimator.reset();
            return Ok(());
        }
        if active && telemetry.ac_connected != Some(false) {
            if self.state != ControllerState::PausedAc || !self.storage.pending()?.is_empty() {
                self.state = ControllerState::Restoring;
                self.restore_all(now)?;
                self.estimator.reset();
                self.controller.reset();
                self.log(
                    now,
                    "Paused and restored",
                    if telemetry.ac_connected == Some(true) {
                        "Power connected. All session adjustments were restored."
                    } else {
                        "The power source is unknown. All session adjustments were restored."
                    },
                    None,
                    None,
                    None,
                    None,
                )?;
            }
            self.state = ControllerState::PausedAc;
            self.prediction = Prediction::default();
            self.message =
                "Paused. Settings are restored. Start again when running on battery.".into();
            return Ok(());
        }
        if active && self.state == ControllerState::PausedAc {
            if !self.settings.resume_on_battery {
                return Ok(());
            }
            self.state = ControllerState::Monitoring;
            self.estimator.reset();
            self.controller.reset();
            self.capabilities = self.hardware.capabilities();
            self.baselines = self
                .capabilities
                .iter()
                .filter_map(|c| c.current.map(|v| (c.kind, v)))
                .collect();
            self.log(
                now,
                "Resumed on battery",
                "Automatic resume is enabled. Learning current usage again.",
                None,
                None,
                None,
                None,
            )?;
        }
        if active
            && telemetry
                .percentage
                .is_some_and(|p| p <= self.settings.reserve_percent as f64)
        {
            self.stop("reserve_reached", now)?;
            self.prediction.feasibility = crate::Feasibility::Impossible;
            return Ok(());
        }
        self.estimator.ingest(telemetry.clone());
        let deadline = self
            .session
            .as_ref()
            .filter(|s| s.ended_at.is_none())
            .map(|s| s.deadline);
        self.prediction =
            self.estimator
                .predict(&telemetry, deadline, self.settings.reserve_percent);
        if self
            .last_scan
            .is_none_or(|t| (now - t).num_seconds() >= if active { 20 } else { 60 })
        {
            self.scan_overrides(now, active)?;
            self.last_scan = Some(now);
        }
        if self
            .last_sample
            .is_none_or(|t| (now - t).num_seconds() >= 10)
        {
            // No idle on-disk telemetry archive; history belongs to explicit sessions.
            if active {
                self.storage.sample(
                    self.session.as_ref().map(|s| s.id.as_str()),
                    &telemetry,
                    self.prediction.estimated_power_w,
                )?;
            }
            self.chart.push_back(ChartPoint {
                timestamp: now,
                power_w: self.prediction.estimated_power_w,
                budget_w: self.prediction.power_budget_w,
                percentage: telemetry.percentage,
            });
            while self.chart.len() > 180 {
                self.chart.pop_front();
            }
            self.last_sample = Some(now);
        }
        if (now - self.last_retention).num_seconds() >= 3600 {
            self.storage.retention(now)?;
            self.last_retention = now;
        }
        if !active || self.state == ControllerState::Error {
            return Ok(());
        }
        self.observe(now)?;
        let (state, decision) = self.controller.evaluate(
            now,
            &self.prediction,
            &self.settings,
            &self.capabilities,
            &self.baselines,
            &self.effects,
        );
        let old_state = self.state;
        self.state = state;
        self.message = match state {
            ControllerState::Monitoring => "Learning your usage before making adjustments.",
            ControllerState::Adjusting => {
                "Making one gentle adjustment, then observing the result."
            }
            ControllerState::AtRisk => {
                "Recent usage may miss the deadline. Consider reducing your workload."
            }
            ControllerState::Critical => "This goal cannot be met with the current reserve.",
            _ => "Keeping enough battery for your deadline.",
        }
        .into();
        if state == ControllerState::AtRisk && old_state != state {
            self.log(
                now,
                "Deadline at risk",
                &self.prediction.explanation.clone(),
                None,
                None,
                None,
                None,
            )?;
        }
        if let Some(decision) = decision {
            let snapshot = self.hardware.snapshot(&decision.capability)?;
            if Some(snapshot.original()) != decision.capability.current {
                self.controller.user_override(now, decision.capability.kind);
                self.last_scan = None;
                return Ok(());
            }
            let id = self.storage.prepare(
                self.session.as_ref().map(|s| s.id.as_str()),
                &snapshot,
                decision.value,
                now,
            )?;
            match self.hardware.apply(&snapshot, decision.value) {
                Ok(()) => {
                    self.storage.mark_applied(id)?;
                    self.controller.applied(now, decision.capability.kind);
                    self.capabilities = self.hardware.capabilities();
                    let expected_target = match &snapshot {
                        crate::Snapshot::CpuPolicy { temporary_guid, .. } => temporary_guid.clone(),
                        _ => snapshot.target().into(),
                    };
                    self.expected
                        .insert(snapshot.kind(), (expected_target, decision.value));
                    self.log(
                        now,
                        match snapshot.kind() {
                            ActuatorKind::Brightness => "Brightness adjusted",
                            ActuatorKind::RefreshRate => "Refresh rate adjusted",
                            ActuatorKind::CpuPolicy => "Processor limit adjusted",
                        },
                        &decision.reason,
                        Some(snapshot.kind()),
                        Some(snapshot.original()),
                        Some(decision.value),
                        None,
                    )?;
                    self.observation = if !decision.relaxing {
                        self.prediction.estimated_power_w.map(|before| Observation {
                            kind: snapshot.kind(),
                            target: snapshot.target().into(),
                            at: now,
                            before,
                            before_value: snapshot.original(),
                            after_value: decision.value,
                        })
                    } else {
                        None
                    };
                }
                Err(error) => {
                    tracing::warn!(actuator=?snapshot.kind(),error=%error,"Adjustment failed");
                    let outcome = self.hardware.restore(&snapshot, decision.value)?;
                    self.storage.mark_restored(
                        id,
                        match outcome {
                            RestoreOutcome::UserOverride => "user_override",
                            _ => "failed_and_restored",
                        },
                    )?;
                    self.controller.failed(now, snapshot.kind());
                    self.state = ControllerState::AtRisk;
                    self.log(
                        now,
                        "Adjustment unavailable",
                        &format!("{} This control is paused for ten minutes.", error),
                        Some(snapshot.kind()),
                        None,
                        None,
                        None,
                    )?;
                    self.capabilities = self.hardware.capabilities();
                }
            }
        }
        Ok(())
    }

    fn scan_overrides(&mut self, now: DateTime<Utc>, active: bool) -> Result<()> {
        let caps = self.hardware.capabilities();
        if active {
            for cap in &caps {
                let Some((target, value)) = self.expected.get(&cap.kind).cloned() else {
                    continue;
                };
                if !cap.supported || cap.current.is_none() {
                    continue;
                }
                if cap.current != Some(value) || cap.target != target {
                    if cap.kind == ActuatorKind::CpuPolicy {
                        for entry in self
                            .storage
                            .pending()?
                            .into_iter()
                            .filter(|e| e.snapshot.kind() == cap.kind)
                        {
                            self.hardware.restore(&entry.snapshot, entry.desired)?;
                            self.storage.mark_restored(entry.id, "user_override")?;
                        }
                    } else {
                        self.storage.supersede(cap.kind, &target)?;
                    }
                    self.expected.remove(&cap.kind);
                    self.controller.user_override(now, cap.kind);
                    if let Some(v) = cap.current {
                        self.baselines.insert(cap.kind, v);
                    }
                    self.observation = None;
                    self.log(now,"Your change takes priority","You changed this setting. Automatic control is paused for ten minutes and the new value becomes the baseline.",Some(cap.kind),Some(value),cap.current,None)?;
                }
            }
        }
        self.effects = self.storage.effects(&caps)?;
        self.capabilities = caps;
        Ok(())
    }

    fn observe(&mut self, now: DateTime<Utc>) -> Result<()> {
        if !self
            .observation
            .as_ref()
            .is_some_and(|o| (now - o.at).num_seconds() >= 45)
        {
            return Ok(());
        }
        let Some(o) = self.observation.take() else {
            return Ok(());
        };
        if self
            .prediction
            .coefficient_of_variation
            .is_none_or(|v| v > 0.15)
        {
            return Ok(());
        }
        if let Some(after) = self.prediction.estimated_power_w {
            let saving = o.before - after;
            self.storage.record_effect(o.kind, &o.target, saving)?;
            self.log(
                now,
                "Observed usage change",
                "Measured after the adjustment. Workload changes can also affect this estimate.",
                Some(o.kind),
                Some(o.before_value),
                Some(o.after_value),
                Some(saving),
            )?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn log(
        &mut self,
        now: DateTime<Utc>,
        title: &str,
        detail: &str,
        kind: Option<ActuatorKind>,
        before: Option<u32>,
        after: Option<u32>,
        saving: Option<f64>,
    ) -> Result<()> {
        let a = self.storage.activity(
            self.session.as_ref().map(|s| s.id.as_str()),
            Activity {
                id: 0,
                timestamp: now,
                title: title.into(),
                detail: detail.into(),
                kind,
                before,
                after,
                observed_saving_w: saving,
            },
        )?;
        self.activity.push_front(a);
        while self.activity.len() > 80 {
            self.activity.pop_back();
        }
        Ok(())
    }

    pub fn simulation_event(&mut self, event: SimulationEvent, now: DateTime<Utc>) -> Result<()> {
        if !self.hardware.is_simulation() {
            return Err(Error::InvalidInput(
                "Simulation controls cannot run on real hardware.".into(),
            ));
        }
        if matches!(event, SimulationEvent::CrashRecovery) {
            // Real restart recovery is exercised by integration tests. Here the UI rehearses it without killing the app.
            self.stop("interrupted", now)?;
            self.message =
                "Simulated interrupted session recovered. All synthetic settings are restored."
                    .into();
            self.log(
                now,
                "Recovery rehearsal completed",
                "Persistent restart recovery is also covered by the automated crash tests.",
                None,
                None,
                None,
                None,
            )?;
        } else {
            let detail = format!("Synthetic event: {event:?}");
            self.hardware.simulation_event(event)?;
            self.last_scan = None;
            self.log(now, "Simulation scenario", &detail, None, None, None, None)?;
            self.tick(now + Duration::milliseconds(1))?;
        }
        Ok(())
    }

    pub fn export_diagnostics(&self) -> Result<PathBuf> {
        let directory = self.storage.directory.join("diagnostics");
        std::fs::create_dir_all(&directory)?;
        let realm = if self.hardware.is_simulation() {
            "simulation"
        } else {
            "windows"
        };
        let path = directory.join(format!(
            "battery-deadline-{realm}-{}.zip",
            Utc::now().format("%Y%m%d-%H%M%S")
        ));
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path)?);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("capabilities.json", options)?;
        zip.write_all(serde_json::to_string_pretty(&self.capabilities)?.as_bytes())?;
        zip.start_file("version.json", options)?;
        zip.write_all(serde_json::to_string_pretty(&serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"simulation":self.hardware.is_simulation(),"state":self.state,"pending_recovery":self.storage.pending()?.len()}))?.as_bytes())?;
        zip.start_file("battery-deadline.log", options)?;
        // Export only application events, never arbitrary user files or the complete database.
        for a in &self.activity {
            zip.write_all(serde_json::to_string(a)?.as_bytes())?;
            zip.write_all(b"\n")?;
        }
        zip.finish()?;
        Ok(path)
    }
}
