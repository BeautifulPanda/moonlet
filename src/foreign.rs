#[cfg(feature = "notes")]
use windows::Win32::Foundation::RECT;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
#[cfg(feature = "notes")]
use windows::Win32::Graphics::Dwm::DWMWA_EXTENDED_FRAME_BOUNDS;
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
#[cfg(feature = "notes")]
use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible,
};
use windows::core::{BOOL, PWSTR};

const BROWSERS: [&str; 12] = [
    "chrome.exe",
    "msedge.exe",
    "firefox.exe",
    "brave.exe",
    "opera.exe",
    "vivaldi.exe",
    "browser.exe",
    "yandex.exe",
    "arc.exe",
    "zen.exe",
    "librewolf.exe",
    "floorp.exe",
];

pub fn is_browser(exe: &str) -> bool {
    BROWSERS.contains(&exe)
}

#[cfg(feature = "scenarios")]
pub fn is_firefox_like(exe: &str) -> bool {
    matches!(
        exe,
        "firefox.exe" | "zen.exe" | "librewolf.exe" | "floorp.exe"
    )
}

pub fn exe_path(pid: u32) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 520];
        let mut size = buffer.len() as u32;
        let named = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(process);
        named.ok()?;
        Some(String::from_utf16_lossy(&buffer[..size as usize]))
    }
}

pub fn file_name(path: &str) -> String {
    path.rsplit(['\\', '/'])
        .next()
        .unwrap_or(path)
        .to_lowercase()
}

#[cfg(feature = "notes")]
pub fn exe_of_process(pid: u32) -> Option<String> {
    exe_path(pid).map(|path| file_name(&path))
}

pub fn pid_of(hwnd: HWND) -> u32 {
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    pid
}

pub fn title_of(hwnd: HWND) -> String {
    unsafe {
        let length = GetWindowTextLengthW(hwnd);
        if length <= 0 {
            return String::new();
        }
        let mut buffer = vec![0u16; length as usize + 1];
        let copied = GetWindowTextW(hwnd, &mut buffer);
        String::from_utf16_lossy(&buffer[..copied.max(0) as usize])
    }
}

#[cfg(feature = "notes")]
pub fn bounds(hwnd: HWND) -> Option<RECT> {
    let mut rect = RECT::default();
    unsafe {
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut rect as *mut RECT).cast(),
            size_of::<RECT>() as u32,
        )
        .is_err()
        {
            GetWindowRect(hwnd, &mut rect).ok()?;
        }
    }
    Some(rect)
}

pub fn cloaked(hwnd: HWND) -> bool {
    let mut value = 0u32;
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&mut value as *mut u32).cast(),
            size_of::<u32>() as u32,
        )
        .is_ok()
            && value != 0
    }
}

pub fn class_of(hwnd: HWND) -> String {
    let mut buffer = [0u16; 64];
    let length = unsafe { GetClassNameW(hwnd, &mut buffer) };
    String::from_utf16_lossy(&buffer[..length.max(0) as usize])
}

pub fn is_shell(class: &str) -> bool {
    matches!(
        class,
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
    )
}

pub fn windows_top_down() -> Vec<HWND> {
    unsafe extern "system" fn collect(hwnd: HWND, list: LPARAM) -> BOOL {
        let list = unsafe { &mut *(list.0 as *mut Vec<HWND>) };
        if unsafe { IsWindowVisible(hwnd) }.as_bool() {
            list.push(hwnd);
        }
        BOOL(1)
    }
    let mut list: Vec<HWND> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(collect), LPARAM(&mut list as *mut Vec<HWND> as isize));
    }
    list
}
