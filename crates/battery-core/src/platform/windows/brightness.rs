use crate::{ActuatorKind, Capability, Error, Result, Snapshot};
use serde::{Deserialize, Serialize};
use wmi::WMIConnection;

#[derive(Deserialize)]
#[serde(rename = "WmiMonitorBrightness")]
struct Brightness {
    #[serde(rename = "InstanceName")]
    instance: String,
    #[serde(rename = "CurrentBrightness")]
    current: u8,
    #[serde(rename = "Level")]
    levels: Vec<u8>,
}
#[derive(Deserialize)]
#[serde(rename = "WmiMonitorBrightnessMethods")]
struct Methods {
    #[serde(rename = "InstanceName")]
    instance: String,
    #[serde(rename = "__PATH")]
    path: String,
}
#[derive(Serialize)]
struct Parameters {
    #[serde(rename = "Timeout")]
    timeout: u32,
    #[serde(rename = "Brightness")]
    brightness: u8,
}
#[derive(Deserialize)]
struct Output {
    #[serde(rename = "ReturnValue")]
    value: u32,
}

fn connection() -> Result<WMIConnection> {
    WMIConnection::with_namespace_path("ROOT\\WMI").map_err(|e| {
        tracing::debug!(error=%e,"Brightness WMI unavailable");
        Error::Hardware("Brightness control is not available on this display.".into())
    })
}
fn monitors() -> Result<Vec<Brightness>> {
    connection()?.raw_query("SELECT InstanceName,CurrentBrightness,Level FROM WmiMonitorBrightness WHERE Active=TRUE").map_err(|e|{tracing::debug!(error=%e,"Brightness query failed");Error::Hardware("Brightness control is not available on this display.".into())})
}

pub fn capability() -> Capability {
    match monitors().ok().and_then(|v| v.into_iter().next()) {
        Some(m) => Capability {
            kind: ActuatorKind::Brightness,
            target: m.instance,
            supported: true,
            current: Some(m.current as u32),
            values: m.levels.into_iter().map(u32::from).collect(),
            explanation: "Internal panel brightness, verified through Windows WMI.".into(),
        },
        None => Capability {
            kind: ActuatorKind::Brightness,
            target: String::new(),
            supported: false,
            current: None,
            values: vec![],
            explanation: "Brightness control is not available on this display.".into(),
        },
    }
}
pub fn current(target: &str) -> Result<u32> {
    monitors()?
        .into_iter()
        .find(|m| m.instance == target)
        .map(|m| m.current as u32)
        .ok_or_else(|| {
            Error::Hardware("Reconnect the internal display to restore brightness.".into())
        })
}
pub fn snapshot(target: &str) -> Result<Snapshot> {
    Ok(Snapshot::Brightness {
        target: target.into(),
        original: current(target)?,
    })
}
pub fn set(target: &str, value: u32) -> Result<()> {
    if value > 100 {
        return Err(Error::InvalidInput(
            "Brightness must be between 0 and 100%.".into(),
        ));
    }
    let connection = connection()?;
    let methods: Vec<Methods> = connection
        .raw_query("SELECT InstanceName,__PATH FROM WmiMonitorBrightnessMethods WHERE Active=TRUE")
        .map_err(|e| {
            tracing::debug!(error=%e,"Brightness methods unavailable");
            Error::Hardware(
                "Brightness could not be adjusted. This display may be unavailable.".into(),
            )
        })?;
    let method = methods
        .into_iter()
        .find(|m| m.instance == target)
        .ok_or_else(|| {
            Error::Hardware("The original brightness display is disconnected.".into())
        })?;
    let output: Output = connection
        .exec_instance_method::<Methods, _>(
            &method.path,
            "WmiSetBrightness",
            Parameters {
                timeout: 0,
                brightness: value as u8,
            },
        )
        .map_err(|e| {
            tracing::debug!(error=%e,"Brightness set failed");
            Error::Hardware("Windows refused the brightness change.".into())
        })?;
    if output.value != 0 || current(target)? != value {
        return Err(Error::Hardware(
            "The brightness change could not be verified.".into(),
        ));
    }
    Ok(())
}
