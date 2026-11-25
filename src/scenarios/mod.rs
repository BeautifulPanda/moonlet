pub mod capture;
mod restore;
pub mod store;

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{MONITORINFOF_PRIMARY, PostMessageW, WM_APP};
use windows::core::BOOL;

use crate::lang::tr;

pub const WM_SCENARIOS: u32 = WM_APP + 9;

static ENABLED: AtomicBool = AtomicBool::new(true);
static RUNNING: AtomicBool = AtomicBool::new(false);
static OUTCOME: Mutex<Option<String>> = Mutex::new(None);

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

pub fn take_outcome() -> Option<String> {
    OUTCOME.lock().ok()?.take()
}

fn report(notify: isize, text: String) {
    if let Ok(mut slot) = OUTCOME.lock() {
        *slot = Some(text);
    }
    unsafe {
        let _ = PostMessageW(
            Some(HWND(notify as *mut _)),
            WM_SCENARIOS,
            WPARAM(0),
            LPARAM(0),
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    AsSaved,
    Screen(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    pub device: String,
    pub rect: [i32; 4],
    pub work: [i32; 4],
    pub dpi: u32,
    pub primary: bool,
}

fn rect4(rect: RECT) -> [i32; 4] {
    [rect.left, rect.top, rect.right, rect.bottom]
}

pub fn monitors() -> Vec<Monitor> {
    unsafe extern "system" fn collect(
        monitor: HMONITOR,
        _: HDC,
        _: *mut RECT,
        list: LPARAM,
    ) -> BOOL {
        let list = unsafe { &mut *(list.0 as *mut Vec<Monitor>) };
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        let ok = unsafe { GetMonitorInfoW(monitor, (&mut info as *mut MONITORINFOEXW).cast()) };
        if ok.as_bool() {
            let (mut dpi, mut dpi_y) = (96, 96);
            unsafe {
                let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut dpi_y);
            }
            let end = info
                .szDevice
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(info.szDevice.len());
            let base: &MONITORINFO = &info.monitorInfo;
            list.push(Monitor {
                device: String::from_utf16_lossy(&info.szDevice[..end]),
                rect: rect4(base.rcMonitor),
                work: rect4(base.rcWork),
                dpi,
                primary: base.dwFlags & MONITORINFOF_PRIMARY != 0,
            });
        }
        BOOL(1)
    }
    let mut list: Vec<Monitor> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect),
            LPARAM(&mut list as *mut Vec<Monitor> as isize),
        );
    }
    list.sort_by_key(|m| (m.rect[0], m.rect[1]));
    list
}

pub fn monitor_label(index: usize, monitor: &Monitor) -> String {
    let [l, t, r, b] = monitor.rect;
    let mut text = format!("{} {}", tr("экран", "screen"), index + 1);
    if monitor.primary {
        text += " · ";
        text += tr("основной", "main");
    }
    text + &format!(" · {}×{}", r - l, b - t)
}

pub fn windows_text(count: usize) -> String {
    if crate::lang::is_english() {
        return format!("{count} window{}", if count == 1 { "" } else { "s" });
    }
    let word = match (count % 10, count % 100) {
        (1, n) if n != 11 => "окно",
        (2..=4, n) if !(12..=14).contains(&n) => "окна",
        _ => "окон",
    };
    format!("{count} {word}")
}

pub fn remember(name: &str, notify: HWND) {
    let Some(name) = store::clean_name(name) else {
        return;
    };
    let notify = notify.0 as isize;
    std::thread::spawn(move || {
        let text = match capture::capture(&name) {
            Ok(count) => format!(
                "«{name}» {}: {}",
                tr("запомнен", "saved"),
                windows_text(count)
            ),
            Err(error) => format!("«{name}»: {error}"),
        };
        report(notify, text);
    });
}

pub fn run(name: &str, place: Place, notify: HWND) {
    let Some(scenario) = store::find(name) else {
        report(
            notify.0 as isize,
            format!("«{name}»: {}", tr("сценария нет", "no such scenario")),
        );
        return;
    };
    if RUNNING.swap(true, Ordering::AcqRel) {
        report(
            notify.0 as isize,
            tr(
                "Сценарий уже запускается — подождите",
                "A scenario is already starting, please wait",
            )
            .to_string(),
        );
        return;
    }
    let notify = notify.0 as isize;
    std::thread::spawn(move || {
        let text = restore::run(&scenario, place);
        RUNNING.store(false, Ordering::Release);
        report(notify, text);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_windows_in_russian() {
        crate::lang::set(crate::lang::Language::Russian);
        assert_eq!(windows_text(1), "1 окно");
        assert_eq!(windows_text(3), "3 окна");
        assert_eq!(windows_text(5), "5 окон");
        assert_eq!(windows_text(11), "11 окон");
        assert_eq!(windows_text(22), "22 окна");
    }
}
