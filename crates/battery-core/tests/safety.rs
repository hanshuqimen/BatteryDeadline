use battery_deadline_core::{
    ActuatorKind, ControllerState, Engine, Hardware, SimulationEvent, simulator::FakeHardware,
    storage::Storage,
};
use chrono::{Duration, Utc};
use tempfile::TempDir;

fn setup() -> (TempDir, FakeHardware, Engine, chrono::DateTime<Utc>) {
    let directory = TempDir::new().unwrap();
    let fake = FakeHardware::default();
    let now = Utc::now();
    let engine = Engine::open(Box::new(fake.clone()), directory.path(), now).unwrap();
    (directory, fake, engine, now)
}
fn advance(engine: &mut Engine, now: chrono::DateTime<Utc>, from: i64, to: i64) {
    for second in from..=to {
        engine.tick(now + Duration::seconds(second)).unwrap();
    }
}

#[test]
fn language_is_persistent_and_does_not_change_the_running_control_contract() {
    let (dir, fake, mut engine, now) = setup();
    engine.start(now + Duration::hours(4), now).unwrap();
    advance(&mut engine, now, 1, 300);
    let before = engine.snapshot().unwrap();
    let mut preferences = before.settings.clone();
    preferences.language = "zh-CN".into();
    engine.save_settings(preferences.clone()).unwrap();
    let after = engine.snapshot().unwrap();
    assert_eq!(after.locale, "zh-CN");
    assert_eq!(
        after.session.as_ref().unwrap().id,
        before.session.as_ref().unwrap().id
    );
    assert_eq!(after.pending_recovery, before.pending_recovery);
    assert_eq!(
        after.settings.min_brightness,
        before.settings.min_brightness
    );
    preferences.reserve_percent = 15;
    assert!(engine.save_settings(preferences).is_err());
    let mut invalid = after.settings.clone();
    invalid.language = "unsupported".into();
    assert!(engine.save_settings(invalid).is_err());
    drop(engine);
    let recovered = Engine::open(Box::new(fake), dir.path(), now + Duration::minutes(10)).unwrap();
    assert_eq!(recovered.snapshot().unwrap().locale, "zh-CN");
    assert_eq!(recovered.snapshot().unwrap().pending_recovery, 0);
}

#[test]
fn older_settings_gain_a_system_language_default() {
    let legacy: battery_deadline_core::Settings =
        serde_json::from_str(r#"{"reserve_percent":10,"theme":"dark"}"#).unwrap();
    assert_eq!(legacy.language, "system");
    assert_eq!(
        battery_deadline_core::locale::from_tag("zh-Hans-CN"),
        "zh-CN"
    );
    assert_eq!(battery_deadline_core::locale::from_tag("ja-JP"), "ja");
}

#[test]
fn closed_loop_converges_then_ac_restores_every_control() {
    let (_dir, fake, mut engine, now) = setup();
    engine.start(now + Duration::hours(4), now).unwrap();
    advance(&mut engine, now, 1, 600);
    let controlled = engine.snapshot().unwrap();
    assert!(controlled.prediction.deadline_margin_minutes.unwrap() > 0.0);
    assert!(controlled.prediction.estimated_power_w.unwrap() < 9.0);
    assert!(
        controlled
            .activity
            .iter()
            .any(|a| a.kind == Some(ActuatorKind::RefreshRate))
    );
    assert!(controlled.pending_recovery > 0);
    engine
        .simulation_event(SimulationEvent::PlugAc, now + Duration::seconds(601))
        .unwrap();
    let s = engine.snapshot().unwrap();
    assert_eq!(s.state, ControllerState::PausedAc);
    assert_eq!(s.pending_recovery, 0);
    let hardware = fake.state.lock().unwrap();
    assert_eq!(hardware.brightness, 75);
    assert_eq!(hardware.refresh, 120);
    assert_eq!(hardware.cpu, 100);
    assert_eq!(hardware.plans.len(), 1);
}

#[test]
fn actual_engine_restart_recovers_all_applied_changes() {
    let (dir, fake, mut engine, now) = setup();
    engine.start(now + Duration::hours(4), now).unwrap();
    advance(&mut engine, now, 1, 400);
    assert!(engine.snapshot().unwrap().pending_recovery >= 3);
    // Dropping models a force-killed process: Engine has no magical Drop restore hook.
    drop(engine);
    let restored = Engine::open(
        Box::new(fake.clone()),
        dir.path(),
        now + Duration::seconds(401),
    )
    .unwrap();
    assert_eq!(restored.snapshot().unwrap().pending_recovery, 0);
    let s = fake.state.lock().unwrap();
    assert_eq!((s.brightness, s.refresh, s.cpu), (75, 120, 100));
    assert_eq!(s.plans.len(), 1);
    assert_eq!(restored.history().unwrap()[0].result, "interrupted");
}

#[test]
fn pending_journal_also_recovers_crash_between_write_and_applied_marker() {
    let (dir, mut fake, engine, now) = setup();
    drop(engine);
    let mut storage = Storage::open(dir.path(), true).unwrap();
    let cap = fake
        .capabilities()
        .into_iter()
        .find(|c| c.kind == ActuatorKind::RefreshRate)
        .unwrap();
    let snapshot = fake.snapshot(&cap).unwrap();
    storage.prepare(None, &snapshot, 60, now).unwrap();
    fake.apply(&snapshot, 60).unwrap();
    drop(storage);
    let restored = Engine::open(
        Box::new(fake.clone()),
        dir.path(),
        now + Duration::seconds(1),
    )
    .unwrap();
    assert_eq!(fake.state.lock().unwrap().refresh, 120);
    assert_eq!(restored.snapshot().unwrap().pending_recovery, 0);
}

#[test]
fn pending_without_a_system_write_is_safe_to_recover() {
    let (dir, mut fake, engine, now) = setup();
    drop(engine);
    let mut storage = Storage::open(dir.path(), true).unwrap();
    for cap in fake.capabilities() {
        let snapshot = fake.snapshot(&cap).unwrap();
        storage
            .prepare(
                None,
                &snapshot,
                cap.current.unwrap().saturating_sub(10),
                now,
            )
            .unwrap();
    }
    drop(storage);
    let recovered = Engine::open(Box::new(fake), dir.path(), now).unwrap();
    assert_eq!(recovered.snapshot().unwrap().pending_recovery, 0);
}

#[test]
fn manual_brightness_is_never_overwritten_on_stop_or_restart() {
    let (dir, fake, mut engine, now) = setup();
    engine.start(now + Duration::hours(4), now).unwrap();
    advance(&mut engine, now, 1, 180);
    engine
        .simulation_event(
            SimulationEvent::ManualBrightness,
            now + Duration::seconds(181),
        )
        .unwrap();
    advance(&mut engine, now, 182, 250);
    engine
        .stop("stopped", now + Duration::seconds(251))
        .unwrap();
    assert_eq!(fake.state.lock().unwrap().brightness, 85);
    drop(engine);
    let recovered = Engine::open(
        Box::new(fake.clone()),
        dir.path(),
        now + Duration::seconds(252),
    )
    .unwrap();
    assert_eq!(fake.state.lock().unwrap().brightness, 85);
    assert_eq!(recovered.snapshot().unwrap().pending_recovery, 0);
}

#[test]
fn failed_actuator_is_recorded_and_does_not_leave_pending_changes() {
    let (_dir, fake, mut engine, now) = setup();
    engine.start(now + Duration::hours(4), now).unwrap();
    fake.state.lock().unwrap().fail_next_action = true;
    advance(&mut engine, now, 1, 80);
    assert!(
        engine
            .snapshot()
            .unwrap()
            .activity
            .iter()
            .any(|a| a.title == "Adjustment unavailable")
    );
    engine.stop("stopped", now + Duration::seconds(81)).unwrap();
    assert_eq!(engine.snapshot().unwrap().pending_recovery, 0);
}

#[test]
fn disconnected_display_keeps_recovery_pending_and_blocks_new_session() {
    let (_dir, fake, mut engine, now) = setup();
    engine.start(now + Duration::hours(4), now).unwrap();
    advance(&mut engine, now, 1, 400);
    fake.state.lock().unwrap().unsupported_display = true;
    assert!(
        engine
            .stop("stopped", now + Duration::seconds(401))
            .is_err()
    );
    assert_eq!(engine.snapshot().unwrap().state, ControllerState::Error);
    assert!(engine.snapshot().unwrap().pending_recovery > 0);
    assert!(
        engine
            .start(now + Duration::hours(5), now + Duration::seconds(402))
            .is_err()
    );
    fake.state.lock().unwrap().unsupported_display = false;
    engine.restore_now(now + Duration::seconds(403)).unwrap();
    assert_eq!(engine.snapshot().unwrap().pending_recovery, 0);
    assert_eq!(fake.state.lock().unwrap().refresh, 120);
}

#[test]
fn deadline_stops_control_and_restores() {
    let (_dir, fake, mut engine, now) = setup();
    engine.start(now + Duration::minutes(3), now).unwrap();
    advance(&mut engine, now, 1, 181);
    let snapshot = engine.snapshot().unwrap();
    assert_eq!(snapshot.state, ControllerState::Idle);
    assert_eq!(snapshot.session.unwrap().result, "reached");
    assert_eq!(fake.state.lock().unwrap().brightness, 75);
}

#[test]
fn sleep_gap_restores_before_estimator_rewarm() {
    let (_dir, fake, mut engine, now) = setup();
    engine.start(now + Duration::hours(4), now).unwrap();
    advance(&mut engine, now, 1, 180);
    assert!(fake.state.lock().unwrap().brightness < 75);
    engine.tick(now + Duration::minutes(20)).unwrap();
    let snapshot = engine.snapshot().unwrap();
    assert_eq!(snapshot.prediction.sample_seconds, 0);
    assert_eq!(snapshot.pending_recovery, 0);
    assert_eq!(fake.state.lock().unwrap().brightness, 75);
}

#[test]
fn no_auto_resume_unless_explicitly_enabled() {
    let (_dir, fake, mut engine, now) = setup();
    engine.start(now + Duration::hours(4), now).unwrap();
    advance(&mut engine, now, 1, 180);
    engine
        .simulation_event(SimulationEvent::PlugAc, now + Duration::seconds(181))
        .unwrap();
    engine
        .simulation_event(SimulationEvent::UnplugAc, now + Duration::seconds(182))
        .unwrap();
    advance(&mut engine, now, 183, 300);
    assert_eq!(engine.snapshot().unwrap().state, ControllerState::PausedAc);
    assert_eq!(fake.state.lock().unwrap().brightness, 75);
}

#[test]
fn missing_rate_and_unsupported_display_do_not_crash() {
    let (_dir, fake, mut engine, now) = setup();
    {
        let mut s = fake.state.lock().unwrap();
        s.missing_rate = true;
        s.unsupported_display = true;
    }
    engine.start(now + Duration::hours(4), now).unwrap();
    advance(&mut engine, now, 1, 450);
    assert!(
        engine
            .snapshot()
            .unwrap()
            .prediction
            .estimated_power_w
            .is_some()
    );
    assert!(
        engine
            .snapshot()
            .unwrap()
            .capabilities
            .iter()
            .any(|c| c.kind == ActuatorKind::RefreshRate && !c.supported)
    );
    engine
        .stop("stopped", now + Duration::seconds(451))
        .unwrap();
}

#[test]
fn reserve_impossible_rejected_and_live_reserve_reached_restores() {
    let (_dir, fake, mut engine, now) = setup();
    fake.state.lock().unwrap().remaining_wh = 4.0;
    assert!(engine.start(now + Duration::hours(5), now).is_err());
    fake.state.lock().unwrap().remaining_wh = 39.0;
    engine.start(now + Duration::hours(4), now).unwrap();
    advance(&mut engine, now, 1, 180);
    engine
        .simulation_event(
            SimulationEvent::ImpossibleDeadline,
            now + Duration::seconds(181),
        )
        .unwrap();
    assert_eq!(engine.snapshot().unwrap().pending_recovery, 0);
    assert_eq!(
        engine.snapshot().unwrap().session.unwrap().result,
        "reserve_reached"
    );
}

#[test]
fn storage_lock_is_exclusive_per_realm_and_diagnostics_is_local() {
    let (dir, _fake, engine, _now) = setup();
    assert!(Storage::open(dir.path(), true).is_err());
    assert!(Storage::open(dir.path(), false).is_ok());
    let path = engine.export_diagnostics().unwrap();
    let zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    assert_eq!(zip.len(), 3);
    assert!(zip.file_names().any(|name| name == "capabilities.json"));
}
