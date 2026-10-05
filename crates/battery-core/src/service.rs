//! One worker owns all hardware and SQLite calls. Frontend requests never run WMI on the UI thread.
use crate::{AppSnapshot, Engine, Error, Result, Settings, SimulationEvent};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    thread,
};

#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    GetSnapshot,
    GetHistory,
    StartSession { deadline: DateTime<Utc> },
    StopSession,
    RestoreSettings,
    SaveSettings { settings: Settings },
    ExportDiagnostics,
    SimulationEvent { event: SimulationEvent },
    AdvanceSimulation { seconds: u32 },
}

struct Request {
    command: Command,
    response: mpsc::Sender<Result<Value>>,
}

#[derive(Clone)]
pub struct Service {
    sender: mpsc::SyncSender<Request>,
    latest: Arc<Mutex<Option<AppSnapshot>>>,
    error: Arc<Mutex<Option<String>>>,
}

impl Service {
    pub fn spawn(directory: PathBuf, simulation: bool) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<Request>(32);
        let latest = Arc::new(Mutex::new(None));
        let error = Arc::new(Mutex::new(None));
        let worker_latest = Arc::clone(&latest);
        let worker_error = Arc::clone(&error);
        thread::spawn(move || {
            let initialize = (|| -> Result<Engine> {
                let hardware: Box<dyn crate::Hardware> = if simulation {
                    Box::<crate::simulator::FakeHardware>::default()
                } else {
                    real_hardware()?
                };
                Engine::open(hardware, &directory, Utc::now())
            })();
            let mut engine = match initialize {
                Ok(engine) => engine,
                Err(e) => {
                    tracing::error!(error=%e,"Backend initialization failed");
                    if let Ok(mut error) = worker_error.lock() {
                        *error = Some(e.to_string());
                    }
                    for request in receiver {
                        let _ = request.response.send(Err(Error::Hardware(e.to_string())));
                    }
                    return;
                }
            };
            let mut offset = Duration::zero();
            loop {
                match receiver.recv_timeout(std::time::Duration::from_secs(1)) {
                    Ok(request) => {
                        let result = dispatch(&mut engine, &mut offset, request.command);
                        let _ = request.response.send(result);
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        let _ = engine.stop("quit", Utc::now() + offset);
                        break;
                    }
                }
                let _ = engine.tick(Utc::now() + offset);
                if let Ok(snapshot) = engine.snapshot()
                    && let Ok(mut latest) = worker_latest.lock()
                {
                    *latest = Some(snapshot);
                }
            }
        });
        Self {
            sender,
            latest,
            error,
        }
    }
    pub fn request(&self, command: Command) -> Result<Value> {
        let (sender, receiver) = mpsc::channel();
        self.sender
            .send(Request {
                command,
                response: sender,
            })
            .map_err(|_| {
                Error::Hardware(
                    "The battery controller is unavailable. Restart the app to recover settings."
                        .into(),
                )
            })?;
        receiver.recv_timeout(std::time::Duration::from_secs(30)).map_err(|_|Error::Hardware("Windows is taking too long to respond. Check the session state before retrying.".into()))?
    }
    pub fn latest(&self) -> Option<AppSnapshot> {
        self.latest.lock().ok().and_then(|s| s.clone())
    }
    pub fn initialization_error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|s| s.clone())
    }
}

fn dispatch(engine: &mut Engine, offset: &mut Duration, command: Command) -> Result<Value> {
    let now = Utc::now() + *offset;
    match command {
        Command::GetSnapshot => Ok(serde_json::to_value(engine.snapshot()?)?),
        Command::GetHistory => Ok(serde_json::to_value(engine.history()?)?),
        Command::StartSession { deadline } => {
            engine.start(deadline, now)?;
            Ok(Value::Null)
        }
        Command::StopSession => {
            engine.stop("stopped", now)?;
            Ok(Value::Null)
        }
        Command::RestoreSettings => {
            engine.restore_now(now)?;
            Ok(Value::Null)
        }
        Command::SaveSettings { settings } => {
            engine.save_settings(settings)?;
            Ok(Value::Null)
        }
        Command::ExportDiagnostics => Ok(Value::String(
            engine.export_diagnostics()?.display().to_string(),
        )),
        Command::SimulationEvent { event } => {
            engine.simulation_event(event, now)?;
            Ok(Value::Null)
        }
        Command::AdvanceSimulation { seconds } => {
            if !engine.snapshot()?.simulation || !(1..=1800).contains(&seconds) {
                return Err(Error::InvalidInput(
                    "Simulation can advance between one second and 30 minutes at a time.".into(),
                ));
            }
            for second in 1..=seconds {
                *offset += Duration::seconds(1);
                engine.tick(now + Duration::seconds(second as i64))?;
            }
            Ok(Value::Null)
        }
    }
}

#[cfg(windows)]
fn real_hardware() -> Result<Box<dyn crate::Hardware>> {
    Ok(Box::<crate::platform::windows::WindowsHardware>::default())
}
#[cfg(not(windows))]
fn real_hardware() -> Result<Box<dyn crate::Hardware>> {
    Err(Error::Hardware(
        "Hardware control requires Windows 11 x64. Use --simulate to develop safely.".into(),
    ))
}
