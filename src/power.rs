use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, WPARAM};
use windows::Win32::System::Power::{
    GetSystemPowerStatus, HPOWERNOTIFY, POWERBROADCAST_SETTING, RegisterPowerSettingNotification,
    SYSTEM_POWER_STATUS, UnregisterPowerSettingNotification,
};
use windows::Win32::System::SystemServices::{
    GUID_ACDC_POWER_SOURCE, GUID_BATTERY_PERCENTAGE_REMAINING,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DEVICE_NOTIFY_WINDOW_HANDLE, PBT_APMPOWERSTATUSCHANGE, PBT_APMRESUMEAUTOMATIC,
    PBT_POWERSETTINGCHANGE,
};
use windows::core::GUID;

pub struct PowerWatch(Vec<HPOWERNOTIFY>);

impl PowerWatch {
    pub fn register(hwnd: HWND) -> Self {
        let settings = [GUID_ACDC_POWER_SOURCE, GUID_BATTERY_PERCENTAGE_REMAINING];
        let handles = settings
            .iter()
            .filter_map(|setting| unsafe {
                RegisterPowerSettingNotification(
                    HANDLE(hwnd.0),
                    setting,
                    DEVICE_NOTIFY_WINDOW_HANDLE,
                )
                .ok()
            })
            .collect();
        Self(handles)
    }
}

impl Drop for PowerWatch {
    fn drop(&mut self) {
        for &handle in &self.0 {
            unsafe {
                let _ = UnregisterPowerSettingNotification(handle);
            }
        }
    }
}

pub fn on_battery() -> bool {
    let mut status = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut status) }.is_ok() && status.ACLineStatus == 0
}

pub fn has_battery() -> bool {
    #[cfg(feature = "dev")]
    if std::env::var_os("MOONLET_NO_BATTERY").is_some() {
        return false;
    }
    let mut status = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut status) }.is_ok()
        && status.BatteryFlag & 128 == 0
        && status.BatteryFlag != 255
}

pub enum PowerEvent {
    Source(bool),
    Battery,
    Resumed,
}

pub fn parse(wparam: WPARAM, lparam: LPARAM) -> Option<PowerEvent> {
    match wparam.0 as u32 {
        PBT_APMPOWERSTATUSCHANGE => Some(PowerEvent::Battery),
        PBT_APMRESUMEAUTOMATIC => Some(PowerEvent::Resumed),
        PBT_POWERSETTINGCHANGE => {
            let setting = unsafe { &*(lparam.0 as *const POWERBROADCAST_SETTING) };
            if setting.DataLength < 4 {
                return None;
            }
            let value = unsafe { setting.Data.as_ptr().cast::<u32>().read_unaligned() };
            let guid: GUID = setting.PowerSetting;
            if guid == GUID_ACDC_POWER_SOURCE {
                Some(PowerEvent::Source(value != 0))
            } else if guid == GUID_BATTERY_PERCENTAGE_REMAINING {
                Some(PowerEvent::Battery)
            } else {
                None
            }
        }
        _ => None,
    }
}
