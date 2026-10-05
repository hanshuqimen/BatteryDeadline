use battery_deadline_core::{Engine, Hardware, Result, SimulationEvent, simulator::FakeHardware};
use chrono::{Duration, Utc};
use std::path::PathBuf;

const HELP: &str = "BatteryDeadline diagnostics and simulator\n\nUsage:\n  battery-deadline-cli diagnostics battery [--json]\n  battery-deadline-cli diagnostics capabilities\n  battery-deadline-cli --simulate [--steps 600] [--data-dir PATH] [--json]\n\nDiagnostics are read-only. Simulation never changes Windows settings.\nSimulation steps represent virtual seconds and run without real-time waiting.\n";

fn main() {
    if let Err(error) = run() {
        eprintln!("BatteryDeadline: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{HELP}");
        return Ok(());
    }
    let json = args.iter().any(|a| a == "--json");
    if args.first().is_some_and(|a| a == "--simulate") {
        let mut steps = 600_i64;
        let mut directory = std::env::temp_dir().join(format!(
            "battery-deadline-simulation-{}",
            uuid::Uuid::new_v4()
        ));
        let mut index = 1;
        while index < args.len() {
            match args[index].as_str() {
                "--json" => {}
                "--steps" => {
                    index += 1;
                    steps = args
                        .get(index)
                        .and_then(|a| a.parse().ok())
                        .filter(|n| (1..=86400).contains(n))
                        .ok_or_else(|| {
                            battery_deadline_core::Error::InvalidInput(
                                "--steps needs an integer between 1 and 86400.".into(),
                            )
                        })?;
                }
                "--data-dir" => {
                    index += 1;
                    directory = PathBuf::from(args.get(index).ok_or_else(|| {
                        battery_deadline_core::Error::InvalidInput(
                            "--data-dir needs a folder path.".into(),
                        )
                    })?);
                }
                _ => {
                    return Err(battery_deadline_core::Error::InvalidInput(format!(
                        "Unknown option: {}. Use --help.",
                        args[index]
                    )));
                }
            }
            index += 1;
        }
        let now = Utc::now();
        let fake = FakeHardware::default();
        let mut engine = Engine::open(Box::new(fake.clone()), &directory, now)?;
        engine.start(now + Duration::hours(4), now)?;
        for second in 1..=steps {
            engine.tick(now + Duration::seconds(second))?;
        }
        let controlled = engine.snapshot()?;
        engine.simulation_event(SimulationEvent::PlugAc, now + Duration::seconds(steps + 1))?;
        let restored = engine.snapshot()?;
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({"controlled":controlled,"after_ac":restored})
                )?
            );
        } else {
            println!(
                "SIMULATION • no Windows changes\nState: {:?}\nPredicted draw: {:?} W\nBudget: {:?} W\nMargin: {:?} min\nActions: {}\nAfter AC: {:?}\nPending recovery: {}\nData: {}",
                controlled.state,
                controlled.prediction.estimated_power_w,
                controlled.prediction.power_budget_w,
                controlled.prediction.deadline_margin_minutes,
                controlled.activity.len(),
                restored.state,
                restored.pending_recovery,
                directory.display()
            );
        }
        return Ok(());
    }
    if args.len() < 2
        || args[0] != "diagnostics"
        || !["battery", "capabilities"].contains(&args[1].as_str())
        || args.iter().skip(2).any(|a| a != "--json")
    {
        return Err(battery_deadline_core::Error::InvalidInput(
            "Unknown command. Use --help for usage.".into(),
        ));
    }
    let mut hardware = real_hardware()?;
    if args[1] == "capabilities" {
        println!(
            "{}",
            serde_json::to_string_pretty(&hardware.capabilities())?
        );
    } else {
        let t = hardware.telemetry(Utc::now())?;
        if json {
            println!("{}", serde_json::to_string_pretty(&t)?);
        } else {
            println!(
                "AC: {:?}\nBattery present: {}\nPercentage: {:?}%\nRemaining: {:?} Wh\nFull charge: {:?} Wh\nDesign: {:?} Wh\nDischarge: {:?} W\nVoltage: {:?} V\nTelemetry quality: {:?}\n{}",
                t.ac_connected,
                t.battery_present,
                t.percentage,
                t.remaining_wh,
                t.full_charge_wh,
                t.design_wh,
                t.discharge_w,
                t.voltage_v,
                t.quality,
                t.explanation
            );
        }
    }
    Ok(())
}
#[cfg(windows)]
fn real_hardware() -> Result<Box<dyn Hardware>> {
    Ok(Box::new(
        battery_deadline_core::platform::windows::WindowsHardware::default(),
    ))
}
#[cfg(not(windows))]
fn real_hardware() -> Result<Box<dyn Hardware>> {
    Err(battery_deadline_core::Error::Hardware(
        "Hardware diagnostics require Windows 11 x64. Use --simulate for development.".into(),
    ))
}
