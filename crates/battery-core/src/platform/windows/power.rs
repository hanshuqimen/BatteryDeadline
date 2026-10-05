use crate::{ActuatorKind, Capability, Error, RestoreOutcome, Result, Snapshot};
use std::ptr::{null, null_mut};
use windows_sys::{
    Win32::{
        Foundation::LocalFree,
        System::{
            Power::*,
            SystemServices::{GUID_PROCESSOR_SETTINGS_SUBGROUP, GUID_PROCESSOR_THROTTLE_MAXIMUM},
        },
    },
    core::GUID,
};

fn code(code: u32, message: &str) -> Result<()> {
    if code == 0 {
        Ok(())
    } else {
        tracing::debug!(
            win32_code = code,
            operation = message,
            "Power policy API failed"
        );
        Err(Error::Hardware(message.into()))
    }
}
fn parse(text: &str) -> Result<GUID> {
    let u = uuid::Uuid::parse_str(text)
        .map_err(|_| Error::Hardware("The saved power plan identifier is invalid.".into()))?;
    Ok(GUID::from_u128(u.as_u128()))
}
fn format(guid: GUID) -> String {
    uuid::Uuid::from_fields(guid.data1, guid.data2, guid.data3, &guid.data4).to_string()
}

fn active() -> Result<GUID> {
    let mut pointer = null_mut();
    // SAFETY: pointer is a writable out parameter; reserved root key must be null.
    code(
        unsafe { PowerGetActiveScheme(null_mut(), &mut pointer) },
        "Windows could not read the active power plan.",
    )?;
    if pointer.is_null() {
        return Err(Error::Hardware(
            "The active power plan is unavailable.".into(),
        ));
    }
    // SAFETY: successful API returned one allocated GUID; copy before releasing with LocalFree.
    let guid = unsafe {
        let guid = *pointer;
        LocalFree(pointer.cast());
        guid
    };
    Ok(guid)
}
fn maximum(guid: &GUID) -> Result<u32> {
    let mut value = 0;
    // SAFETY: GUID pointers and value out parameter remain live; SDK processor GUIDs are used.
    code(
        unsafe {
            PowerReadDCValueIndex(
                null_mut(),
                guid,
                &GUID_PROCESSOR_SETTINGS_SUBGROUP,
                &GUID_PROCESSOR_THROTTLE_MAXIMUM,
                &mut value,
            )
        },
        "Processor limits are unavailable under the current power policy.",
    )?;
    if value > 100 {
        return Err(Error::Hardware(
            "This processor policy uses an unsupported value.".into(),
        ));
    }
    Ok(value)
}
pub fn capability() -> Capability {
    match active().and_then(|g| Ok((g, maximum(&g)?))) {
        Ok((g, value)) => Capability {
            kind: ActuatorKind::CpuPolicy,
            target: format(g),
            supported: true,
            current: Some(value),
            values: vec![],
            explanation:
                "DC processor limit on a private temporary plan. Windows may restrict changes."
                    .into(),
        },
        Err(error) => Capability {
            kind: ActuatorKind::CpuPolicy,
            target: String::new(),
            supported: false,
            current: None,
            values: vec![],
            explanation: error.to_string(),
        },
    }
}
pub fn snapshot() -> Result<Snapshot> {
    let guid = active()?;
    Ok(Snapshot::CpuPolicy {
        target: format(guid),
        original: maximum(&guid)?,
        temporary_guid: uuid::Uuid::new_v4().to_string(),
    })
}
pub fn apply(snapshot: &Snapshot, value: u32) -> Result<()> {
    let Snapshot::CpuPolicy {
        target,
        original,
        temporary_guid,
    } = snapshot
    else {
        return Err(Error::InvalidInput("Invalid power policy snapshot.".into()));
    };
    if !(65..=100).contains(&value) {
        return Err(Error::InvalidInput(
            "Processor limits must be 65–100%.".into(),
        ));
    }
    if format(active()?) != *target || maximum(&parse(target)?)? != *original {
        return Err(Error::Hardware(
            "Your active power policy changed. Automatic adjustment was cancelled.".into(),
        ));
    }
    let source = parse(target)?;
    let mut destination = parse(temporary_guid)?;
    let mut pointer = &mut destination as *mut GUID;
    // SAFETY: source is the existing scheme; destination is an initialized, pre-journaled new GUID.
    // Passing a non-null destination pointer avoids an unjournaled, OS-generated scheme identity.
    code(
        unsafe { PowerDuplicateScheme(null_mut(), &source, &mut pointer) },
        "Windows could not create a temporary power plan. This control is unavailable.",
    )?;
    // SAFETY: only the private destination scheme is written; AC values are never modified.
    code(
        unsafe {
            PowerWriteDCValueIndex(
                null_mut(),
                &destination,
                &GUID_PROCESSOR_SETTINGS_SUBGROUP,
                &GUID_PROCESSOR_THROTTLE_MAXIMUM,
                value,
            )
        },
        "Windows refused the temporary processor limit.",
    )?;
    let name = super::wide("BatteryDeadline Temporary");
    // SAFETY: name is a live UTF-16 buffer and length includes its null terminator. Failure is cosmetic.
    unsafe {
        PowerWriteFriendlyName(
            null_mut(),
            &destination,
            null(),
            null(),
            name.as_ptr().cast(),
            (name.len() * 2) as u32,
        );
    }
    // SAFETY: the scheme is already journaled and populated before activation.
    code(
        unsafe { PowerSetActiveScheme(null_mut(), &destination) },
        "Windows refused to activate the temporary power plan.",
    )?;
    if format(active()?) != *temporary_guid || maximum(&destination)? != value {
        return Err(Error::Hardware(
            "The temporary power plan change could not be verified.".into(),
        ));
    }
    Ok(())
}
fn exists(guid: &GUID) -> Result<bool> {
    for index in 0..1024 {
        let mut candidate = GUID::from_u128(0);
        let mut size = std::mem::size_of::<GUID>() as u32;
        // SAFETY: AccessScheme enumeration writes a GUID to this exactly sized, aligned buffer.
        let result = unsafe {
            PowerEnumerate(
                null_mut(),
                null(),
                null(),
                ACCESS_SCHEME,
                index,
                (&mut candidate as *mut GUID).cast(),
                &mut size,
            )
        };
        if result == 259 {
            return Ok(false);
        }
        code(
            result,
            "Windows could not check the saved temporary power plan.",
        )?;
        if format(candidate) == format(*guid) {
            return Ok(true);
        }
    }
    Err(Error::Hardware(
        "Too many power schemes to safely verify temporary-plan cleanup.".into(),
    ))
}
pub fn restore(snapshot: &Snapshot, applied: u32) -> Result<RestoreOutcome> {
    let Snapshot::CpuPolicy {
        target,
        original: _,
        temporary_guid,
    } = snapshot
    else {
        return Err(Error::InvalidInput(
            "Invalid power policy recovery record.".into(),
        ));
    };
    let current = format(active()?);
    let outcome = if current == *temporary_guid {
        if maximum(&parse(temporary_guid)?)? != applied {
            // The user adopted/edited our active clone. Keep it; deleting an active user policy is unsafe.
            return Ok(RestoreOutcome::UserOverride);
        }
        let original = parse(target)?;
        // SAFETY: original scheme was saved before cloning. No user scheme values are edited.
        code(
            unsafe { PowerSetActiveScheme(null_mut(), &original) },
            "Reconnect or retry to restore the original power plan.",
        )?;
        if format(active()?) != *target {
            return Err(Error::Hardware(
                "Restoring the original power plan could not be verified.".into(),
            ));
        }
        RestoreOutcome::Restored
    } else if current == *target {
        RestoreOutcome::AlreadyOriginal
    } else {
        RestoreOutcome::UserOverride
    };
    let temporary = parse(temporary_guid)?;
    if exists(&temporary)? {
        // SAFETY: the GUID was generated and journaled exclusively for our private clone, now inactive.
        code(
            unsafe { PowerDeleteScheme(null_mut(), &temporary) },
            "The temporary plan could not be removed. Retry Restore.",
        )?;
    }
    Ok(outcome)
}
