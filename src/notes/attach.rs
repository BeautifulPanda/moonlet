use std::collections::HashMap;

use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GA_ROOT, GWL_EXSTYLE, GetAncestor, GetClassNameW, GetWindowLongW, GetWindowRect,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};
use windows::core::{BOOL, PWSTR};

use super::store::Attach;

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

pub fn tab_title(title: &str) -> &str {
    let cut = [" - ", " — "]
        .iter()
        .filter_map(|sep| title.rfind(sep))
        .max();
    match cut {
        Some(at) if at > 0 => title[..at].trim(),
        _ => title.trim(),
    }
}

pub fn exe_of_process(pid: u32) -> Option<String> {
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
        let path = String::from_utf16_lossy(&buffer[..size as usize]);
        let name = path.rsplit('\\').next().unwrap_or(&path);
        Some(name.to_lowercase())
    }
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

fn cloaked(hwnd: HWND) -> bool {
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

fn class_of(hwnd: HWND) -> String {
    let mut buffer = [0u16; 64];
    let length = unsafe { GetClassNameW(hwnd, &mut buffer) };
    String::from_utf16_lossy(&buffer[..length.max(0) as usize])
}

pub fn usable(hwnd: HWND) -> bool {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return false;
        }
        if GetAncestor(hwnd, GA_ROOT) != hwnd || pid_of(hwnd) == GetCurrentProcessId() {
            return false;
        }
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if ex & (WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0) != 0 || cloaked(hwnd) {
            return false;
        }
    }
    !matches!(
        class_of(hwnd).as_str(),
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
    )
}

pub fn matches(hwnd: HWND, attach: &Attach, exe: &str) -> bool {
    exe == attach.exe && (attach.tab.is_empty() || tab_title(&title_of(hwnd)) == attach.tab)
}

fn windows_top_down() -> Vec<HWND> {
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

pub fn window_at(point: POINT) -> Option<HWND> {
    windows_top_down().into_iter().find(|&hwnd| {
        bounds(hwnd).is_some_and(|r| {
            point.x >= r.left && point.x < r.right && point.y >= r.top && point.y < r.bottom
        }) && usable(hwnd)
    })
}

pub fn find(attach: &Attach) -> (Option<HWND>, Option<u32>) {
    let mut exes: HashMap<u32, Option<String>> = HashMap::new();
    let mut process = None;
    for hwnd in windows_top_down() {
        if !usable(hwnd) {
            continue;
        }
        let pid = pid_of(hwnd);
        let exe = exes.entry(pid).or_insert_with(|| exe_of_process(pid));
        let Some(exe) = exe.as_deref() else {
            continue;
        };
        if exe != attach.exe {
            continue;
        }
        process.get_or_insert(pid);
        if matches(hwnd, attach, exe) {
            return (Some(hwnd), Some(pid));
        }
    }
    (None, process)
}

pub fn label(hwnd: HWND, exe: &str) -> String {
    let title = title_of(hwnd);
    let name = if is_browser(exe) {
        tab_title(&title).to_string()
    } else {
        title.trim().to_string()
    };
    let name = if name.is_empty() {
        exe.trim_end_matches(".exe").to_string()
    } else {
        name
    };
    let mut short: String = name.chars().take(40).collect();
    if name.chars().count() > 40 {
        short.push('…');
    }
    short
}

pub fn anchor(note: RECT, window: RECT) -> (u8, i32, i32) {
    let right = (note.left + note.right) > (window.left + window.right);
    let bottom = (note.top + note.bottom) > (window.top + window.bottom);
    let corner = u8::from(right) | (u8::from(bottom) << 1);
    let (ax, ay) = corner_point(corner, window);
    (corner, note.left - ax, note.top - ay)
}

pub fn corner_point(corner: u8, window: RECT) -> (i32, i32) {
    let x = if corner & 1 != 0 {
        window.right
    } else {
        window.left
    };
    let y = if corner & 2 != 0 {
        window.bottom
    } else {
        window.top
    };
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuts_the_browser_name() {
        assert_eq!(
            tab_title("Moonlet - GitHub - Google Chrome"),
            "Moonlet - GitHub"
        );
        assert_eq!(tab_title("YouTube — Mozilla Firefox"), "YouTube");
        assert_eq!(tab_title("Новая вкладка"), "Новая вкладка");
        assert_eq!(tab_title(" - Google Chrome"), "- Google Chrome");
    }

    #[test]
    fn anchors_to_the_nearest_corner() {
        let window = RECT {
            left: 100,
            top: 100,
            right: 900,
            bottom: 700,
        };
        let note = |left, top| RECT {
            left,
            top,
            right: left + 200,
            bottom: top + 100,
        };
        assert_eq!(anchor(note(120, 130), window), (0, 20, 30));
        let (corner, dx, dy) = anchor(note(680, 580), window);
        assert_eq!((corner, dx, dy), (3, -220, -120));
        let wider = RECT {
            right: 1100,
            ..window
        };
        let (x, y) = corner_point(corner, wider);
        assert_eq!((x + dx, y + dy), (880, 580));
    }
}
