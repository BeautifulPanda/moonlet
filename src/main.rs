// No console window.
#![windows_subsystem = "windows"]

mod app;
#[cfg(any(feature = "launcher", all(feature = "settings", feature = "orbit")))]
mod apps;
mod autostart;
#[cfg(any(feature = "launcher", feature = "settings"))]
mod clipboard;
mod config;
#[cfg(feature = "launcher")]
mod hotkey;
mod icon;
mod lang;
#[cfg(feature = "timers")]
mod reminders;
mod screen;
#[cfg(feature = "settings")]
mod settings;
#[cfg(feature = "timers")]
mod sound;
mod toml;
mod tray;
#[cfg(feature = "update")]
mod update;
mod watch;

#[cfg(feature = "orbit")]
mod fullscreen;
#[cfg(feature = "gfx")]
mod gfx;
#[cfg(feature = "orbit")]
mod orbit;
#[cfg(feature = "orbit")]
mod power;
#[cfg(feature = "orbit")]
mod tile;

use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
use windows::core::{HSTRING, w};

fn main() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    #[cfg(feature = "settings")]
    if std::env::args().any(|arg| arg == "--settings") {
        settings::run(std::env::args().any(|arg| arg == "--news"));
        return;
    }

    let updated = std::env::args().any(|arg| arg == "--updated");

    let mut instance = app::SingleInstance::acquire();
    for _ in 0..50 {
        if instance.is_some() || !updated {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
        instance = app::SingleInstance::acquire();
    }
    let Some(_instance) = instance else {
        return;
    };

    if let Err(err) = app::run(updated) {
        let text = HSTRING::from(format!("Moonlet failed to start:\n{err}"));
        unsafe {
            MessageBoxW(None, &text, w!("Moonlet"), MB_OK | MB_ICONERROR);
        }
    }
}
