use crate::{Result, Telemetry, TelemetryQuality};
use chrono::{DateTime, Utc};
use std::{
    hash::{Hash, Hasher},
    mem::size_of,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Devices::DeviceAndDriverInstallation::*,
    Foundation::{GENERIC_READ, INVALID_HANDLE_VALUE},
    Storage::FileSystem::*,
    System::{IO::DeviceIoControl, Power::*},
};

struct Device {
    handle: OwnedHandle,
    identity: u64,
}
struct DeviceSet(HDEVINFO);
impl Drop for DeviceSet {
    fn drop(&mut self) {
        // SAFETY: this owns the successful SetupDiGetClassDevsW handle and releases it once.
        unsafe {
            SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}

#[derive(Default)]
pub struct BatteryReader {
    devices: Vec<Device>,
    last_scan: Option<DateTime<Utc>>,
    incomplete: bool,
}

pub fn system_status() -> Result<SYSTEM_POWER_STATUS> {
    let mut status = SYSTEM_POWER_STATUS::default();
    // SAFETY: status is a correctly sized, writable SYSTEM_POWER_STATUS value.
    if unsafe { GetSystemPowerStatus(&mut status) } == 0 {
        return Err(super::api_error(
            "Windows could not report the current power source. Wait and retry.",
        ));
    }
    Ok(status)
}

fn ioctl<I: Copy, O: Default + Copy>(device: &Device, code: u32, input: &I) -> Result<O> {
    let mut output = O::default();
    let mut returned = 0;
    // SAFETY: private call sites use SDK POD types with correct IOCTL input/output sizes.
    // The live OwnedHandle and buffers remain valid for this synchronous operation.
    let ok = unsafe {
        DeviceIoControl(
            device.handle.as_raw_handle(),
            code,
            (input as *const I).cast(),
            size_of::<I>() as u32,
            (&mut output as *mut O).cast(),
            size_of::<O>() as u32,
            &mut returned,
            null_mut(),
        )
    };
    if ok == 0 || returned < size_of::<O>() as u32 {
        return Err(super::api_error(
            "Detailed battery readings are unavailable.",
        ));
    }
    Ok(output)
}

impl BatteryReader {
    fn enumerate(&mut self, now: DateTime<Utc>) -> Result<()> {
        self.devices.clear();
        self.incomplete = false;
        self.last_scan = Some(now);
        // SAFETY: GUID is an SDK constant; null optional arguments and documented flags are valid.
        let handle = unsafe {
            SetupDiGetClassDevsW(
                &GUID_DEVICE_BATTERY,
                null(),
                null_mut(),
                DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
            )
        };
        if handle == -1 {
            return Err(super::api_error(
                "Windows battery devices could not be enumerated.",
            ));
        }
        let set = DeviceSet(handle);
        for index in 0..32 {
            let mut interface = SP_DEVICE_INTERFACE_DATA {
                cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                ..Default::default()
            };
            // SAFETY: set is live, interface is correctly sized, and the GUID matches the enumeration.
            if unsafe {
                SetupDiEnumDeviceInterfaces(
                    set.0,
                    null(),
                    &GUID_DEVICE_BATTERY,
                    index,
                    &mut interface,
                )
            } == 0
            {
                break;
            }
            let mut required = 0;
            // SAFETY: this first call only requests the required buffer size.
            unsafe {
                SetupDiGetDeviceInterfaceDetailW(
                    set.0,
                    &interface,
                    null_mut(),
                    0,
                    &mut required,
                    null_mut(),
                );
            }
            if !(8..=1_048_576).contains(&required) {
                self.incomplete = true;
                continue;
            }
            // u64 storage guarantees the alignment expected by the x64 SDK structure.
            let mut buffer = vec![0_u64; (required as usize).div_ceil(8)];
            let detail = buffer
                .as_mut_ptr()
                .cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
            // SAFETY: buffer is aligned and at least required bytes long. DevicePath starts at byte 4.
            let path = unsafe {
                (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
                if SetupDiGetDeviceInterfaceDetailW(
                    set.0,
                    &interface,
                    detail,
                    required,
                    null_mut(),
                    null_mut(),
                ) == 0
                {
                    self.incomplete = true;
                    continue;
                }
                let chars = std::slice::from_raw_parts(
                    (*detail).DevicePath.as_ptr(),
                    (required as usize - 4) / 2,
                );
                super::from_wide(chars)
            };
            let wide = super::wide(&path);
            // SAFETY: wide is null terminated and valid for this call. Only battery read access is requested.
            let file = unsafe {
                CreateFileW(
                    wide.as_ptr(),
                    GENERIC_READ,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    null(),
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL,
                    null_mut(),
                )
            };
            if file == INVALID_HANDLE_VALUE {
                self.incomplete = true;
                continue;
            }
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            path.hash(&mut hasher);
            // SAFETY: successful CreateFileW returned an owned, non-null handle, transferred exactly once.
            self.devices.push(Device {
                handle: unsafe { OwnedHandle::from_raw_handle(file) },
                identity: hasher.finish(),
            });
        }
        Ok(())
    }

    pub fn sample(&mut self, now: DateTime<Utc>) -> Result<Telemetry> {
        let basic = system_status()?;
        if self.last_scan.is_none_or(|t| (now - t).num_seconds() >= 60)
            && let Err(error) = self.enumerate(now)
        {
            tracing::debug!(error=%error,"Using basic battery telemetry");
            self.incomplete = true;
        }
        let mut records = Vec::new();
        let mut all_ok = !self.incomplete;
        for device in &self.devices {
            let record = (|| -> Result<_> {
                let tag: u32 = ioctl(device, IOCTL_BATTERY_QUERY_TAG, &0_u32)?;
                if tag == 0 {
                    return Err(crate::Error::Hardware("Battery tag is unavailable.".into()));
                }
                let info: BATTERY_INFORMATION = ioctl(
                    device,
                    IOCTL_BATTERY_QUERY_INFORMATION,
                    &BATTERY_QUERY_INFORMATION {
                        BatteryTag: tag,
                        InformationLevel: BatteryInformation,
                        AtRate: 0,
                    },
                )?;
                if info.Capabilities & BATTERY_SYSTEM_BATTERY == 0
                    || info.Capabilities & BATTERY_IS_SHORT_TERM != 0
                {
                    return Ok(None);
                }
                let status: BATTERY_STATUS = ioctl(
                    device,
                    IOCTL_BATTERY_QUERY_STATUS,
                    &BATTERY_WAIT_STATUS {
                        BatteryTag: tag,
                        Timeout: 0,
                        PowerState: 0,
                        LowCapacity: 0,
                        HighCapacity: u32::MAX,
                    },
                )?;
                Ok(Some((device.identity, tag, info, status)))
            })();
            match record {
                Ok(Some(record)) => records.push(record),
                Ok(None) => {}
                Err(_) => {
                    all_ok = false;
                }
            }
        }
        let ac = match basic.ACLineStatus {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        };
        let mut t = Telemetry {
            timestamp: now,
            battery_present: basic.BatteryFlag != 128
                && (basic.BatteryFlag != 255 || basic.BatteryLifePercent <= 100),
            ac_connected: ac,
            charging: basic.BatteryFlag != 255 && basic.BatteryFlag & 8 != 0,
            percentage: (basic.BatteryLifePercent <= 100)
                .then_some(basic.BatteryLifePercent as f64),
            remaining_wh: None,
            full_charge_wh: None,
            design_wh: None,
            discharge_w: None,
            voltage_v: None,
            os_remaining_minutes: (basic.BatteryLifeTime != u32::MAX)
                .then_some(basic.BatteryLifeTime as f64 / 60.0),
            quality: TelemetryQuality::Poor,
            battery_identity: "windows-basic".into(),
            explanation: "Using Windows battery percentage. Detailed readings are unavailable."
                .into(),
        };
        if !records.is_empty() {
            t.battery_present = true;
            t.battery_identity = records
                .iter()
                .map(|(id, tag, _, _)| format!("{id:x}:{tag}"))
                .collect::<Vec<_>>()
                .join("+");
        }
        let absolute = all_ok
            && !records.is_empty()
            && records.iter().all(|(_, _, i, s)| {
                i.Capabilities & BATTERY_CAPACITY_RELATIVE == 0
                    && i.FullChargedCapacity != BATTERY_UNKNOWN_CAPACITY
                    && i.FullChargedCapacity > 0
                    && s.Capacity != BATTERY_UNKNOWN_CAPACITY
                    && (s.Capacity as f64) <= i.FullChargedCapacity as f64 * 1.1
            });
        if absolute {
            let current: f64 = records
                .iter()
                .map(|(_, _, _, s)| s.Capacity as f64 / 1000.0)
                .sum();
            let full: f64 = records
                .iter()
                .map(|(_, _, i, _)| i.FullChargedCapacity as f64 / 1000.0)
                .sum();
            t.remaining_wh = Some(current);
            t.full_charge_wh = Some(full);
            t.percentage = Some((current / full * 100.0).clamp(0.0, 100.0));
            if records.iter().all(|(_, _, i, _)| {
                i.DesignedCapacity != BATTERY_UNKNOWN_CAPACITY && i.DesignedCapacity > 0
            }) {
                t.design_wh = Some(
                    records
                        .iter()
                        .map(|(_, _, i, _)| i.DesignedCapacity as f64 / 1000.0)
                        .sum(),
                );
            }
            t.quality = TelemetryQuality::Good;
            t.explanation="Battery capacity is available. Estimating power when the hardware rate is missing.".into();
            // All system batteries must have a known signed rate; partial sums are unsafe.
            if ac == Some(false)
                && records
                    .iter()
                    .all(|(_, _, _, s)| s.Rate as u32 != BATTERY_UNKNOWN_RATE && s.Rate <= 0)
            {
                let power: f64 = records
                    .iter()
                    .map(|(_, _, _, s)| -(s.Rate as f64) / 1000.0)
                    .sum();
                if (0.1..=300.0).contains(&power) {
                    t.discharge_w = Some(power);
                    t.quality = TelemetryQuality::Excellent;
                    t.explanation =
                        "Absolute capacity and real-time power from Windows battery devices."
                            .into();
                }
            }
            if records.len() == 1 && records[0].3.Voltage != BATTERY_UNKNOWN_VOLTAGE {
                t.voltage_v = Some(records[0].3.Voltage as f64 / 1000.0);
            }
        }
        Ok(t)
    }
}
