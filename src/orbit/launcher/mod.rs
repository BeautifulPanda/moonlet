mod apps;
mod calc;
mod search;
mod shell;
pub mod view;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateRectRgn, SetWindowRgn};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, SetFocus, VK_BACK, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME,
    VK_LEFT, VK_RETURN, VK_RIGHT, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GetForegroundWindow, IDC_ARROW, IsWindow, LoadCursorW,
    PostMessageW, RegisterClassW, SW_SHOW, SetForegroundWindow, SetTimer, ShowWindow, WA_INACTIVE,
    WM_ACTIVATE, WM_CHAR, WM_CLOSE, WM_COMMAND, WM_KEYDOWN, WM_MOUSEMOVE, WM_SYSCHAR, WNDCLASSW,
    WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{PCWSTR, Result, w};

use super::motion::{Channel, Target, now};
use super::{CARD_HIDE_SECS, HOVER_TIMER, Orbit, RETURN_SECS, point_from};
use crate::app::{CMD_OPEN_CONFIG, CMD_ROTATION, CMD_SCREENSHOT, CMD_SHAPE_CIRCLE, CMD_SHAPE_OVAL};
use search::{Action, Command, Item};

const CLASS: PCWSTR = w!("MoonletLauncher");
pub const GONE_TIMER: usize = 7;
const OPEN_SECS: f64 = 0.22;
const WM_LBUTTONUP: u32 = 0x0202;
const MAX_CHARS: usize = 200;

pub struct Launcher {
    pub hwnd: HWND,
    previous: HWND,
    text: Vec<char>,
    caret: usize,
    items: Vec<Item>,
    selected: usize,
    apps: Vec<apps::App>,
    search: String,
    music: bool,
}

impl Drop for Launcher {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

impl Launcher {
    fn open(rect: RECT, search: &str, music: bool) -> Result<Self> {
        unsafe {
            let previous = GetForegroundWindow();
            let instance = GetModuleHandleW(None)?;
            let class = WNDCLASSW {
                lpfnWndProc: Some(crate::app::window_proc),
                hInstance: instance.into(),
                lpszClassName: CLASS,
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                ..Default::default()
            };
            RegisterClassW(&class);
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP,
                CLASS,
                w!("Moonlet"),
                WS_POPUP,
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                None,
                None,
                Some(instance.into()),
                None,
            )?;
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            let _ = SetFocus(Some(hwnd));
            Ok(Self {
                hwnd,
                previous,
                text: Vec::new(),
                caret: 0,
                items: Vec::new(),
                selected: 0,
                apps: apps::scan(),
                search: search.to_string(),
                music,
            })
        }
    }

    fn insert(&mut self, text: &str) {
        for c in text.chars() {
            if self.text.len() >= MAX_CHARS {
                break;
            }
            self.text.insert(self.caret, c);
            self.caret += 1;
        }
    }

    fn delete_word(&mut self) {
        let mut start = self.caret;
        while start > 0 && self.text[start - 1].is_whitespace() {
            start -= 1;
        }
        while start > 0 && !self.text[start - 1].is_whitespace() {
            start -= 1;
        }
        self.text.drain(start..self.caret);
        self.caret = start;
    }
}

impl Orbit {
    pub(super) fn launcher_open(&self) -> bool {
        self.launcher.is_some()
    }

    pub fn toggle_launcher(&mut self, search: &str) -> Result<()> {
        if self.launcher.is_some() {
            return self.close_launcher(true);
        }
        if self.hidden {
            return Ok(());
        }
        let t = now();
        self.stop_timer(HOVER_TIMER);
        self.dismiss_notice(t);
        if self.hub_open() {
            self.close_hub()?;
        }
        if self.expanded {
            self.expanded = false;
            self.hide_card(t);
            self.retarget(t, RETURN_SECS, false, 0.0);
            self.set_input_region();
        }
        self.stop_timer(GONE_TIMER);
        self.remove_launcher_view();

        let mut view = view::build(&self.gfx, &self.geo, &self.overlay)?;
        let layout = view.layout;
        let geo = &self.geo;
        let left = geo.window.left + (geo.center.x - layout.width / 2.0).round() as i32;
        let top = geo.window.top + (geo.center.y + layout.top).round() as i32;
        let rect = RECT {
            left,
            top,
            right: left + layout.width as i32,
            bottom: top + layout.height(search::MAX_ITEMS) as i32,
        };
        let launcher = Launcher::open(rect, search, self.has_music())?;
        view.size = Channel::fixed(0.3).to(t, 0.0, OPEN_SECS, Target::hold(1.0), true);
        view.alpha = Channel::fixed(0.0).to(t, 0.0, OPEN_SECS * 0.7, Target::hold(1.0), false);
        self.launcher = Some(launcher);
        self.launcher_view = Some(view);
        self.launcher_changed(true)
    }

    fn has_music(&self) -> bool {
        #[cfg(feature = "music")]
        return self.music.is_some();
        #[cfg(not(feature = "music"))]
        false
    }

    pub(super) fn close_launcher(&mut self, restore_focus: bool) -> Result<()> {
        let Some(launcher) = self.launcher.take() else {
            return Ok(());
        };
        unsafe {
            if restore_focus && IsWindow(Some(launcher.previous)).as_bool() {
                let _ = SetForegroundWindow(launcher.previous);
            }
        }
        drop(launcher);
        let t = now();
        if let Some(view) = &mut self.launcher_view {
            view.hiding = true;
            view.alpha = view
                .alpha
                .to(t, 0.0, CARD_HIDE_SECS, Target::hold(0.0), false);
            view.size = view
                .size
                .to(t, 0.0, CARD_HIDE_SECS, Target::hold(0.94), false);
        }
        unsafe {
            SetTimer(
                Some(self.input_hwnd),
                GONE_TIMER,
                (CARD_HIDE_SECS * 1000.0) as u32 + 50,
                None,
            );
        }
        self.apply_all()
    }

    pub(super) fn remove_launcher_view(&mut self) {
        if let Some(view) = self.launcher_view.take() {
            view.detach(&self.overlay);
            drop(view);
            unsafe {
                let _ = self.gfx.dcomp.Commit();
            }
            self.gfx.trim();
        }
    }

    fn launcher_changed(&mut self, search: bool) -> Result<()> {
        let (Some(launcher), Some(view)) = (&mut self.launcher, &mut self.launcher_view) else {
            return Ok(());
        };
        if search {
            let text: String = launcher.text.iter().collect();
            launcher.items = search::suggestions(&text, &launcher.apps, launcher.music);
            launcher.selected = 0;
            view.hl_y = Channel::fixed(view.layout.highlight_y(0));
            view.hl_alpha = Channel::fixed(if launcher.items.is_empty() { 0.0 } else { 1.0 });
            let height = view.layout.height(launcher.items.len());
            unsafe {
                let region = CreateRectRgn(0, 0, view.layout.width as i32, height as i32);
                SetWindowRgn(launcher.hwnd, Some(region), false);
            }
        }
        view.paint(&self.gfx, &launcher.text, launcher.caret, &launcher.items)?;
        self.apply_all()
    }

    fn launcher_select(&mut self, index: usize) -> Result<()> {
        let (Some(launcher), Some(view)) = (&mut self.launcher, &mut self.launcher_view) else {
            return Ok(());
        };
        if index == launcher.selected || index >= launcher.items.len() {
            return Ok(());
        }
        launcher.selected = index;
        let y = view.layout.highlight_y(index);
        view.hl_y = view
            .hl_y
            .to(now(), 0.0, CARD_HIDE_SECS, Target::hold(y), true);
        self.apply_all()
    }

    fn launcher_run(&mut self, index: usize) -> Result<()> {
        let Some(launcher) = &mut self.launcher else {
            return Ok(());
        };
        let Some(item) = launcher.items.get(index).cloned() else {
            return Ok(());
        };
        match item.action {
            Action::Complete(text) => {
                launcher.text = text.chars().collect();
                launcher.caret = launcher.text.len();
                self.launcher_changed(true)
            }
            Action::Launch(path) => {
                shell::launch(&path);
                self.close_launcher(false)
            }
            Action::Open(url) => {
                shell::open(&url, None);
                self.close_launcher(false)
            }
            Action::Search(query) => {
                shell::web_search(&query, &launcher.search);
                self.close_launcher(false)
            }
            Action::Copy(text) => {
                crate::clipboard::copy(launcher.hwnd, &text);
                self.close_launcher(true)
            }
            Action::Command(command) => {
                self.close_launcher(true)?;
                self.run_command(command);
                Ok(())
            }
        }
    }

    fn run_command(&mut self, command: Command) {
        let host_command = match command {
            Command::Circle => CMD_SHAPE_CIRCLE,
            Command::Oval => CMD_SHAPE_OVAL,
            Command::Rotation => CMD_ROTATION,
            Command::Screenshot => CMD_SCREENSHOT,
            Command::Settings => CMD_OPEN_CONFIG,
            #[cfg(feature = "music")]
            Command::PlayPause | Command::Next | Command::Previous | Command::Volume(_) => {
                self.music_command(command);
                return;
            }
            #[cfg(not(feature = "music"))]
            _ => return,
        };
        unsafe {
            let _ = PostMessageW(
                Some(self.host),
                WM_COMMAND,
                WPARAM(host_command as usize),
                LPARAM(0),
            );
        }
    }

    #[cfg(feature = "music")]
    fn music_command(&mut self, command: Command) {
        if let Command::Volume(level) = command {
            let volume = self.volume.take().or_else(super::volume::Volume::new);
            if let Some(volume) = &volume {
                volume.set("", level as f32 / 100.0);
            }
            self.volume = volume;
            let _ = self.repaint_music_card();
            return;
        }
        let Some(music) = &self.music else {
            return;
        };
        match command {
            Command::PlayPause => music.media.toggle(),
            Command::Next => music.media.next(),
            Command::Previous => music.media.previous(),
            _ => {}
        }
    }

    pub(super) fn launcher_message(
        &mut self,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<LRESULT> {
        let result = match msg {
            WM_KEYDOWN => self.launcher_key(wparam.0 as u16),
            WM_CHAR => {
                let code = wparam.0 as u32;
                match char::from_u32(code).filter(|c| !c.is_control()) {
                    Some(c) => {
                        if let Some(launcher) = &mut self.launcher {
                            launcher.insert(c.encode_utf8(&mut [0; 4]));
                        }
                        self.launcher_changed(true)
                    }
                    None => Ok(()),
                }
            }
            WM_ACTIVATE if (wparam.0 & 0xFFFF) as u32 == WA_INACTIVE => self.close_launcher(false),
            WM_CLOSE => self.close_launcher(true),
            WM_MOUSEMOVE | WM_LBUTTONUP => {
                let point = point_from(lparam);
                let row = match (&self.launcher, &self.launcher_view) {
                    (Some(launcher), Some(view)) => {
                        view.layout.row_at(point.y, launcher.items.len())
                    }
                    _ => None,
                };
                match row {
                    Some(index) if msg == WM_LBUTTONUP => self.launcher_run(index),
                    Some(index) => self.launcher_select(index),
                    None => Ok(()),
                }
            }
            WM_SYSCHAR => Ok(()),
            _ => return None,
        };
        let _ = result;
        Some(LRESULT(0))
    }

    fn launcher_key(&mut self, key: u16) -> Result<()> {
        let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
        let Some(launcher) = &mut self.launcher else {
            return Ok(());
        };
        let count = launcher.items.len();
        let selected = launcher.selected;
        match key {
            k if k == VK_ESCAPE.0 => return self.close_launcher(true),
            k if k == VK_RETURN.0 => return self.launcher_run(selected),
            k if k == VK_DOWN.0 || k == VK_TAB.0 => {
                let next = if count == 0 {
                    0
                } else {
                    (selected + 1) % count
                };
                return self.launcher_select(next);
            }
            k if k == VK_UP.0 => {
                let next = if count == 0 {
                    0
                } else {
                    (selected + count - 1) % count
                };
                return self.launcher_select(next);
            }
            k if k == VK_LEFT.0 => launcher.caret = launcher.caret.saturating_sub(1),
            k if k == VK_RIGHT.0 => launcher.caret = (launcher.caret + 1).min(launcher.text.len()),
            k if k == VK_HOME.0 => launcher.caret = 0,
            k if k == VK_END.0 => launcher.caret = launcher.text.len(),
            k if k == VK_BACK.0 && ctrl => {
                launcher.delete_word();
                return self.launcher_changed(true);
            }
            k if k == VK_BACK.0 => {
                if launcher.caret == 0 {
                    return Ok(());
                }
                launcher.caret -= 1;
                launcher.text.remove(launcher.caret);
                return self.launcher_changed(true);
            }
            k if k == VK_DELETE.0 => {
                if launcher.caret >= launcher.text.len() {
                    return Ok(());
                }
                launcher.text.remove(launcher.caret);
                return self.launcher_changed(true);
            }
            0x56 if ctrl => {
                if let Some(text) = crate::clipboard::paste(launcher.hwnd) {
                    launcher.insert(&text);
                }
                return self.launcher_changed(true);
            }
            _ => return Ok(()),
        }
        self.launcher_changed(false)
    }

    pub(super) fn bind_launcher(&self) -> Result<()> {
        let Some(view) = &self.launcher_view else {
            return Ok(());
        };
        unsafe {
            let (scale, holder, light) = (&view.scale, &view.holder3, &view.highlight);
            self.bind(&view.size, |a| scale.SetScaleX(a), |v| scale.SetScaleX2(v))?;
            self.bind(&view.size, |a| scale.SetScaleY(a), |v| scale.SetScaleY2(v))?;
            self.bind(
                &view.alpha,
                |a| holder.SetOpacity(a),
                |v| holder.SetOpacity2(v),
            )?;
            self.bind(
                &view.hl_y,
                |a| light.SetOffsetY(a),
                |v| light.SetOffsetY2(v),
            )?;
            self.bind(
                &view.hl_alpha,
                |a| light.SetOpacity(a),
                |v| light.SetOpacity2(v),
            )?;
        }
        Ok(())
    }
}
