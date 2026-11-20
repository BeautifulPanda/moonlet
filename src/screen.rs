// Primary monitor rectangle and DPI. With per-monitor awareness
// GetDpiForSystem stays fixed for the whole session, so ask the monitor.

use windows::Win32::Foundation::POINT;
#[cfg(feature = "orbit")]
use windows::Win32::Foundation::RECT;
#[cfg(feature = "orbit")]
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITORINFO};
use windows::Win32::Graphics::Gdi::{HMONITOR, MONITOR_DEFAULTTOPRIMARY, MonitorFromPoint};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

fn primary() -> HMONITOR {
    unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) }
}

pub fn dpi() -> u32 {
    let (mut dpi, mut dpi_y) = (96, 96);
    unsafe {
        let _ = GetDpiForMonitor(primary(), MDT_EFFECTIVE_DPI, &mut dpi, &mut dpi_y);
    }
    dpi
}

#[cfg(feature = "orbit")]
pub fn rect() -> RECT {
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        let _ = GetMonitorInfoW(primary(), &mut info);
    }
    info.rcMonitor
}
