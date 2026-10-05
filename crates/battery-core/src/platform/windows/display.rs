use crate::{ActuatorKind, Capability, Error, RestoreOutcome, Result, Snapshot};
use std::{
    mem::size_of,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{Devices::Display::*, Graphics::Gdi::*};

fn primary() -> Result<String> {
    for index in 0..32 {
        let mut device = DISPLAY_DEVICEW {
            cb: size_of::<DISPLAY_DEVICEW>() as u32,
            ..Default::default()
        };
        // SAFETY: the output structure is correctly sized; null means enumerate display adapters.
        if unsafe { EnumDisplayDevicesW(null(), index, &mut device, 0) } == 0 {
            break;
        }
        if device.StateFlags & DISPLAY_DEVICE_PRIMARY_DEVICE != 0 {
            return Ok(super::from_wide(&device.DeviceName));
        }
    }
    Err(Error::Hardware(
        "The primary display is unavailable.".into(),
    ))
}
fn monitor_id(target: &str) -> Result<String> {
    let name = super::wide(target);
    let mut device = DISPLAY_DEVICEW {
        cb: size_of::<DISPLAY_DEVICEW>() as u32,
        ..Default::default()
    };
    // SAFETY: name and output structure remain valid; index zero identifies the adapter's monitor.
    if unsafe { EnumDisplayDevicesW(name.as_ptr(), 0, &mut device, 0) } == 0 {
        return Err(Error::Hardware(
            "Reconnect the original display to restore its refresh rate.".into(),
        ));
    }
    let id = super::from_wide(&device.DeviceID);
    if id.is_empty() {
        return Err(Error::Hardware(
            "The display does not provide a stable identity.".into(),
        ));
    }
    Ok(id)
}
fn current(target: &str) -> Result<DEVMODEW> {
    let name = super::wide(target);
    let mut mode = DEVMODEW {
        dmSize: size_of::<DEVMODEW>() as u16,
        ..Default::default()
    };
    // SAFETY: mode is initialized to the SDK size, and name is null terminated.
    if unsafe { EnumDisplaySettingsW(name.as_ptr(), ENUM_CURRENT_SETTINGS, &mut mode) } == 0 {
        return Err(Error::Hardware(
            "Reconnect the display to read its current refresh rate.".into(),
        ));
    }
    Ok(mode)
}
fn orientation(mode: &DEVMODEW) -> u32 {
    // SAFETY: the mode came from a display API; the display variant of this SDK union is active.
    unsafe { mode.Anonymous1.Anonymous2.dmDisplayOrientation }
}
fn same_geometry(a: &DEVMODEW, b: &DEVMODEW) -> bool {
    a.dmPelsWidth == b.dmPelsWidth
        && a.dmPelsHeight == b.dmPelsHeight
        && a.dmBitsPerPel == b.dmBitsPerPel
        && orientation(a) == orientation(b)
}

/// External/mirrored and active HDR displays are excluded from first-version automatic control.
fn conservative_target(target: &str) -> Result<()> {
    let mut path_count = 0;
    let mut mode_count = 0;
    // SAFETY: writable size parameters and documented active-path flag.
    if unsafe {
        GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
    } != 0
        || path_count > 64
        || mode_count > 256
    {
        return Err(Error::Hardware(
            "Windows cannot safely identify this display configuration.".into(),
        ));
    }
    let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
    let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
    // SAFETY: both arrays have the API-reported capacity; topology ID must be null with this flag.
    if unsafe {
        QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            null_mut(),
        )
    } != 0
    {
        return Err(Error::Hardware(
            "The display configuration changed. Refresh control is paused.".into(),
        ));
    }
    let mut matches = 0;
    for path in paths.iter().take(path_count as usize) {
        let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                size: size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                adapterId: path.sourceInfo.adapterId,
                id: path.sourceInfo.id,
            },
            ..Default::default()
        };
        // SAFETY: the header identifies the complete, correctly sized source-name packet.
        if unsafe { DisplayConfigGetDeviceInfo(&mut source.header) } != 0
            || super::from_wide(&source.viewGdiDeviceName) != target
        {
            continue;
        }
        matches += 1;
        if ![
            DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL,
            DISPLAYCONFIG_OUTPUT_TECHNOLOGY_LVDS,
            DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED,
            DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EMBEDDED,
        ]
        .contains(&path.targetInfo.outputTechnology)
        {
            return Err(Error::Hardware(
                "Automatic refresh control is limited to the internal primary display.".into(),
            ));
        }
        let mut color = DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
                size: size_of::<DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO>() as u32,
                adapterId: path.targetInfo.adapterId,
                id: path.targetInfo.id,
            },
            ..Default::default()
        };
        // SAFETY: the header points to the complete advanced-color packet for this target.
        if unsafe { DisplayConfigGetDeviceInfo(&mut color.header) } != 0 {
            return Err(Error::Hardware(
                "HDR status is unavailable. Refresh control is paused for safety.".into(),
            ));
        }
        // SAFETY: successful API populated the SDK flags union. Bit 1 is advancedColorEnabled.
        if unsafe { color.Anonymous.value } & 2 != 0 {
            return Err(Error::Hardware(
                "Refresh control is paused while HDR is enabled.".into(),
            ));
        }
    }
    if matches != 1 {
        return Err(Error::Hardware(
            "Refresh control is unavailable on mirrored or disconnected displays.".into(),
        ));
    }
    Ok(())
}
fn frequencies(target: &str, active: &DEVMODEW) -> Vec<u32> {
    let mut values = Vec::new();
    let name = super::wide(target);
    for index in 0..4096 {
        let mut mode = DEVMODEW {
            dmSize: size_of::<DEVMODEW>() as u16,
            ..Default::default()
        };
        // SAFETY: the live buffer has the SDK size; index is a bounded mode enumeration.
        if unsafe { EnumDisplaySettingsW(name.as_ptr(), index, &mut mode) } == 0 {
            break;
        }
        if same_geometry(&mode, active) && (30..=360).contains(&mode.dmDisplayFrequency) {
            values.push(mode.dmDisplayFrequency);
        }
    }
    values.sort_unstable();
    values.dedup();
    values
}

pub fn capability() -> Capability {
    let info = (|| -> Result<_> {
        let target = primary()?;
        let mode = current(&target)?;
        conservative_target(&target)?;
        monitor_id(&target)?;
        let values = frequencies(&target, &mode);
        Ok((target, mode, values))
    })();
    match info {
        Ok((target,mode,values))=>Capability {kind:ActuatorKind::RefreshRate,target,supported:values.len()>1,current:Some(mode.dmDisplayFrequency),values,
            explanation:"Only enumerated rates at the current resolution and color depth. HDR and external displays are excluded.".into()},
        Err(error)=>Capability {kind:ActuatorKind::RefreshRate,target:String::new(),supported:false,current:None,values:vec![],explanation:error.to_string()},
    }
}
pub fn snapshot(target: &str) -> Result<Snapshot> {
    conservative_target(target)?;
    let mode = current(target)?;
    Ok(Snapshot::RefreshRate {
        target: target.into(),
        original: mode.dmDisplayFrequency,
        monitor_id: monitor_id(target)?,
        width: mode.dmPelsWidth,
        height: mode.dmPelsHeight,
        bits_per_pixel: mode.dmBitsPerPel,
        orientation: orientation(&mode),
    })
}
fn matches_snapshot(s: &Snapshot, mode: &DEVMODEW) -> bool {
    matches!(s,Snapshot::RefreshRate {width,height,bits_per_pixel,orientation:o,..} if *width==mode.dmPelsWidth && *height==mode.dmPelsHeight && *bits_per_pixel==mode.dmBitsPerPel && *o==orientation(mode))
}
fn set(target: &str, value: u32) -> Result<()> {
    let before = current(target)?;
    if !frequencies(target, &before).contains(&value) {
        return Err(Error::Hardware(
            "The saved refresh rate is not supported by the current display mode.".into(),
        ));
    }
    let name = super::wide(target);
    let mut mode = before;
    mode.dmDisplayFrequency = value;
    mode.dmFields = DM_DISPLAYFREQUENCY;
    // SAFETY: only the frequency field is requested; CDS_TEST validates before any system change.
    if unsafe { ChangeDisplaySettingsExW(name.as_ptr(), &mode, null_mut(), CDS_TEST, null()) }
        != DISP_CHANGE_SUCCESSFUL
    {
        return Err(Error::Hardware(
            "Windows refused this refresh rate. Your resolution was not changed.".into(),
        ));
    }
    // SAFETY: this uses the validated mode, no registry-update flag, and only DM_DISPLAYFREQUENCY.
    if unsafe { ChangeDisplaySettingsExW(name.as_ptr(), &mode, null_mut(), 0, null()) }
        != DISP_CHANGE_SUCCESSFUL
    {
        return Err(Error::Hardware(
            "The refresh rate could not be adjusted.".into(),
        ));
    }
    let after = current(target)?;
    if after.dmDisplayFrequency != value || !same_geometry(&before, &after) {
        return Err(Error::Hardware(
            "The display change could not be verified. Restore settings before continuing.".into(),
        ));
    }
    Ok(())
}
pub fn apply(s: &Snapshot, value: u32) -> Result<()> {
    let Snapshot::RefreshRate {
        target,
        original,
        monitor_id: id,
        ..
    } = s
    else {
        return Err(Error::InvalidInput("Invalid display snapshot.".into()));
    };
    conservative_target(target)?;
    let mode = current(target)?;
    if monitor_id(target)? != *id
        || mode.dmDisplayFrequency != *original
        || !matches_snapshot(s, &mode)
    {
        return Err(Error::Hardware(
            "The display was changed manually. Automatic adjustment was cancelled.".into(),
        ));
    }
    set(target, value)
}
pub fn restore(s: &Snapshot, applied: u32) -> Result<RestoreOutcome> {
    let Snapshot::RefreshRate {
        target,
        original,
        monitor_id: id,
        ..
    } = s
    else {
        return Err(Error::InvalidInput(
            "Invalid display recovery record.".into(),
        ));
    };
    if monitor_id(target)? != *id {
        return Err(Error::Hardware(
            "Reconnect the original display to restore its refresh rate.".into(),
        ));
    }
    let mode = current(target)?;
    if mode.dmDisplayFrequency == *original {
        return Ok(RestoreOutcome::AlreadyOriginal);
    }
    if mode.dmDisplayFrequency != applied
        || !matches_snapshot(s, &mode)
        || conservative_target(target).is_err()
    {
        return Ok(RestoreOutcome::UserOverride);
    }
    set(target, *original)?;
    Ok(RestoreOutcome::Restored)
}
