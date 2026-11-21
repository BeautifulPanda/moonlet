#[cfg(feature = "orbit")]
mod choose;
mod field;
mod page;
mod view;

use std::cell::RefCell;
use std::path::Path;

use windows::Win32::Foundation::{
    HANDLE, HWND, LPARAM, LRESULT, POINT, RECT, WAIT_OBJECT_0, WPARAM,
};
use windows::Win32::Graphics::Dwm::{
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmExtendFrameIntoClientArea,
    DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, ScreenToClient,
};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::INFINITE;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetSystemMetricsForDpi, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_BACK,
    VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT, VK_NEXT, VK_PRIOR,
    VK_RETURN, VK_RIGHT, VK_SHIFT, VK_SPACE, VK_TAB, VK_UP,
};
#[cfg(feature = "launcher")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey,
    UnregisterHotKey, VK_LWIN, VK_MENU, VK_RWIN,
};
use windows::Win32::UI::Shell::ShellExecuteW;
#[cfg(feature = "update")]
use windows::Win32::UI::WindowsAndMessaging::WM_COMMAND;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, FindWindowW, GetCursorPos,
    HTCAPTION, HTCLIENT, ICON_BIG, ICON_SMALL, IDC_ARROW, IDC_IBEAM, IsIconic, KillTimer,
    LoadCursorW, MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE, PeekMessageW,
    PostMessageW, PostQuitMessage, QS_ALLINPUT, RegisterClassW, SM_CXICON, SM_CXSMICON, SW_RESTORE,
    SW_SHOW, SW_SHOWNORMAL, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    SendMessageW, SetCursor, SetForegroundWindow, SetTimer, SetWindowPos, SetWindowTextW,
    ShowWindow, TranslateMessage, WA_INACTIVE, WM_ACTIVATE, WM_CHAR, WM_CLOSE, WM_DESTROY,
    WM_DPICHANGED, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL,
    WM_NCACTIVATE, WM_NCCALCSIZE, WM_NCHITTEST, WM_QUIT, WM_SETCURSOR, WM_SETICON, WM_SYSCHAR,
    WM_SYSCOMMAND, WM_SYSKEYDOWN, WM_TIMER, WNDCLASSW, WS_CAPTION, WS_EX_APPWINDOW,
    WS_EX_NOREDIRECTIONBITMAP, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU,
};
use windows::core::{HSTRING, PCWSTR, Result, w};

use crate::app::SingleInstance;
use crate::autostart;
use crate::config::{self, Config};
use crate::gfx::Gfx;
#[cfg(feature = "launcher")]
use crate::hotkey::Hotkey;
use crate::icon;
use crate::lang::{self, Language, tr};
use crate::watch::DirWatch;
use field::Field;
use page::{Change, Control, Row, Section, Setting, Ui};
use view::{Chrome, Layout, PageState, Part, View};

const CLASS: PCWSTR = w!("MoonletSettings");
const RELOAD_TIMER: usize = 1;
const RELOAD_DELAY_MS: u32 = 200;
#[cfg(feature = "launcher")]
const HOTKEY_TEST: i32 = 7;
const WM_MOUSELEAVE: u32 = 0x02A3;
const WHEEL_STEP: f32 = 56.0;
const SITE: PCWSTR = w!("https://github.com/BeautifulPanda/moonlet");

#[derive(Clone, Copy, Debug, PartialEq)]
enum Hover {
    None,
    Chrome(Chrome),
    Row(usize, Part),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Focus {
    Sidebar,
    Row(Setting),
}

struct Canvas {
    view: View,
    gfx: Gfx,
}

struct Settings {
    hwnd: HWND,
    config: Config,
    ui: Ui,
    sections: Vec<Section>,
    section: usize,
    rows: Vec<Row>,
    layout: Layout,
    hover: Hover,
    pressed: Hover,
    focus: Focus,
    focus_visible: bool,
    field: Option<Field>,
    recording: bool,
    drag: Option<(usize, f32)>,
    tracking: bool,
    canvas: Option<Canvas>,
}

thread_local! {
    static WINDOW: RefCell<Option<Settings>> = const { RefCell::new(None) };
}

pub fn run(news: bool) {
    let Some(_instance) = SingleInstance::named(w!("Local\\Moonlet.Settings")) else {
        bring_to_front();
        return;
    };
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
    lang::set(Language::Auto);
    let (config, file_error) = match Config::load() {
        Ok(config) => (config, None),
        Err(error) => (Config::default(), Some(error.to_string())),
    };
    lang::set(config.language);
    if let Err(error) = open(config, file_error, news) {
        let text = HSTRING::from(format!("Moonlet settings failed to open:\n{error}"));
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
                None,
                &text,
                w!("Moonlet"),
                windows::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
            );
        }
        return;
    }
    let watch = config::path()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .and_then(|dir| DirWatch::new(&dir));
    message_loop(watch.as_ref());
    drop(WINDOW.with(|cell| cell.borrow_mut().take()));
}

fn bring_to_front() {
    unsafe {
        if let Ok(hwnd) = FindWindowW(CLASS, PCWSTR::null()) {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

fn open(config: Config, file_error: Option<String>, news: bool) -> Result<()> {
    let (hwnd, scale) = create_window()?;
    #[allow(unused_mut)]
    let mut ui = Ui {
        file_error,
        news_open: news,
        ..Ui::default()
    };
    #[cfg(feature = "orbit")]
    {
        ui.has_battery = crate::power::has_battery();
    }
    #[cfg(feature = "timers")]
    {
        ui.reminders = crate::reminders::load();
    }
    let mut settings = Settings {
        hwnd,
        config,
        ui,
        sections: Section::all(),
        section: 0,
        rows: Vec::new(),
        layout: Layout {
            rows: Vec::new(),
            height: 0.0,
        },
        hover: Hover::None,
        pressed: Hover::None,
        focus: Focus::Sidebar,
        focus_visible: false,
        field: None,
        recording: false,
        drag: None,
        tracking: false,
        canvas: None,
    };
    settings.build_canvas(scale)?;
    if news {
        settings.reveal_news();
    }
    set_icons(hwnd, scale);
    WINDOW.with(|cell| *cell.borrow_mut() = Some(settings));
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
    Ok(())
}

fn create_window() -> Result<(HWND, f32)> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: CLASS,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            ..Default::default()
        };
        RegisterClassW(&class);
        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(monitor, &mut info);
        let (mut dpi, mut dpi_y) = (96, 96);
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut dpi_y);
        let scale = dpi as f32 / 96.0;
        let (w, h) = view::window_size(scale);
        let work = info.rcWork;
        let x = work.left + (work.right - work.left - w) / 2;
        let y = work.top + (work.bottom - work.top - h) / 2;
        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW | WS_EX_NOREDIRECTIONBITMAP,
            CLASS,
            &HSTRING::from(tr("Настройки Moonlet", "Moonlet settings")),
            WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            x,
            y,
            w,
            h,
            None,
            None,
            Some(instance.into()),
            None,
        )?;
        let margins = MARGINS {
            cxLeftWidth: 1,
            cxRightWidth: 1,
            cyTopHeight: 1,
            cyBottomHeight: 1,
        };
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
        let corners = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corners as *const _ as *const _,
            size_of_val(&corners) as u32,
        );
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        Ok((hwnd, scale))
    }
}

fn set_icons(hwnd: HWND, scale: f32) {
    let dpi = (scale * 96.0).round() as u32;
    unsafe {
        for (kind, metric) in [(ICON_BIG, SM_CXICON), (ICON_SMALL, SM_CXSMICON)] {
            if let Ok(icon) = icon::moon(GetSystemMetricsForDpi(metric, dpi)) {
                SendMessageW(
                    hwnd,
                    WM_SETICON,
                    Some(WPARAM(kind as usize)),
                    Some(LPARAM(icon.0 as isize)),
                );
            }
        }
    }
}

fn message_loop(watch: Option<&DirWatch>) {
    let mut msg = MSG::default();
    loop {
        let mut handles = [HANDLE::default(); 2];
        let mut count = 0;
        if let Some(watch) = watch {
            handles[count] = watch.handle();
            count += 1;
        }
        if let Some(Some(lost)) = with(|s| s.canvas.as_ref().and_then(|c| c.gfx.lost_event())) {
            handles[count] = lost;
            count += 1;
        }
        let handles = &handles[..count];
        let woke = unsafe {
            MsgWaitForMultipleObjectsEx(Some(handles), INFINITE, QS_ALLINPUT, MWMO_INPUTAVAILABLE)
        };
        let index = woke.0.wrapping_sub(WAIT_OBJECT_0.0) as usize;
        match (watch, handles.get(index)) {
            (Some(watch), Some(&handle)) if handle == watch.handle() => {
                watch.rearm();
                with(|s| unsafe {
                    SetTimer(Some(s.hwnd), RELOAD_TIMER, RELOAD_DELAY_MS, None);
                });
            }
            (_, Some(_)) => {
                with(|s| {
                    let scale = s.scale();
                    s.canvas = None;
                    let _ = s.build_canvas(scale);
                });
            }
            _ => {}
        }
        while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
            if msg.message == WM_QUIT {
                return;
            }
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

fn with<R>(f: impl FnOnce(&mut Settings) -> R) -> Option<R> {
    WINDOW.with(|cell| {
        let mut slot = cell.try_borrow_mut().ok()?;
        slot.as_mut().map(f)
    })
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCCALCSIZE => return LRESULT(0),
        WM_NCACTIVATE => return unsafe { DefWindowProcW(hwnd, msg, wparam, LPARAM(-1)) },
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            return LRESULT(0);
        }
        _ => {}
    }
    match with(|s| s.handle(msg, wparam, lparam)) {
        Some(Some(result)) => result,
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn point(lparam: LPARAM) -> (f32, f32) {
    let x = (lparam.0 & 0xFFFF) as i16 as f32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
    (x, y)
}

fn key_down(key: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY) -> bool {
    unsafe { GetKeyState(key.0 as i32) < 0 }
}

impl Settings {
    fn handle(&mut self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        match msg {
            WM_NCHITTEST => {
                let (x, y) = point(lparam);
                let mut at = POINT {
                    x: x as i32,
                    y: y as i32,
                };
                unsafe {
                    let _ = ScreenToClient(self.hwnd, &mut at);
                }
                let caption = self
                    .canvas
                    .as_ref()
                    .is_some_and(|c| c.view.is_caption(at.x as f32, at.y as f32));
                Some(LRESULT(if caption { HTCAPTION } else { HTCLIENT } as isize))
            }
            WM_MOUSEMOVE => {
                let (x, y) = point(lparam);
                self.track_leave();
                if self.drag.is_some() {
                    self.drag_to(x);
                } else {
                    let hover = self.hit(x, y);
                    self.set_hover(hover);
                }
                Some(LRESULT(0))
            }
            WM_MOUSELEAVE => {
                self.tracking = false;
                if self.drag.is_none() {
                    self.set_hover(Hover::None);
                }
                Some(LRESULT(0))
            }
            WM_LBUTTONDOWN => {
                let (x, y) = point(lparam);
                self.mouse_down(x, y);
                Some(LRESULT(0))
            }
            WM_LBUTTONUP => {
                let (x, y) = point(lparam);
                self.mouse_up(x, y);
                Some(LRESULT(0))
            }
            WM_MOUSEWHEEL => {
                let delta = ((wparam.0 >> 16) & 0xFFFF) as i16 as f32 / 120.0;
                let s = self.scale();
                self.scroll_by(-delta * WHEEL_STEP * s);
                Some(LRESULT(0))
            }
            WM_SETCURSOR if matches!(self.hover, Hover::Row(_, Part::Field)) => unsafe {
                if let Ok(cursor) = LoadCursorW(None, IDC_IBEAM) {
                    SetCursor(Some(cursor));
                }
                Some(LRESULT(1))
            },
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                let handled = self.key(wparam.0 as u16, msg == WM_SYSKEYDOWN);
                handled.then_some(LRESULT(0))
            }
            WM_CHAR => {
                if let Some(c) = char::from_u32(wparam.0 as u32)
                    && !c.is_control()
                    && self.field.is_some()
                {
                    self.edit(|field| field.insert(c.encode_utf8(&mut [0; 4])));
                }
                Some(LRESULT(0))
            }
            WM_SYSCHAR if self.recording => Some(LRESULT(0)),
            WM_ACTIVATE if (wparam.0 & 0xFFFF) as u32 == WA_INACTIVE => {
                if self.recording {
                    self.recording = false;
                    self.repaint_focus();
                }
                if self.commit_field() {
                    self.refresh();
                }
                None
            }
            WM_TIMER if wparam.0 == RELOAD_TIMER => {
                unsafe {
                    let _ = KillTimer(Some(self.hwnd), RELOAD_TIMER);
                }
                self.reload_file();
                Some(LRESULT(0))
            }
            WM_DPICHANGED => {
                let dpi = (wparam.0 & 0xFFFF) as f32;
                let rect = unsafe { &*(lparam.0 as *const RECT) };
                unsafe {
                    let _ = SetWindowPos(
                        self.hwnd,
                        None,
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
                self.canvas = None;
                let _ = self.build_canvas(dpi / 96.0);
                set_icons(self.hwnd, dpi / 96.0);
                Some(LRESULT(0))
            }
            WM_CLOSE => {
                self.commit_field();
                unsafe {
                    let _ = DestroyWindow(self.hwnd);
                }
                Some(LRESULT(0))
            }
            _ => None,
        }
    }

    fn scale(&self) -> f32 {
        self.canvas.as_ref().map_or(1.0, |c| c.view.scale)
    }

    fn build_canvas(&mut self, scale: f32) -> Result<()> {
        let mut gfx = Gfx::new()?;
        gfx.cleartype = true;
        let view = View::new(&gfx, self.hwnd, scale)?;
        self.canvas = Some(Canvas { view, gfx });
        self.hover = Hover::None;
        self.show_section(self.section, false);
        Ok(())
    }

    fn paint_chrome(&self, part: Option<Chrome>) {
        let Some(c) = &self.canvas else {
            return;
        };
        let hover = match self.hover {
            Hover::Chrome(chrome) => Some(chrome),
            _ => None,
        };
        let _ = c
            .view
            .paint_chrome(&c.gfx, &self.sections, self.section, hover, part);
    }

    fn show_section(&mut self, index: usize, animate: bool) {
        self.commit_field();
        self.section = index.min(self.sections.len() - 1);
        self.ui.message = None;
        self.recording = false;
        self.paint_chrome(None);
        if let Some(c) = &mut self.canvas {
            let _ = c.view.show_page(&c.gfx);
            c.view.reset_thumb();
            let _ = c.view.select(&c.gfx, self.section, animate);
        }
        self.refresh();
    }

    fn refresh(&mut self) {
        self.rows = page::rows(self.sections[self.section], &self.config, &self.ui);
        let Some(c) = &mut self.canvas else {
            return;
        };
        self.layout = c.view.layout(&c.gfx, &self.rows);
        if let Focus::Row(setting) = self.focus
            && !self.rows.iter().any(|row| row.setting == Some(setting))
        {
            self.focus = Focus::Sidebar;
            self.field = None;
        }
        let max = (self.layout.height - c.view.viewport().1).max(0.0);
        if c.view.scroll.to > max {
            let _ = c.view.scroll_to(&c.gfx, max, self.layout.height);
        }
        self.paint_rows(None);
        if let Some(c) = &mut self.canvas {
            let _ = c.view.place_knobs(&c.gfx, &self.rows, &self.layout);
            let _ = c.view.place_thumb(&c.gfx, self.layout.height, false);
        }
        self.place_caret();
        self.commit();
    }

    fn commit(&self) {
        if let Some(c) = &self.canvas {
            c.view.commit(&c.gfx);
        }
    }

    fn focus_index(&self) -> Option<usize> {
        match self.focus {
            Focus::Row(setting) => self
                .rows
                .iter()
                .position(|row| row.setting == Some(setting)),
            Focus::Sidebar => None,
        }
    }

    fn paint_rows(&mut self, part: Option<&[usize]>) {
        let hover = match self.hover {
            Hover::Row(index, part) => Some((index, part)),
            _ => None,
        };
        let state = PageState {
            hover,
            focus: self.focus_index(),
            focus_visible: self.focus_visible,
            field: self.field.as_ref(),
            recording: self.recording,
        };
        if let Some(c) = &mut self.canvas {
            let _ = c
                .view
                .paint_page(&c.gfx, &self.rows, &self.layout, &state, part);
        }
    }

    fn repaint_focus(&mut self) {
        if let Some(index) = self.focus_index() {
            self.paint_rows(Some(&[index]));
            self.commit();
        }
    }

    fn place_caret(&self) {
        let Some(c) = &self.canvas else {
            return;
        };
        let at = match (&self.field, self.focus_index()) {
            (Some(field), Some(index)) => self
                .layout
                .rows
                .get(index)
                .and_then(|b| c.view.caret_at(&c.gfx, b, field)),
            _ => None,
        };
        let _ = c.view.place_caret(&c.gfx, at);
    }

    fn hit(&self, x: f32, y: f32) -> Hover {
        let Some(c) = &self.canvas else {
            return Hover::None;
        };
        if let Some(chrome) = c.view.chrome_at(x, y, self.sections.len()) {
            return Hover::Chrome(chrome);
        }
        let Some((px, py)) = c.view.to_page(x, y) else {
            return Hover::None;
        };
        match self.layout.hit(&self.rows, px, py) {
            Some((index, part)) => Hover::Row(index, part),
            None => Hover::None,
        }
    }

    fn set_hover(&mut self, hover: Hover) {
        if hover == self.hover {
            return;
        }
        let old = std::mem::replace(&mut self.hover, hover);
        let mut rows = Vec::new();
        for h in [old, hover] {
            match h {
                Hover::Chrome(chrome) => self.paint_chrome(Some(chrome)),
                Hover::Row(index, _) if !rows.contains(&index) => rows.push(index),
                _ => {}
            }
        }
        if !rows.is_empty() {
            self.paint_rows(Some(&rows));
        }
        self.commit();
    }

    fn track_leave(&mut self) {
        if self.tracking {
            return;
        }
        let mut track = TRACKMOUSEEVENT {
            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: self.hwnd,
            dwHoverTime: 0,
        };
        self.tracking = unsafe { TrackMouseEvent(&mut track) }.is_ok();
    }

    fn mouse_down(&mut self, x: f32, y: f32) {
        let hit = self.hit(x, y);
        self.pressed = hit;
        if self.field.is_some() && !matches!(hit, Hover::Row(_, Part::Field)) {
            self.focus_visible = false;
            self.set_focus(Focus::Sidebar);
        }
        unsafe {
            SetCapture(self.hwnd);
        }
        if let Hover::Row(index, part) = hit {
            self.focus_visible = false;
            if let Some(setting) = self.rows[index].setting
                && self.rows[index].focusable()
            {
                self.set_focus(Focus::Row(setting));
            }
            if part == Part::Slider
                && let Control::Slider(slider) = self.rows[index].control
            {
                self.drag = Some((index, slider.value));
                self.drag_to(x);
            }
        }
    }

    fn mouse_up(&mut self, x: f32, y: f32) {
        unsafe {
            let _ = ReleaseCapture();
        }
        if let Some((index, start)) = self.drag.take() {
            if let (Some(setting), Control::Slider(slider)) =
                (self.rows[index].setting, &self.rows[index].control)
                && slider.value != start
            {
                let value = slider.value;
                self.change(setting, Change::Value(value));
            }
            let hover = self.hit(x, y);
            self.set_hover(hover);
            return;
        }
        let hit = self.hit(x, y);
        let pressed = std::mem::replace(&mut self.pressed, Hover::None);
        if hit != pressed {
            return;
        }
        match hit {
            Hover::Chrome(Chrome::Item(index)) => {
                if index != self.section {
                    self.show_section(index, true);
                }
            }
            Hover::Chrome(Chrome::Close) => unsafe {
                let _ = PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            },
            Hover::Chrome(Chrome::Minimize) => unsafe {
                let _ = PostMessageW(Some(self.hwnd), WM_SYSCOMMAND, WPARAM(0xF020), LPARAM(0));
            },
            Hover::Row(index, part) => self.activate(index, part),
            Hover::None => {}
        }
    }

    fn drag_to(&mut self, x: f32) {
        let Some((index, _)) = self.drag else {
            return;
        };
        let Some(c) = &self.canvas else {
            return;
        };
        let Some((px, _)) = c.view.to_page(x, c.view.height / 2.0) else {
            return;
        };
        let (tx, tw) = self.layout.rows[index].track;
        if let Control::Slider(slider) = &mut self.rows[index].control {
            let fraction = ((px - tx) / tw).clamp(0.0, 1.0);
            let value = slider.snap(slider.min + fraction * (slider.max - slider.min));
            if value == slider.value {
                return;
            }
            slider.value = value;
        }
        self.paint_rows(Some(&[index]));
        self.commit();
    }

    fn scroll_by(&mut self, dy: f32) {
        let Some(c) = &mut self.canvas else {
            return;
        };
        let max = (self.layout.height - c.view.viewport().1).max(0.0);
        let y = (c.view.scroll.to + dy).clamp(0.0, max);
        if y != c.view.scroll.to {
            let _ = c.view.scroll_to(&c.gfx, y, self.layout.height);
            c.view.commit(&c.gfx);
        }
    }

    fn scroll_into_view(&mut self) {
        let (Some(index), Some(c)) = (self.focus_index(), &self.canvas) else {
            return;
        };
        let b = &self.layout.rows[index];
        let s = c.view.scale;
        let (scroll, view_h) = (c.view.scroll.to, c.view.viewport().1);
        let margin = 12.0 * s;
        let dy = if b.y - margin < scroll {
            b.y - margin - scroll
        } else if b.y + b.h + margin > scroll + view_h {
            b.y + b.h + margin - scroll - view_h
        } else {
            return;
        };
        self.scroll_by(dy);
    }

    fn reveal_news(&mut self) {
        let Some(c) = &self.canvas else {
            return;
        };
        let Some(start) = self
            .rows
            .iter()
            .position(|row| row.setting == Some(Setting::News))
        else {
            return;
        };
        let end = self.rows.len() - 1;
        let s = c.view.scale;
        let (scroll, view_h) = (c.view.scroll.to, c.view.viewport().1);
        let margin = 12.0 * s;
        let top = self.layout.rows[start].y - margin;
        let bottom = self.layout.rows[end].y + self.layout.rows[end].h + margin;
        let target = (bottom - view_h).min(top).max(0.0);
        if target > scroll {
            self.scroll_by(target - scroll);
        }
    }

    fn set_focus(&mut self, focus: Focus) {
        if focus == self.focus {
            return;
        }
        let old = self.focus_index();
        let changed = self.commit_field();
        self.focus = focus;
        self.recording = false;
        self.field = self
            .focus_index()
            .and_then(|index| match &self.rows[index].control {
                Control::Field(text) => Some(Field::new(text)),
                _ => None,
            });
        if changed {
            self.refresh();
        } else {
            let mut rows: Vec<usize> = old.into_iter().collect();
            rows.extend(self.focus_index());
            self.paint_rows(Some(&rows));
            self.place_caret();
            self.commit();
        }
        if self.focus_visible {
            self.scroll_into_view();
        }
    }

    fn move_focus(&mut self, delta: isize) {
        let mut stops = vec![Focus::Sidebar];
        stops.extend(
            self.rows
                .iter()
                .filter(|row| row.focusable())
                .filter_map(|row| row.setting.map(Focus::Row)),
        );
        let current = stops.iter().position(|f| *f == self.focus).unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(stops.len() as isize) as usize;
        self.focus_visible = true;
        let focus = stops[next];
        if focus == self.focus {
            return;
        }
        self.set_focus(focus);
        self.repaint_focus();
    }

    fn activate(&mut self, index: usize, part: Part) {
        let row = &self.rows[index];
        let Some(setting) = row.setting else {
            return;
        };
        let change = match (&row.control, part) {
            (Control::Toggle(_), _) | (Control::Order { .. }, Part::Row) => Change::Toggle,
            (Control::Order { up: true, .. }, Part::Up)
            | (Control::Tile { up: true, .. }, Part::Up) => Change::Move(-1),
            (Control::Order { down: true, .. }, Part::Down)
            | (Control::Tile { down: true, .. }, Part::Down) => Change::Move(1),
            (Control::Tile { .. }, Part::Remove) => Change::Remove,
            (Control::Picks(_), Part::Option(i)) => Change::Pick(i),
            (Control::Choice(_, selected), Part::Segment(i)) if *selected != Some(i) => {
                Change::Pick(i)
            }
            (Control::List(_, selected), Part::Option(i)) if *selected != i => Change::Pick(i),
            (Control::Button(_), Part::Button) => {
                self.button(setting);
                return;
            }
            #[cfg(feature = "launcher")]
            (Control::Hotkey(_), Part::Hotkey) => {
                self.recording = true;
                self.ui.message = None;
                self.refresh();
                return;
            }
            _ => return,
        };
        self.change(setting, change);
    }

    fn change(&mut self, setting: Setting, change: Change) {
        #[cfg(feature = "orbit")]
        if let (Setting::Tile(index), Change::Move(delta)) = (setting, &change) {
            let to = (index as isize + delta).max(0) as usize;
            self.focus = Focus::Row(Setting::Tile(to));
        }
        let language = self.config.language;
        let autostart = self.config.autostart;
        let writes = page::apply(&mut self.config, &mut self.ui, setting, change);
        for w in &writes {
            if let Err(error) = Config::save_value(w.section, w.key, &w.value) {
                self.ui.file_error = Some(error.to_string());
            }
        }
        if self.config.autostart != autostart {
            autostart::apply(self.config.autostart);
        }
        if self.config.language != language {
            self.language_changed();
        }
        if !matches!(self.ui.message, Some((target, _, _)) if target != setting) {
            self.ui.message = None;
        }
        #[cfg(feature = "launcher")]
        if setting == Setting::Search && self.ui.custom_search {
            self.refresh();
            self.set_focus(Focus::Row(Setting::SearchUrl));
            self.scroll_into_view();
            self.edit(Field::select_all);
            return;
        }
        self.refresh();
        self.scroll_into_view();
    }

    fn language_changed(&mut self) {
        lang::set(self.config.language);
        unsafe {
            let _ = SetWindowTextW(
                self.hwnd,
                &HSTRING::from(tr("Настройки Moonlet", "Moonlet settings")),
            );
        }
        self.paint_chrome(None);
    }

    fn button(&mut self, setting: Setting) {
        match setting {
            #[cfg(feature = "update")]
            Setting::CheckNow => {
                let host = unsafe { FindWindowW(w!("MoonletHost"), PCWSTR::null()) };
                self.ui.message = Some(match host {
                    Ok(host) if !host.is_invalid() => {
                        let command = crate::app::CMD_UPDATE_NOW as usize;
                        unsafe {
                            let _ =
                                PostMessageW(Some(host), WM_COMMAND, WPARAM(command), LPARAM(0));
                        }
                        (
                            setting,
                            tr(
                                "Проверяю — ответ придёт уведомлением у значка",
                                "Checking, the answer comes as a tray notification",
                            ),
                            false,
                        )
                    }
                    _ => (
                        setting,
                        tr(
                            "Moonlet не запущен — проверять некому",
                            "Moonlet is not running, nothing can check",
                        ),
                        true,
                    ),
                });
                self.refresh();
            }
            Setting::News => {
                self.ui.news_open = !self.ui.news_open;
                self.refresh();
                if self.ui.news_open {
                    self.reveal_news();
                }
            }
            #[cfg(feature = "orbit")]
            Setting::Browse => {
                let folder = self.ui.add_kind == 3;
                let programs = self.ui.add_kind == 1;
                if let Some(path) = choose::path(self.hwnd, folder, programs) {
                    self.change(setting, Change::Text(path));
                }
            }
            #[cfg(feature = "timers")]
            Setting::TimerSoundFile => {
                if let Some(path) = choose::sound(self.hwnd) {
                    self.change(setting, Change::Text(path));
                }
            }
            #[cfg(feature = "timers")]
            Setting::TimerSoundTest => crate::sound::play(&self.config.timer_sound),
            #[cfg(feature = "timers")]
            Setting::CancelReminder(index) => {
                if let Some(r) = self.ui.reminders.get(index) {
                    crate::reminders::remove(r.id);
                }
                self.ui.reminders = crate::reminders::load();
                self.refresh();
            }
            Setting::OpenFile => config::open_in_notepad(),
            Setting::Website => unsafe {
                ShellExecuteW(
                    Some(self.hwnd),
                    w!("open"),
                    SITE,
                    PCWSTR::null(),
                    PCWSTR::null(),
                    SW_SHOWNORMAL,
                );
            },
            _ => {}
        }
    }

    fn commit_field(&mut self) -> bool {
        let Some(field) = &self.field else {
            return false;
        };
        let (Some(index), value) = (self.focus_index(), field.value()) else {
            return false;
        };
        let row = &self.rows[index];
        let (Some(setting), Control::Field(saved)) = (row.setting, &row.control) else {
            return false;
        };
        if value.trim() == saved.trim() {
            return false;
        }
        let writes = page::apply(&mut self.config, &mut self.ui, setting, Change::Text(value));
        for w in &writes {
            let _ = Config::save_value(w.section, w.key, &w.value);
        }
        self.rows = page::rows(self.sections[self.section], &self.config, &self.ui);
        let saved = self.rows.iter().find_map(|row| match &row.control {
            Control::Field(saved) if row.setting == Some(setting) => Some(saved.clone()),
            _ => None,
        });
        if let Some(field) = &mut self.field {
            match saved {
                Some(saved) if saved.trim() != field.value().trim() => *field = Field::new(&saved),
                _ => field.all = false,
            }
        }
        true
    }

    fn edit(&mut self, action: impl FnOnce(&mut Field)) {
        if let Some(field) = &mut self.field {
            action(field);
        }
        #[cfg(feature = "orbit")]
        if let (Some(field), Some(index)) = (&self.field, self.focus_index())
            && self.rows[index].setting == Some(Setting::AppQuery)
            && field.value() != self.ui.app_query
        {
            self.ui.app_query = field.value();
            self.refresh();
            return;
        }
        self.repaint_focus();
        self.place_caret();
        self.commit();
    }

    fn key(&mut self, vk: u16, system: bool) -> bool {
        #[cfg(feature = "launcher")]
        if self.recording {
            return self.record(vk);
        }
        if system {
            return false;
        }
        let ctrl = key_down(VK_CONTROL);
        let shift = key_down(VK_SHIFT);
        let back = if shift { -1 } else { 1 };
        let section_step = |s: &mut Self, delta: isize| {
            let n = s.sections.len() as isize;
            let next = (s.section as isize + delta).rem_euclid(n) as usize;
            s.show_section(next, true);
        };
        match vk {
            v if v == VK_TAB.0 && ctrl => section_step(self, back),
            v if v == VK_TAB.0 => self.move_focus(back),
            v if (v == VK_NEXT.0 || v == VK_PRIOR.0) && ctrl => {
                section_step(self, if v == VK_NEXT.0 { 1 } else { -1 })
            }
            v if v == VK_ESCAPE.0 => {
                let index = self.focus_index();
                if let (Some(field), Some(index)) = (&mut self.field, index)
                    && let Control::Field(saved) = &self.rows[index].control
                    && field.value() != *saved
                {
                    *field = Field::new(saved);
                    self.edit(|_| {});
                    return true;
                }
                unsafe {
                    let _ = PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
            }
            _ if self.field.is_some() => return self.field_key(vk, ctrl),
            _ => return self.focus_key(vk, ctrl),
        }
        true
    }

    fn field_key(&mut self, vk: u16, ctrl: bool) -> bool {
        match vk {
            v if v == VK_LEFT.0 => self.edit(|f| f.left(ctrl)),
            v if v == VK_RIGHT.0 => self.edit(|f| f.right(ctrl)),
            v if v == VK_HOME.0 => self.edit(Field::home),
            v if v == VK_END.0 => self.edit(Field::end),
            v if v == VK_BACK.0 => self.edit(|f| f.backspace(ctrl)),
            v if v == VK_DELETE.0 => self.edit(|f| f.delete(ctrl)),
            v if v == VK_RETURN.0 => {
                if self.commit_field() {
                    self.refresh();
                }
            }
            v if v == VK_UP.0 => self.move_focus(-1),
            v if v == VK_DOWN.0 => self.move_focus(1),
            0x41 if ctrl => self.edit(Field::select_all),
            0x56 if ctrl => {
                if let Some(text) = crate::clipboard::paste(self.hwnd) {
                    self.edit(|f| f.insert(&text));
                }
            }
            0x43 | 0x58 if ctrl => {
                let Some(field) = &self.field else {
                    return true;
                };
                crate::clipboard::copy(self.hwnd, &field.value());
                if vk == 0x58 {
                    self.edit(|f| {
                        f.select_all();
                        f.backspace(false);
                    });
                }
            }
            _ => return false,
        }
        true
    }

    fn focus_key(&mut self, vk: u16, ctrl: bool) -> bool {
        let Some(index) = self.focus_index() else {
            let n = self.sections.len();
            match vk {
                v if v == VK_UP.0 && self.section > 0 => self.show_section(self.section - 1, true),
                v if v == VK_DOWN.0 && self.section + 1 < n => {
                    self.show_section(self.section + 1, true)
                }
                v if v == VK_HOME.0 => self.show_section(0, true),
                v if v == VK_END.0 => self.show_section(n - 1, true),
                v if v == VK_RIGHT.0 || v == VK_RETURN.0 => self.move_focus(1),
                v if v == VK_NEXT.0 || v == VK_PRIOR.0 => self.page_key(vk),
                _ => return false,
            }
            return true;
        };
        let Some(setting) = self.rows[index].setting else {
            return false;
        };
        let control = self.rows[index].control.clone();
        let step = |delta: isize, len: usize, at: usize| -> Option<usize> {
            let next = at as isize + delta;
            (next >= 0 && (next as usize) < len).then_some(next as usize)
        };
        match (vk, &control) {
            (v, _) if v == VK_SPACE.0 || v == VK_RETURN.0 => {
                let part = match control {
                    Control::Toggle(_) | Control::Order { .. } => Part::Row,
                    Control::Button(_) => Part::Button,
                    Control::Hotkey(_) => Part::Hotkey,
                    Control::Choice(ref options, selected) => {
                        Part::Segment(selected.map_or(0, |s| (s + 1) % options.len()))
                    }
                    Control::Picks(_) => Part::Option(0),
                    _ => return true,
                };
                self.activate(index, part);
            }
            (v, Control::Tile { .. }) if v == VK_DELETE.0 => self.activate(index, Part::Remove),
            (v, Control::Tile { up, down }) if ctrl && (v == VK_UP.0 || v == VK_DOWN.0) => {
                if v == VK_UP.0 && *up {
                    self.change(setting, Change::Move(-1));
                } else if v == VK_DOWN.0 && *down {
                    self.change(setting, Change::Move(1));
                }
            }
            (v, Control::Choice(options, selected)) if v == VK_LEFT.0 || v == VK_RIGHT.0 => {
                let delta = if v == VK_LEFT.0 { -1 } else { 1 };
                if let Some(next) = step(delta, options.len(), selected.unwrap_or(0)) {
                    self.change(setting, Change::Pick(next));
                }
            }
            (v, Control::List(options, selected)) if v == VK_UP.0 || v == VK_DOWN.0 => {
                let delta = if v == VK_UP.0 { -1 } else { 1 };
                match step(delta, options.len(), *selected) {
                    Some(next) => self.change(setting, Change::Pick(next)),
                    None => self.move_focus(delta),
                }
            }
            (v, Control::Slider(slider))
                if v == VK_LEFT.0 || v == VK_RIGHT.0 || v == VK_HOME.0 || v == VK_END.0 =>
            {
                let value = match v {
                    _ if v == VK_LEFT.0 => slider.snap(slider.value - slider.step),
                    _ if v == VK_RIGHT.0 => slider.snap(slider.value + slider.step),
                    _ if v == VK_HOME.0 => slider.min,
                    _ => slider.max,
                };
                if value != slider.value {
                    self.change(setting, Change::Value(value));
                }
            }
            (v, Control::Order { up, down, .. }) if ctrl && (v == VK_UP.0 || v == VK_DOWN.0) => {
                if v == VK_UP.0 && *up {
                    self.change(setting, Change::Move(-1));
                } else if v == VK_DOWN.0 && *down {
                    self.change(setting, Change::Move(1));
                }
            }
            (v, _) if v == VK_UP.0 => self.move_focus(-1),
            (v, _) if v == VK_DOWN.0 => self.move_focus(1),
            (v, _) if v == VK_NEXT.0 || v == VK_PRIOR.0 => self.page_key(vk),
            _ => return false,
        }
        true
    }

    fn page_key(&mut self, vk: u16) {
        let Some(c) = &self.canvas else {
            return;
        };
        let page = c.view.viewport().1 * 0.8;
        self.scroll_by(if vk == VK_NEXT.0 { page } else { -page });
    }

    #[cfg(feature = "launcher")]
    fn record(&mut self, vk: u16) -> bool {
        let modifier_keys = [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN];
        if modifier_keys.iter().any(|k| k.0 == vk) || (0xA0..=0xA5).contains(&vk) {
            return true;
        }
        if vk == VK_ESCAPE.0 {
            self.recording = false;
            self.refresh();
            return true;
        }
        let mut modifiers = 0;
        for (key, flag) in [
            (VK_CONTROL, MOD_CONTROL.0),
            (VK_MENU, MOD_ALT.0),
            (VK_SHIFT, MOD_SHIFT.0),
            (VK_LWIN, MOD_WIN.0),
            (VK_RWIN, MOD_WIN.0),
        ] {
            if key_down(key) {
                modifiers |= flag;
            }
        }
        let key = Hotkey {
            modifiers,
            key: vk as u32,
        };
        let warn = |s: &mut Self, text: &'static str| {
            s.ui.message = Some((Setting::Hotkey, text, true));
            s.refresh();
        };
        if modifiers == 0 {
            warn(
                self,
                tr(
                    "Нужен Ctrl, Alt, Shift или Win",
                    "Add Ctrl, Alt, Shift or Win",
                ),
            );
            return true;
        }
        if Hotkey::parse(&key.name()) != Some(key) {
            warn(
                self,
                tr("Эта клавиша не подходит", "This key cannot be used"),
            );
            return true;
        }
        self.recording = false;
        if key == self.config.launcher.hotkey {
            self.ui.message = None;
            self.refresh();
            return true;
        }
        let free = unsafe {
            let flags = HOT_KEY_MODIFIERS(modifiers) | MOD_NOREPEAT;
            let ok = RegisterHotKey(Some(self.hwnd), HOTKEY_TEST, flags, key.key).is_ok();
            if ok {
                let _ = UnregisterHotKey(Some(self.hwnd), HOTKEY_TEST);
            }
            ok
        };
        if free {
            self.change(Setting::Hotkey, Change::Hotkey(key));
        } else {
            warn(
                self,
                tr(
                    "Это сочетание занято другой программой",
                    "Another program already uses it",
                ),
            );
        }
        true
    }

    fn reload_file(&mut self) {
        #[cfg(feature = "timers")]
        {
            let list = crate::reminders::load();
            if list != self.ui.reminders {
                self.ui.reminders = list;
                self.refresh();
            }
        }
        match Config::load() {
            Ok(config) => {
                let had_error = self.ui.file_error.take().is_some();
                if config == self.config && !had_error {
                    return;
                }
                let language = self.config.language;
                self.config = config;
                if self.config.language != language {
                    self.language_changed();
                }
            }
            Err(error) => self.ui.file_error = Some(error.to_string()),
        }
        self.refresh();
    }
}
