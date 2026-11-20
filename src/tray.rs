use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIIF_WARNING, NIM_ADD, NIM_DELETE,
    NIM_MODIFY, NIM_SETVERSION, NIN_BALLOONUSERCLICK, NIN_SELECT, NINF_KEY,
    NOTIFY_ICON_INFOTIP_FLAGS, NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CheckMenuRadioItem, CreatePopupMenu, DestroyIcon, DestroyMenu, HICON, HMENU,
    MF_BYCOMMAND, MF_CHECKED, MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING, MF_UNCHECKED,
    PostMessageW, SetForegroundWindow, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    TrackPopupMenu, WM_APP, WM_CONTEXTMENU, WM_NULL,
};
use windows::core::{HSTRING, PCWSTR, Result};

pub const WM_TRAY: u32 = WM_APP + 1;
const ICON_ID: u32 = 1;
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;

pub struct Tray {
    hwnd: HWND,
    icon: HICON,
}

impl Tray {
    pub fn new(hwnd: HWND, icon: HICON) -> Result<Self> {
        let tray = Self { hwnd, icon };
        tray.add()?;
        Ok(tray)
    }

    pub fn add(&self) -> Result<()> {
        let mut data = self.data();
        data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        data.uCallbackMessage = WM_TRAY;
        data.hIcon = self.icon;
        copy_truncated(&mut data.szTip, "Moonlet");
        unsafe {
            Shell_NotifyIconW(NIM_ADD, &data).ok()?;
            data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            Shell_NotifyIconW(NIM_SETVERSION, &data).ok()?;
        }
        Ok(())
    }

    pub fn set_icon(&mut self, icon: HICON) {
        let old = std::mem::replace(&mut self.icon, icon);
        let mut data = self.data();
        data.uFlags = NIF_ICON;
        data.hIcon = icon;
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
            let _ = DestroyIcon(old);
        }
    }

    pub fn notify(&self, title: &str, text: &str) {
        self.balloon(title, text, NIIF_WARNING);
    }

    #[cfg_attr(not(feature = "update"), allow(dead_code))]
    pub fn inform(&self, title: &str, text: &str) {
        self.balloon(title, text, NIIF_INFO);
    }

    fn balloon(&self, title: &str, text: &str, kind: NOTIFY_ICON_INFOTIP_FLAGS) {
        let mut data = self.data();
        data.uFlags = NIF_INFO;
        data.dwInfoFlags = kind;
        copy_truncated(&mut data.szInfoTitle, title);
        copy_truncated(&mut data.szInfo, text);
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
        }
    }

    fn data(&self) -> NOTIFYICONDATAW {
        NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: ICON_ID,
            ..Default::default()
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.data());
            let _ = DestroyIcon(self.icon);
        }
    }
}

fn copy_truncated(field: &mut [u16], text: &str) {
    let wide: Vec<u16> = text.encode_utf16().take(field.len() - 1).collect();
    field[..wide.len()].copy_from_slice(&wide);
    field[wide.len()] = 0;
}

pub fn menu_request(wparam: WPARAM, lparam: LPARAM) -> Option<POINT> {
    let event = (lparam.0 & 0xFFFF) as u32;
    if event != WM_CONTEXTMENU && event != NIN_SELECT && event != NIN_KEYSELECT {
        return None;
    }
    let x = (wparam.0 & 0xFFFF) as i16 as i32;
    let y = ((wparam.0 >> 16) & 0xFFFF) as i16 as i32;
    Some(POINT { x, y })
}

#[cfg_attr(not(feature = "update"), allow(dead_code))]
pub fn balloon_clicked(lparam: LPARAM) -> bool {
    (lparam.0 & 0xFFFF) as u32 == NIN_BALLOONUSERCLICK
}

#[cfg_attr(not(feature = "orbit"), allow(dead_code))]
pub enum MenuItem {
    Action(u32, &'static str),
    #[cfg_attr(not(feature = "update"), allow(dead_code))]
    Label(u32, String),
    Check(u32, String, bool),
    Radio(u32, &'static str, bool),
    #[cfg_attr(not(feature = "update"), allow(dead_code))]
    Submenu(&'static str, Vec<MenuItem>),
    Separator,
}

pub fn show_menu(hwnd: HWND, items: &[MenuItem], at: POINT) -> u32 {
    unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return 0;
        };
        fill(menu, items);

        // Known tray quirk: without this the menu does not close on an outside click.
        let _ = SetForegroundWindow(hwnd);
        let flags = TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY;
        let command = TrackPopupMenu(menu, flags, at.x, at.y, None, hwnd, None);
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        command.0 as u32
    }
}

fn fill(menu: HMENU, items: &[MenuItem]) {
    unsafe {
        for item in items {
            let _ = match item {
                MenuItem::Separator => AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()),
                &MenuItem::Action(id, text) => {
                    AppendMenuW(menu, MF_STRING, id as usize, &HSTRING::from(text))
                }
                MenuItem::Label(id, text) => {
                    let flags = if *id == 0 {
                        MF_STRING | MF_GRAYED
                    } else {
                        MF_STRING
                    };
                    AppendMenuW(menu, flags, *id as usize, &HSTRING::from(text.as_str()))
                }
                MenuItem::Check(id, text, checked) => {
                    let mark = if *checked { MF_CHECKED } else { MF_UNCHECKED };
                    AppendMenuW(
                        menu,
                        MF_STRING | mark,
                        *id as usize,
                        &HSTRING::from(text.as_str()),
                    )
                }
                &MenuItem::Radio(id, text, selected) => {
                    let added = AppendMenuW(menu, MF_STRING, id as usize, &HSTRING::from(text));
                    if selected {
                        let _ = CheckMenuRadioItem(menu, id, id, id, MF_BYCOMMAND.0);
                    }
                    added
                }
                MenuItem::Submenu(text, children) => match CreatePopupMenu() {
                    Ok(sub) => {
                        fill(sub, children);
                        AppendMenuW(menu, MF_POPUP, sub.0 as usize, &HSTRING::from(*text))
                    }
                    Err(error) => Err(error),
                },
            };
        }
    }
}
