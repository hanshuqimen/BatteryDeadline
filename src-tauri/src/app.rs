use battery_deadline_core::locale::{resolve, text};
use battery_deadline_core::{
    Settings, SimulationEvent,
    service::{Command, Service},
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tauri::{
    Manager, State,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

struct AppState {
    service: Service,
    quitting: Arc<AtomicBool>,
}

async fn request(state: &AppState, command: Command) -> std::result::Result<Value, String> {
    let service = state.service.clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.request(command).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn get_snapshot(state: State<'_, AppState>) -> std::result::Result<Value, String> {
    request(&state, Command::GetSnapshot).await
}
#[tauri::command]
async fn get_history(state: State<'_, AppState>) -> std::result::Result<Value, String> {
    request(&state, Command::GetHistory).await
}
#[tauri::command]
async fn start_session(
    state: State<'_, AppState>,
    deadline: DateTime<Utc>,
) -> std::result::Result<Value, String> {
    request(&state, Command::StartSession { deadline }).await
}
#[tauri::command]
async fn stop_session(state: State<'_, AppState>) -> std::result::Result<Value, String> {
    request(&state, Command::StopSession).await
}
#[tauri::command]
async fn restore_settings(state: State<'_, AppState>) -> std::result::Result<Value, String> {
    request(&state, Command::RestoreSettings).await
}
#[tauri::command]
async fn save_settings(
    state: State<'_, AppState>,
    settings: Settings,
) -> std::result::Result<Value, String> {
    request(&state, Command::SaveSettings { settings }).await
}
#[tauri::command]
async fn export_diagnostics(state: State<'_, AppState>) -> std::result::Result<Value, String> {
    request(&state, Command::ExportDiagnostics).await
}
#[tauri::command]
async fn simulation_event(
    state: State<'_, AppState>,
    event: SimulationEvent,
) -> std::result::Result<Value, String> {
    request(&state, Command::SimulationEvent { event }).await
}
#[tauri::command]
async fn advance_simulation(
    state: State<'_, AppState>,
    seconds: u32,
) -> std::result::Result<Value, String> {
    request(&state, Command::AdvanceSimulation { seconds }).await
}

fn open_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
fn quit_safely(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        match state.service.request(Command::StopSession) {
            Ok(_) => {
                state.quitting.store(true, Ordering::SeqCst);
                app.exit(0);
            }
            Err(error) => {
                tracing::error!(error=%error,"Quit deferred until system settings can be restored");
                open_window(&app);
            }
        }
    });
}

pub fn run() {
    let simulation = std::env::args().any(|arg| arg == "--simulate");
    let fallback = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("BatteryDeadline");
    let log_directory = fallback.join("logs");
    let file_appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("battery-deadline")
        .filename_suffix("log")
        .max_log_files(7)
        .build(log_directory);
    let guard = match file_appender {
        Ok(appender) => {
            let (writer, guard) = tracing_appender::non_blocking(appender);
            let filter =
                tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                    tracing_subscriber::EnvFilter::new(if cfg!(debug_assertions) {
                        "battery_deadline=debug,battery_deadline_core=debug"
                    } else {
                        "battery_deadline=info,battery_deadline_core=info"
                    })
                });
            let _ = tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_writer(writer)
                .json()
                .try_init();
            Some(guard)
        }
        Err(_) => None,
    };
    let application = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            open_window(app)
        }))
        .setup(move |app| {
            // Storage and COM are initialized exclusively on the hardware worker.
            let service = Service::spawn(fallback, simulation);
            let quitting = Arc::new(AtomicBool::new(false));
            app.manage(AppState {
                service: service.clone(),
                quitting,
            });
            let language = resolve("system");
            let open = MenuItem::with_id(
                app,
                "open",
                text(
                    language,
                    "Open BatteryDeadline",
                    "打开 BatteryDeadline",
                    "BatteryDeadline を開く",
                ),
                true,
                None::<&str>,
            )?;
            let deadline = MenuItem::with_id(
                app,
                "deadline",
                text(
                    language,
                    "Choose a deadline",
                    "选择截止时间",
                    "目標時刻を選択",
                ),
                false,
                None::<&str>,
            )?;
            let prediction = MenuItem::with_id(
                app,
                "prediction",
                text(
                    language,
                    "Collecting battery readings",
                    "正在读取电池数据",
                    "バッテリーの測定中",
                ),
                false,
                None::<&str>,
            )?;
            let pause = MenuItem::with_id(
                app,
                "pause",
                text(language, "Stop and restore", "停止并恢复", "停止して復元"),
                true,
                None::<&str>,
            )?;
            let restore = MenuItem::with_id(
                app,
                "restore",
                text(
                    language,
                    "Restore system settings",
                    "恢复系统设置",
                    "システム設定を復元",
                ),
                true,
                None::<&str>,
            )?;
            let quit = MenuItem::with_id(
                app,
                "quit",
                text(language, "Quit", "退出", "終了"),
                true,
                None::<&str>,
            )?;
            let menu = Menu::with_items(
                app,
                &[&open, &deadline, &prediction, &pause, &restore, &quit],
            )?;
            let icon = app
                .default_window_icon()
                .cloned()
                .ok_or("Application icon is unavailable")?;
            TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .tooltip("BatteryDeadline")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => open_window(app),
                    "quit" => quit_safely(app.clone()),
                    "pause" | "restore" => {
                        let handle = app.clone();
                        std::thread::spawn(move || {
                            let state = handle.state::<AppState>();
                            if let Err(error) = state.service.request(Command::RestoreSettings) {
                                tracing::warn!(error=%error,"Tray restore failed");
                                open_window(&handle);
                            }
                        });
                    }
                    _ => {}
                })
                .build(app)?;
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(10));
                    if handle.state::<AppState>().quitting.load(Ordering::SeqCst) {
                        break;
                    }
                    if let Some(snapshot) = service.latest() {
                        let language = snapshot.locale.as_str();
                        let _ = open.set_text(text(
                            language,
                            "Open BatteryDeadline",
                            "打开 BatteryDeadline",
                            "BatteryDeadline を開く",
                        ));
                        let _ = pause.set_text(text(
                            language,
                            "Stop and restore",
                            "停止并恢复",
                            "停止して復元",
                        ));
                        let _ = restore.set_text(text(
                            language,
                            "Restore system settings",
                            "恢复系统设置",
                            "システム設定を復元",
                        ));
                        let _ = quit.set_text(text(language, "Quit", "退出", "終了"));
                        let percent = snapshot
                            .telemetry
                            .as_ref()
                            .and_then(|t| t.percentage)
                            .map_or(
                                text(
                                    language,
                                    "Unknown battery",
                                    "电池状态未知",
                                    "バッテリーの状態が不明",
                                )
                                .into(),
                                |p| format!("{p:.0}%"),
                            );
                        let status = match snapshot.state {
                            battery_deadline_core::ControllerState::Idle => {
                                text(language, "Idle", "空闲", "待機中")
                            }
                            battery_deadline_core::ControllerState::PausedAc => {
                                text(language, "Paused", "已暂停", "一時停止中")
                            }
                            battery_deadline_core::ControllerState::AtRisk => {
                                text(language, "At risk", "目标有风险", "目標に注意")
                            }
                            battery_deadline_core::ControllerState::Error => {
                                text(language, "Restore needed", "需要恢复", "復元が必要")
                            }
                            _ => text(language, "Monitoring", "正在监测", "監視中"),
                        };
                        let margin = snapshot
                            .prediction
                            .deadline_margin_minutes
                            .map_or(String::new(), |m| {
                                format!(" · {m:+.0} {}", text(language, "min", "分钟", "分"))
                            });
                        if let Some(tray) = handle.tray_by_id("main-tray") {
                            let _ = tray.set_tooltip(Some(format!(
                                "BatteryDeadline\n{percent} · {status}{margin}"
                            )));
                        }
                        let _ = deadline.set_text(
                            snapshot
                                .session
                                .as_ref()
                                .filter(|s| s.ended_at.is_none())
                                .map_or(
                                    text(
                                        language,
                                        "Choose a deadline",
                                        "选择截止时间",
                                        "目標時刻を選択",
                                    )
                                    .into(),
                                    |s| {
                                        format!(
                                            "{}: {}",
                                            text(language, "Deadline", "截止时间", "目標時刻"),
                                            s.deadline
                                                .with_timezone(&chrono::Local)
                                                .format("%H:%M")
                                        )
                                    },
                                ),
                        );
                        let _ = prediction.set_text(
                            snapshot.prediction.predicted_depletion_time.map_or(
                                text(
                                    language,
                                    "Learning recent usage",
                                    "正在学习近期功耗",
                                    "最近の使用状況を測定中",
                                )
                                .into(),
                                |p| {
                                    format!(
                                        "{}: {}",
                                        text(
                                            language,
                                            "Expected until",
                                            "预计可用到",
                                            "使用できる時刻の予測"
                                        ),
                                        p.with_timezone(&chrono::Local).format("%H:%M")
                                    )
                                },
                            ),
                        );
                    }
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                if state
                    .service
                    .latest()
                    .is_none_or(|s| s.settings.close_to_tray)
                {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    api.prevent_close();
                    quit_safely(window.app_handle().clone());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_history,
            start_session,
            stop_session,
            restore_settings,
            save_settings,
            export_diagnostics,
            simulation_event,
            advance_simulation
        ])
        .build(tauri::generate_context!());
    match application {
        Ok(app) => app.run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event
                && !app.state::<AppState>().quitting.load(Ordering::SeqCst)
            {
                api.prevent_exit();
                quit_safely(app.clone());
            }
        }),
        Err(error) => {
            tracing::error!(error=%error,"Desktop startup failed");
            eprintln!("BatteryDeadline could not start: {error}");
        }
    }
    drop(guard);
}
