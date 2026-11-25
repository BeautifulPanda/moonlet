use windows::Win32::Foundation::{HWND, PROPERTYKEY};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomation2, IUIAutomationValuePattern, TreeScope_Descendants,
    UIA_ControlTypePropertyId, UIA_EditControlTypeId, UIA_ValuePatternId,
};
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow};
use windows::Win32::UI::Shell::{
    Folder2, IShellFolderViewDual, IShellWindows, IWebBrowser2, ShellWindows,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GW_OWNER, GWL_EXSTYLE, GetWindow, GetWindowLongW, GetWindowPlacement, SW_SHOWMAXIMIZED,
    SW_SHOWMINIMIZED, WINDOWPLACEMENT, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};
use windows::core::{BSTR, Interface};

use super::store::{self, Scenario, Screen, State, Window};
use crate::foreign::{
    class_of, cloaked, exe_path, file_name, is_browser, is_shell, pid_of, title_of,
    windows_top_down,
};
use crate::lang::tr;

const APP_ID_KEY: PROPERTYKEY = PROPERTYKEY {
    fmtid: windows::core::GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
    pid: 5,
};

#[derive(Clone, Debug)]
pub struct Open {
    pub hwnd: HWND,
    pub exe: String,
    pub exe_name: String,
    pub title: String,
    pub app: String,
    pub folder: String,
}

fn is_app_window(hwnd: HWND) -> bool {
    unsafe {
        if cloaked(hwnd) {
            return false;
        }
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        let owned = GetWindow(hwnd, GW_OWNER).is_ok_and(|o| !o.is_invalid());
        if ex & WS_EX_APPWINDOW.0 == 0
            && (owned || ex & (WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0) != 0)
        {
            return false;
        }
    }
    let class = class_of(hwnd);
    !is_shell(&class) && class != "Windows.UI.Core.CoreWindow" && !title_of(hwnd).is_empty()
}

pub fn app_id(hwnd: HWND) -> String {
    unsafe {
        let Ok(store) = SHGetPropertyStoreForWindow::<IPropertyStore>(hwnd) else {
            return String::new();
        };
        store
            .GetValue(&APP_ID_KEY)
            .ok()
            .and_then(|value| BSTR::try_from(&value).ok())
            .map(|text| text.to_string())
            .unwrap_or_default()
    }
}

fn own_exe() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

pub fn open_windows(folders: bool) -> Vec<Open> {
    let own_pid = unsafe { GetCurrentProcessId() };
    let own = own_exe();
    let paths = if folders {
        explorer_folders()
    } else {
        Vec::new()
    };
    let mut list = Vec::new();
    for hwnd in windows_top_down() {
        let pid = pid_of(hwnd);
        if pid == own_pid || !is_app_window(hwnd) {
            continue;
        }
        let Some(exe) = exe_path(pid) else {
            continue;
        };
        if exe.to_lowercase() == own {
            continue;
        }
        let exe_name = file_name(&exe);
        let app = if exe_name == "applicationframehost.exe" {
            app_id(hwnd)
        } else {
            String::new()
        };
        let folder = paths
            .iter()
            .find(|(h, _)| *h == hwnd.0 as isize)
            .map(|(_, f)| f.clone())
            .unwrap_or_default();
        list.push(Open {
            hwnd,
            title: title_of(hwnd),
            exe,
            exe_name,
            app,
            folder,
        });
    }
    list
}

fn explorer_folders() -> Vec<(isize, String)> {
    let mut out = Vec::new();
    unsafe {
        let Ok(windows) = CoCreateInstance::<_, IShellWindows>(&ShellWindows, None, CLSCTX_ALL)
        else {
            return out;
        };
        let count = windows.Count().unwrap_or(0);
        for index in 0..count {
            let Ok(item) = windows.Item(&VARIANT::from(index)) else {
                continue;
            };
            let Ok(browser) = item.cast::<IWebBrowser2>() else {
                continue;
            };
            let Ok(hwnd) = browser.HWND() else {
                continue;
            };
            let mut folder = browser
                .LocationURL()
                .map(|url| folder_from_url(&url.to_string()))
                .unwrap_or_default();
            if folder.is_empty() {
                folder = view_path(&browser).unwrap_or_default();
            }
            out.push((hwnd.0, folder));
        }
    }
    out
}

fn view_path(browser: &IWebBrowser2) -> Option<String> {
    unsafe {
        let view: IShellFolderViewDual = browser.Document().ok()?.cast().ok()?;
        let folder: Folder2 = view.Folder().ok()?.cast().ok()?;
        let path = folder.Self_().ok()?.Path().ok()?.to_string();
        (!path.is_empty()).then_some(path)
    }
}

// "file:///C:/Users/Me/My%20Docs" → "C:\Users\Me\My Docs";
pub fn folder_from_url(url: &str) -> String {
    let Some(rest) = url.strip_prefix("file:") else {
        return String::new();
    };
    let path = match rest.strip_prefix("///") {
        Some(local) => local.to_string(),
        None => match rest.strip_prefix("//") {
            Some(share) => format!("//{share}"),
            None => return String::new(),
        },
    };
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(byte) = path
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            decoded.push(byte);
            i += 3;
            continue;
        }
        decoded.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&decoded).replace('/', "\\")
}

fn active_url(automation: &IUIAutomation, hwnd: HWND) -> Option<String> {
    unsafe {
        let root = automation.ElementFromHandle(hwnd).ok()?;
        let condition = automation
            .CreatePropertyCondition(
                UIA_ControlTypePropertyId,
                &VARIANT::from(UIA_EditControlTypeId.0),
            )
            .ok()?;
        let edit = root.FindFirst(TreeScope_Descendants, &condition).ok()?;
        let pattern: IUIAutomationValuePattern =
            edit.GetCurrentPatternAs(UIA_ValuePatternId).ok()?;
        let value = pattern.CurrentValue().ok()?.to_string();
        normalize_url(&value)
    }
}

pub fn normalize_url(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || text.contains(char::is_whitespace) {
        return None;
    }
    if text.contains("://") || text.starts_with("about:") {
        return Some(text.to_string());
    }
    (text.contains('.') || text.contains(':')).then(|| format!("https://{text}"))
}

fn placement(hwnd: HWND) -> Option<([i32; 4], State)> {
    let mut wp = WINDOWPLACEMENT {
        length: size_of::<WINDOWPLACEMENT>() as u32,
        ..Default::default()
    };
    unsafe {
        GetWindowPlacement(hwnd, &mut wp).ok()?;
    }
    let state = match wp.showCmd {
        cmd if cmd == SW_SHOWMAXIMIZED.0 as u32 => State::Maximized,
        cmd if cmd == SW_SHOWMINIMIZED.0 as u32 => State::Minimized,
        _ => State::Normal,
    };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let _ = GetMonitorInfoW(monitor, &mut info);
    }
    let dx = info.rcWork.left - info.rcMonitor.left;
    let dy = info.rcWork.top - info.rcMonitor.top;
    let r = wp.rcNormalPosition;
    Some((
        [r.left + dx, r.top + dy, r.right + dx, r.bottom + dy],
        state,
    ))
}

pub fn capture(name: &str) -> Result<usize, String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let result = capture_inner(name);
    unsafe {
        CoUninitialize();
    }
    result
}

fn capture_inner(name: &str) -> Result<usize, String> {
    let monitors = super::monitors();
    let screens: Vec<Screen> = monitors
        .iter()
        .map(|m| Screen {
            device: m.device.clone(),
            work: m.work,
            dpi: m.dpi,
        })
        .collect();
    let automation: Option<IUIAutomation> =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok() };
    if let Some(a2) = automation
        .as_ref()
        .and_then(|a| a.cast::<IUIAutomation2>().ok())
    {
        unsafe {
            let _ = a2.SetConnectionTimeout(2000);
            let _ = a2.SetTransactionTimeout(2000);
        }
    }
    let mut windows = Vec::new();
    for open in open_windows(true) {
        let Some((rect, state)) = placement(open.hwnd) else {
            continue;
        };
        let (cx, cy) = ((rect[0] + rect[2]) / 2, (rect[1] + rect[3]) / 2);
        let screen = monitors
            .iter()
            .position(|m| cx >= m.rect[0] && cx < m.rect[2] && cy >= m.rect[1] && cy < m.rect[3])
            .unwrap_or(0);
        let mut urls = Vec::new();
        if is_browser(&open.exe_name)
            && let Some(automation) = &automation
        {
            let url = active_url(automation, open.hwnd).or_else(|| {
                std::thread::sleep(std::time::Duration::from_millis(300));
                active_url(automation, open.hwnd)
            });
            urls.extend(url);
        }
        windows.push(Window {
            screen,
            state,
            rect,
            exe: open.exe,
            title: open.title,
            app: open.app,
            folder: open.folder,
            urls,
        });
    }
    drop(automation);
    if windows.is_empty() {
        return Err(tr("нет открытых окон", "no open windows").to_string());
    }
    let count = windows.len();
    let scenario = Scenario {
        name: name.to_string(),
        screens,
        windows,
    };
    store::save(&scenario)
        .map_err(|_| tr("не удалось записать файл", "could not write the file").to_string())?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_folder_urls() {
        assert_eq!(
            folder_from_url("file:///C:/Users/Me/My%20Docs"),
            "C:\\Users\\Me\\My Docs"
        );
        assert_eq!(
            folder_from_url("file:///D:/%D0%9F%D0%B0%D0%BF%D0%BA%D0%B0"),
            "D:\\Папка"
        );
        assert_eq!(folder_from_url("file://server/share"), "\\\\server\\share");
        assert_eq!(folder_from_url(""), "");
        assert_eq!(folder_from_url("https://x.y"), "");
    }

    #[test]
    fn cleans_address_bar_text() {
        assert_eq!(
            normalize_url("github.com/BeautifulPanda").as_deref(),
            Some("https://github.com/BeautifulPanda")
        );
        assert_eq!(
            normalize_url("http://localhost:3000").as_deref(),
            Some("http://localhost:3000")
        );
        assert_eq!(
            normalize_url("chrome://settings").as_deref(),
            Some("chrome://settings")
        );
        assert_eq!(normalize_url(""), None);
        assert_eq!(normalize_url("как сварить кофе"), None);
        assert_eq!(normalize_url("погода"), None);
    }
}
