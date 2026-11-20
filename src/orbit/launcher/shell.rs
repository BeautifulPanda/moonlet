use std::path::Path;

use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::UI::Shell::{
    ASSOCF_NONE, ASSOCSTR_EXECUTABLE, AssocQueryStringW, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, SW_SHOWNORMAL};
use windows::core::{HSTRING, PCWSTR, PWSTR, w};

const CF_UNICODETEXT: u32 = 13;
const FALLBACK_SEARCH: &str = "https://www.google.com/search?q=";

pub fn open(target: &str, parameters: Option<&str>) {
    // A started app may already be running (Telegram, Spotify) and wants to bring
    // its window to the front; without this Windows only flashes it on the taskbar.
    unsafe {
        let _ = AllowSetForegroundWindow(u32::MAX);
        let parameters = parameters.map(HSTRING::from);
        ShellExecuteW(
            None,
            w!("open"),
            &HSTRING::from(target),
            parameters
                .as_ref()
                .map_or(PCWSTR::null(), |p| PCWSTR(p.as_ptr())),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

pub fn launch(path: &Path) {
    open(&path.to_string_lossy(), None);
}

pub fn web_search(query: &str, template: &str) {
    if template.is_empty() && search_in_browser(query) {
        return;
    }
    let base = if template.is_empty() {
        FALLBACK_SEARCH
    } else {
        template
    };
    open(&format!("{base}{}", encode(query)), None);
}

fn search_in_browser(query: &str) -> bool {
    let Some(browser) = default_browser() else {
        return false;
    };
    let name = Path::new(&browser)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let query = query.replace('"', " ");
    let arguments = match name.as_str() {
        "chrome" | "msedge" | "browser" | "opera" | "brave" | "vivaldi" | "chromium" => {
            format!("\"? {query}\"")
        }
        "firefox" => format!("-search \"{query}\""),
        _ => return false,
    };
    open(&browser, Some(&arguments));
    true
}

fn default_browser() -> Option<String> {
    let mut buffer = [0u16; 520];
    let mut length = buffer.len() as u32;
    let result = unsafe {
        AssocQueryStringW(
            ASSOCF_NONE,
            ASSOCSTR_EXECUTABLE,
            w!("https"),
            w!("open"),
            Some(PWSTR(buffer.as_mut_ptr())),
            &mut length,
        )
    };
    if result.is_err() {
        return None;
    }
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..end]))
}

pub fn encode(text: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(text.len() * 3);
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 15) as usize] as char);
        }
    }
    out
}

pub fn copy(owner: HWND, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    unsafe {
        if OpenClipboard(Some(owner)).is_err() {
            return;
        }
        let _ = EmptyClipboard();
        if let Ok(memory) = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2) {
            let target = GlobalLock(memory) as *mut u16;
            if !target.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), target, wide.len());
                let _ = GlobalUnlock(memory);
                let _ = SetClipboardData(CF_UNICODETEXT, Some(HANDLE(memory.0)));
            }
        }
        let _ = CloseClipboard();
    }
}

pub fn paste(owner: HWND) -> Option<String> {
    unsafe {
        OpenClipboard(Some(owner)).ok()?;
        let text = GetClipboardData(CF_UNICODETEXT).ok().and_then(|handle| {
            let memory = HGLOBAL(handle.0);
            let source = GlobalLock(memory) as *const u16;
            if source.is_null() {
                return None;
            }
            let mut length = 0;
            while *source.add(length) != 0 {
                length += 1;
            }
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(source, length));
            let _ = GlobalUnlock(memory);
            Some(text)
        });
        let _ = CloseClipboard();
        text.map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_queries() {
        assert_eq!(encode("a b&c"), "a%20b%26c");
        assert_eq!(encode("кот"), "%D0%BA%D0%BE%D1%82");
    }
}
