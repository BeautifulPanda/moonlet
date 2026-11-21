pub mod attach;
pub mod store;

use std::cell::RefCell;

use windows::Win32::Foundation::{COLORREF, HMODULE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWM_WINDOW_CORNER_PREFERENCE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUNDSMALL,
    DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontIndirectW, CreateFontW, CreateSolidBrush, DT_CENTER, DT_END_ELLIPSIS,
    DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DeleteObject, DrawTextW, EndPaint, FillRect,
    FrameRect, GetMonitorInfoW, HFONT, HGDIOBJ, InvalidateRect, MONITOR_DEFAULTTONEAREST,
    MONITORINFO, MonitorFromPoint, MonitorFromRect, PAINTSTRUCT, SelectObject, SetBkMode,
    SetTextColor, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, LoadLibraryW};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::Controls::NMHDR;
use windows::Win32::UI::Controls::RichEdit::{
    AURL_ENABLEURL, CFM_COLOR, CFM_FACE, CFM_SIZE, CHARFORMAT2W, CHARRANGE, EM_AUTOURLDETECT,
    EM_EXGETSEL, EM_GETTEXTRANGE, EM_GETZOOM, EM_SETBKGNDCOLOR, EM_SETCHARFORMAT, EM_SETEVENTMASK,
    EM_SETTEXTMODE, EM_SETZOOM, EN_LINK, ENLINK, ENM_CHANGE, ENM_LINK, MSFTEDIT_CLASS, SCF_ALL,
    TEXTRANGEW, TM_MULTILEVELUNDO, TM_PLAINTEXT,
};
use windows::Win32::UI::HiDpi::{
    GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI, SystemParametersInfoForDpi,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SetFocus, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    CHILDID_SELF, CS_DROPSHADOW, CreateWindowExW, DefWindowProcW, DestroyWindow, EN_CHANGE,
    ES_AUTOVSCROLL, ES_MULTILINE, ES_WANTRETURN, EVENT_OBJECT_CLOAKED, EVENT_OBJECT_DESTROY,
    EVENT_OBJECT_HIDE, EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_NAMECHANGE,
    EVENT_OBJECT_UNCLOAKED, EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND,
    EVENT_SYSTEM_MINIMIZESTART, GA_ROOT, GW_CHILD, GW_HWNDPREV, GWL_EXSTYLE, GWLP_USERDATA,
    GetAncestor, GetClientRect, GetCursorPos, GetWindow, GetWindowLongPtrW, GetWindowLongW,
    GetWindowRect, GetWindowTextLengthW, GetWindowTextW, HMENU, HTBOTTOM, HTBOTTOMLEFT,
    HTBOTTOMRIGHT, HTCAPTION, HTCLIENT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, HWND_BOTTOM,
    HWND_NOTOPMOST, HWND_TOP, HWND_TOPMOST, IDC_ARROW, IsWindow, KillTimer, LoadCursorW,
    MINMAXINFO, MoveWindow, NONCLIENTMETRICSW, OBJID_WINDOW, RegisterClassW,
    SPI_GETNONCLIENTMETRICS, SW_HIDE, SW_SHOWNORMAL, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SWP_SHOWWINDOW, SetForegroundWindow, SetTimer, SetWindowLongPtrW, SetWindowPos,
    SetWindowTextW, ShowWindow, WA_INACTIVE, WINDOW_STYLE, WINDOWPOS, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS, WM_ACTIVATE, WM_APP, WM_COMMAND, WM_DPICHANGED, WM_ENDSESSION,
    WM_ERASEBKGND, WM_EXITSIZEMOVE, WM_GETMINMAXINFO, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCHITTEST,
    WM_NCLBUTTONDBLCLK, WM_NCRBUTTONUP, WM_NOTIFY, WM_PAINT, WM_QUERYENDSESSION, WM_RBUTTONUP,
    WM_SIZE, WM_TIMER, WM_WINDOWPOSCHANGING, WNDCLASSW, WS_CHILD, WS_CLIPCHILDREN,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
};
use windows::core::{HSTRING, PCWSTR, PWSTR, w};

use crate::lang::tr;
use crate::tray::{self, MenuItem};
use store::{Attach, Mode, Note};

pub const WM_NOTES: u32 = WM_APP + 8;
pub const NOTE_SHOW: usize = 1;
pub const NOTE_DELETE: usize = 2;

const CLASS: PCWSTR = w!("MoonletNote");

const WM_MOUSELEAVE: u32 = 0x02A3;
const EM_SETSEL: u32 = 0x00B1;

const SAVE_TIMER: usize = 1;
const SAVE_DELAY_MS: u32 = 1000;
const ARM_TIMER: usize = 2;
const ARM_MS: u32 = 3000;

const HEADER: f32 = 26.0;
const PAD_X: f32 = 10.0;
const PAD_BOTTOM: f32 = 8.0;
const EDGE: f32 = 6.0;
const BUTTON: f32 = 24.0;
const DEFAULT_W: f32 = 240.0;
const DEFAULT_H: f32 = 180.0;
const MIN_W: f32 = 150.0;
const MIN_H: f32 = 90.0;
const CASCADE: f32 = 24.0;

const BACK: u32 = 0x1C1F25;
const BORDER: u32 = 0x31353E;
const TEXT: u32 = 0xE6E8EE;
const DIM: u32 = 0x8C92A0;
const HOVER: u32 = 0x2A2E36;
const WARN: u32 = 0xFFA28A;

const MENU_TOP: u32 = 1;
const MENU_DESKTOP: u32 = 2;
const MENU_NORMAL: u32 = 3;
const MENU_ATTACH: u32 = 4;
const MENU_NEW: u32 = 5;
const MENU_DELETE: u32 = 6;

const GLYPH_TOP: char = '\u{E718}';
const GLYPH_DESKTOP: char = '\u{E7F4}';
const GLYPH_NORMAL: char = '\u{E70B}';
const GLYPH_ATTACH: char = '\u{E71B}';
const GLYPH_MORE: char = '\u{E712}';
const GLYPH_CLOSE: char = '\u{E711}';

const DESKTOP_FLAG: isize = 1;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Button {
    Menu,
    Close,
}

struct Win {
    note: Note,
    hwnd: HWND,
    edit: HWND,
    dpi: u32,
    glyphs: HFONT,
    small: HFONT,
    hover: Option<Button>,
    tracking: bool,
    armed: bool,
    dirty: bool,
    target: Option<HWND>,
    pid: Option<u32>,
    label: String,
    visible: bool,
}

impl Drop for Win {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
            let _ = DeleteObject(HGDIOBJ(self.glyphs.0));
            let _ = DeleteObject(HGDIOBJ(self.small.0));
        }
    }
}

struct Notes {
    wins: Vec<Win>,
    mode: Mode,
    fullscreen: bool,
    global: Vec<HWINEVENTHOOK>,
    process: Vec<(u32, Vec<HWINEVENTHOOK>)>,
    candidate: Option<HWND>,
}

thread_local! {
    static NOTES: RefCell<Option<Notes>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Notes) -> R) -> Option<R> {
    NOTES.with(|cell| {
        let mut slot = cell.try_borrow_mut().ok()?;
        slot.as_mut().map(f)
    })
}

fn colorref(rgb: u32) -> COLORREF {
    COLORREF(((rgb >> 16) & 0xFF) | (rgb & 0xFF00) | ((rgb & 0xFF) << 16))
}

fn px(value: f32, dpi: u32) -> i32 {
    (value * dpi as f32 / 96.0).round() as i32
}

pub fn start(mode: Mode) {
    if with(|notes| notes.mode = mode).is_some() {
        return;
    }
    NOTES.with(|cell| {
        *cell.borrow_mut() = Some(Notes {
            wins: Vec::new(),
            mode,
            fullscreen: false,
            global: Vec::new(),
            process: Vec::new(),
            candidate: None,
        })
    });
    for note in store::load() {
        let text = store::read_text(note.id);
        if let Some(win) = open_window(note, &text) {
            with(|notes| {
                notes.wins.push(win);
                let index = notes.wins.len() - 1;
                notes.show(index);
            });
        }
    }
    with(Notes::sync_hooks);
}

pub fn stop() {
    let taken = NOTES.with(|cell| cell.try_borrow_mut().ok().and_then(|mut s| s.take()));
    if let Some(mut notes) = taken {
        notes.flush();
        notes.unhook_all();
        drop(notes);
    }
}

#[cfg_attr(not(feature = "orbit"), allow(dead_code))]
pub fn set_fullscreen(fullscreen: bool) {
    with(|notes| {
        if notes.fullscreen != fullscreen {
            notes.fullscreen = fullscreen;
            for index in 0..notes.wins.len() {
                notes.show(index);
            }
        }
    });
}

pub fn create(text: &str) {
    let Some(note) = with(|notes| notes.new_note()) else {
        return;
    };
    let Some(win) = open_window(note, text) else {
        return;
    };
    let (hwnd, edit) = (win.hwnd, win.edit);
    with(|notes| {
        notes.wins.push(win);
        let index = notes.wins.len() - 1;
        notes.show(index);
        notes.save_layout();
        if !text.is_empty() {
            store::write_text(notes.wins[index].note.id, text);
        }
    });
    focus(hwnd, edit);
}

pub fn show_all() {
    let first = with(|notes| {
        let mut first = None;
        for win in notes.wins.iter().filter(|w| w.visible) {
            lift(win.hwnd);
            first.get_or_insert((win.hwnd, win.edit));
        }
        first
    });
    if let Some(Some((hwnd, edit))) = first {
        focus(hwnd, edit);
    }
}

#[cfg_attr(not(feature = "orbit"), allow(dead_code))]
pub fn enabled() -> bool {
    NOTES.with(|cell| cell.try_borrow().map_or(true, |slot| slot.is_some()))
}

pub fn count() -> usize {
    with(|notes| notes.wins.len()).unwrap_or(0)
}

pub fn message(wparam: WPARAM, lparam: LPARAM) {
    let id = lparam.0 as u64;
    match wparam.0 {
        NOTE_SHOW => {
            let found = with(|notes| {
                let index = notes.wins.iter().position(|w| w.note.id == id)?;
                if !notes.wins[index].visible {
                    let win = &mut notes.wins[index];
                    win.note.attach = None;
                    win.note.mode = Mode::Top;
                    notes.show(index);
                    notes.sync_hooks();
                    notes.save_layout();
                }
                let win = &notes.wins[index];
                lift(win.hwnd);
                Some((win.hwnd, win.edit))
            });
            if let Some(Some((hwnd, edit))) = found {
                focus(hwnd, edit);
            }
        }
        NOTE_DELETE => {
            with(|notes| notes.delete(id));
        }
        _ => {}
    }
}

fn lift(hwnd: HWND) {
    unsafe {
        let flag = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, flag);
    }
}

fn focus(hwnd: HWND, edit: HWND) {
    unsafe {
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(edit));
        let _ = windows::Win32::UI::WindowsAndMessaging::SendMessageW(
            edit,
            EM_SETSEL,
            Some(WPARAM(usize::MAX)),
            Some(LPARAM(-1)),
        );
    }
}

fn send(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize {
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::SendMessageW(
            hwnd,
            msg,
            Some(WPARAM(wparam)),
            Some(LPARAM(lparam)),
        )
        .0
    }
}

fn prepare() -> bool {
    thread_local! {
        static READY: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    }
    if let Some(ready) = READY.get() {
        return ready;
    }
    let ready = unsafe {
        let library: windows::core::Result<HMODULE> = LoadLibraryW(w!("Msftedit.dll"));
        let instance = GetModuleHandleW(None).unwrap_or_default();
        let class = WNDCLASSW {
            style: CS_DROPSHADOW,
            lpfnWndProc: Some(note_proc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: CLASS,
            ..Default::default()
        };
        library.is_ok() && RegisterClassW(&class) != 0
    };
    READY.set(Some(ready));
    ready
}

fn system_font(dpi: u32) -> windows::Win32::Graphics::Gdi::LOGFONTW {
    let mut metrics = NONCLIENTMETRICSW {
        cbSize: size_of::<NONCLIENTMETRICSW>() as u32,
        ..Default::default()
    };
    unsafe {
        let _ = SystemParametersInfoForDpi(
            SPI_GETNONCLIENTMETRICS.0,
            metrics.cbSize,
            Some((&mut metrics as *mut NONCLIENTMETRICSW).cast()),
            0,
            dpi,
        );
    }
    metrics.lfMessageFont
}

fn header_fonts(dpi: u32) -> (HFONT, HFONT) {
    unsafe {
        let glyphs = CreateFontW(
            -px(12.0, dpi),
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            0,
            w!("Segoe MDL2 Assets"),
        );
        let mut small = system_font(dpi);
        small.lfHeight = -px(12.0, dpi);
        (glyphs, CreateFontIndirectW(&small))
    }
}

fn apply_text_format(edit: HWND, dpi: u32) {
    let font = system_font(dpi);
    let points = (-font.lfHeight).max(1) as f32 * 72.0 / dpi as f32;
    let mut format = CHARFORMAT2W::default();
    format.Base.cbSize = size_of::<CHARFORMAT2W>() as u32;
    format.Base.dwMask = CFM_COLOR | CFM_FACE | CFM_SIZE;
    format.Base.crTextColor = colorref(TEXT);
    format.Base.yHeight = ((points + 2.0) * 20.0).round() as i32;
    let face_len = font.lfFaceName.iter().position(|&c| c == 0).unwrap_or(32);
    format.Base.szFaceName[..face_len].copy_from_slice(&font.lfFaceName[..face_len]);
    send(
        edit,
        EM_SETCHARFORMAT,
        SCF_ALL as usize,
        &format as *const CHARFORMAT2W as isize,
    );
    send(
        edit,
        EM_SETCHARFORMAT,
        0,
        &format as *const CHARFORMAT2W as isize,
    );
}

fn open_window(note: Note, text: &str) -> Option<Win> {
    if !prepare() {
        return None;
    }
    unsafe {
        let instance = GetModuleHandleW(None).ok()?;
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            CLASS,
            w!("Moonlet"),
            WS_POPUP | WS_CLIPCHILDREN,
            note.x,
            note.y,
            note.w,
            note.h,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .ok()?;
        let corner = DWMWCP_ROUNDSMALL;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&corner as *const DWM_WINDOW_CORNER_PREFERENCE).cast(),
            size_of_val(&corner) as u32,
        );
        let style = WINDOW_STYLE(
            (WS_CHILD | WS_VISIBLE).0 | (ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN) as u32,
        );
        let edit = CreateWindowExW(
            Default::default(),
            MSFTEDIT_CLASS,
            PCWSTR::null(),
            style,
            0,
            0,
            0,
            0,
            Some(hwnd),
            Some(HMENU(1 as _)),
            Some(instance.into()),
            None,
        )
        .ok();
        let Some(edit) = edit else {
            let _ = DestroyWindow(hwnd);
            return None;
        };
        let dpi = GetDpiForWindow(hwnd).max(96);
        send(
            edit,
            EM_SETTEXTMODE,
            (TM_PLAINTEXT.0 | TM_MULTILEVELUNDO.0) as usize,
            0,
        );
        send(edit, EM_SETBKGNDCOLOR, 0, colorref(BACK).0 as isize);
        apply_text_format(edit, dpi);
        send(edit, EM_AUTOURLDETECT, AURL_ENABLEURL as usize, 0);
        let _ = SetWindowTextW(edit, &HSTRING::from(text));
        send(edit, EM_SETZOOM, note.zoom as usize, 100);
        send(edit, EM_SETEVENTMASK, 0, (ENM_CHANGE | ENM_LINK) as isize);
        layout_edit(hwnd);
        let (glyphs, small) = header_fonts(dpi);
        Some(Win {
            note,
            hwnd,
            edit,
            dpi,
            glyphs,
            small,
            hover: None,
            tracking: false,
            armed: false,
            dirty: false,
            target: None,
            pid: None,
            label: String::new(),
            visible: false,
        })
    }
}

fn layout_edit(hwnd: HWND) {
    unsafe {
        let Ok(edit) = GetWindow(hwnd, GW_CHILD) else {
            return;
        };
        let dpi = GetDpiForWindow(hwnd).max(96);
        let mut rect = RECT::default();
        let _ = GetClientRect(hwnd, &mut rect);
        let (x, y) = (px(PAD_X, dpi), px(HEADER, dpi));
        let w = (rect.right - 2 * x).max(1);
        let h = (rect.bottom - y - px(PAD_BOTTOM, dpi)).max(1);
        let _ = MoveWindow(edit, x, y, w, h, true);
    }
}

fn window_rect(hwnd: HWND) -> RECT {
    let mut rect = RECT::default();
    unsafe {
        let _ = GetWindowRect(hwnd, &mut rect);
    }
    rect
}

fn edit_text(edit: HWND) -> String {
    unsafe {
        let length = GetWindowTextLengthW(edit);
        let mut buffer = vec![0u16; length.max(0) as usize + 1];
        let copied = GetWindowTextW(edit, &mut buffer);
        String::from_utf16_lossy(&buffer[..copied.max(0) as usize])
    }
}

fn zoom_of(edit: HWND) -> u32 {
    let (mut num, mut den) = (0i32, 0i32);
    send(
        edit,
        EM_GETZOOM,
        &mut num as *mut i32 as usize,
        &mut den as *mut i32 as isize,
    );
    if num <= 0 || den <= 0 {
        100
    } else {
        ((num as f32 * 100.0 / den as f32).round() as u32).clamp(25, 400)
    }
}

enum Reply {
    Done(LRESULT),
    Default,
    Open(String),
    Menu(Vec<MenuItem>, POINT),
}

unsafe extern "system" fn note_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_SIZE => {
            layout_edit(hwnd);
            return LRESULT(0);
        }
        WM_NCLBUTTONDBLCLK if wparam.0 == HTCAPTION as usize => return LRESULT(0),
        WM_WINDOWPOSCHANGING => {
            if unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } == DESKTOP_FLAG {
                let pos = unsafe { &mut *(lparam.0 as *mut WINDOWPOS) };
                if pos.flags.0 & SWP_NOZORDER.0 == 0 {
                    pos.hwndInsertAfter = HWND_BOTTOM;
                }
            }
            return LRESULT(0);
        }
        _ => {}
    }
    match with(|notes| notes.handle(hwnd, msg, wparam, lparam)) {
        Some(Reply::Done(result)) => result,
        Some(Reply::Open(url)) => {
            open_url(&url);
            LRESULT(0)
        }
        Some(Reply::Menu(items, at)) => {
            let command = tray::show_menu(hwnd, &items, at);
            if command == MENU_NEW {
                create("");
            } else if command != 0 {
                with(|notes| notes.menu_chosen(hwnd, command));
            }
            LRESULT(0)
        }
        Some(Reply::Default) | None => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn open_url(url: &str) {
    let url = if url.contains(':') {
        url.to_string()
    } else {
        format!("https://{url}")
    };
    unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &HSTRING::from(url),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

unsafe extern "system" fn win_event(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    object: i32,
    child: i32,
    _thread: u32,
    _time: u32,
) {
    if object != OBJID_WINDOW.0 || child != CHILDID_SELF as i32 {
        return;
    }
    with(|notes| notes.window_event(event, hwnd));
}

impl Notes {
    fn index(&self, hwnd: HWND) -> Option<usize> {
        self.wins.iter().position(|w| w.hwnd == hwnd)
    }

    fn new_note(&self) -> Note {
        let mut cursor = POINT::default();
        unsafe {
            let _ = GetCursorPos(&mut cursor);
        }
        let monitor = unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) };
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let (mut dpi, mut dpi_y) = (96, 96);
        unsafe {
            let _ = GetMonitorInfoW(monitor, &mut info);
            let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut dpi_y);
        }
        let work = info.rcWork;
        let (w, h) = (px(DEFAULT_W, dpi), px(DEFAULT_H, dpi));
        let step = px(CASCADE, dpi) * (self.wins.len() % 6) as i32;
        let ids: Vec<Note> = self.wins.iter().map(|w| w.note.clone()).collect();
        Note {
            id: store::next_id(&ids).max(store::next_id(&store::load())),
            x: work.left + (work.right - work.left - w) / 2 + step,
            y: work.top + (work.bottom - work.top) / 3 - h / 2 + step,
            w,
            h,
            zoom: 100,
            mode: self.mode,
            attach: None,
        }
    }

    fn show(&mut self, index: usize) {
        let fullscreen = self.fullscreen;
        let win = &mut self.wins[index];
        let hwnd = win.hwnd;
        unsafe {
            SetWindowLongPtrW(
                hwnd,
                GWLP_USERDATA,
                if win.note.attach.is_none() && win.note.mode == Mode::Desktop {
                    DESKTOP_FLAG
                } else {
                    0
                },
            );
        }
        if win.note.attach.is_some() {
            unsafe {
                let topmost = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST.0 != 0;
                if topmost {
                    let _ = SetWindowPos(
                        hwnd,
                        Some(HWND_NOTOPMOST),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                }
            }
            self.follow(index);
            return;
        }
        win.target = None;
        win.pid = None;
        if win.note.mode == Mode::Top && fullscreen {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            win.visible = false;
            return;
        }
        keep_on_screen(win);
        let after = match win.note.mode {
            Mode::Top => HWND_TOPMOST,
            Mode::Desktop => HWND_BOTTOM,
            Mode::Normal => HWND_NOTOPMOST,
        };
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                Some(after),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
        win.visible = true;
        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
    }

    fn follow(&mut self, index: usize) {
        let fullscreen = self.fullscreen;
        let win = &mut self.wins[index];
        let Some(attach) = win.note.attach.clone() else {
            return;
        };
        let still = win.target.filter(|&h| {
            unsafe { IsWindow(Some(h)) }.as_bool()
                && attach::usable(h)
                && (attach.tab.is_empty() || attach::tab_title(&attach::title_of(h)) == attach.tab)
        });
        let target = match still {
            Some(h) => Some(h),
            None => {
                let (found, pid) = attach::find(&attach);
                win.pid = pid;
                found
            }
        };
        if target != win.target {
            win.label = match target {
                Some(h) => attach::label(h, &attach.exe),
                None if !attach.tab.is_empty() => attach.tab.clone(),
                None => attach.exe.trim_end_matches(".exe").to_string(),
            };
            unsafe {
                let _ = InvalidateRect(Some(win.hwnd), None, false);
            }
        }
        win.target = target;
        let place = target.filter(|_| !fullscreen).and_then(attach::bounds);
        let (Some(target), Some(bounds)) = (target, place) else {
            if win.visible {
                unsafe {
                    let _ = ShowWindow(win.hwnd, SW_HIDE);
                }
                win.visible = false;
            }
            return;
        };
        win.pid = Some(attach::pid_of(target));
        let (cx, cy) = attach::corner_point(attach.corner, bounds);
        let (x, y) = (cx + attach.dx, cy + attach.dy);
        win.note.x = x;
        win.note.y = y;
        let above = unsafe { GetWindow(target, GW_HWNDPREV) }.ok();
        let after = match above {
            Some(h) if h == win.hwnd => None,
            Some(h) if unsafe { GetWindowLongW(h, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST.0 == 0 => {
                Some(h)
            }
            _ => Some(HWND_TOP),
        };
        let mut flags = SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW;
        if after.is_none() {
            flags |= SWP_NOZORDER;
        }
        unsafe {
            let _ = SetWindowPos(win.hwnd, after, x, y, 0, 0, flags);
        }
        win.visible = true;
    }

    fn sync_hooks(&mut self) {
        let attached = self.wins.iter().any(|w| w.note.attach.is_some());
        let hook = |min: u32, max: u32, pid: u32| unsafe {
            SetWinEventHook(
                min,
                max,
                None,
                Some(win_event),
                pid,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            )
        };
        if attached && self.global.is_empty() {
            self.global = vec![
                hook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, 0),
                hook(EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND, 0),
            ];
        } else if !attached {
            for hook in self.global.drain(..) {
                unsafe {
                    let _ = UnhookWinEvent(hook);
                }
            }
        }
        let wanted: Vec<u32> = self.wins.iter().filter_map(|w| w.pid).collect();
        self.process.retain(|(pid, hooks)| {
            let keep = wanted.contains(pid);
            if !keep {
                for &hook in hooks {
                    unsafe {
                        let _ = UnhookWinEvent(hook);
                    }
                }
            }
            keep
        });
        for pid in wanted {
            if !self.process.iter().any(|(p, _)| *p == pid) {
                self.process.push((
                    pid,
                    vec![
                        hook(EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE, pid),
                        hook(EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_NAMECHANGE, pid),
                        hook(EVENT_OBJECT_CLOAKED, EVENT_OBJECT_UNCLOAKED, pid),
                    ],
                ));
            }
        }
    }

    fn unhook_all(&mut self) {
        for hook in self.global.drain(..) {
            unsafe {
                let _ = UnhookWinEvent(hook);
            }
        }
        for (_, hooks) in self.process.drain(..) {
            for hook in hooks {
                unsafe {
                    let _ = UnhookWinEvent(hook);
                }
            }
        }
    }

    fn window_event(&mut self, event: u32, hwnd: HWND) {
        let is_target = self.wins.iter().any(|w| w.target == Some(hwnd));
        if event == EVENT_OBJECT_LOCATIONCHANGE {
            for index in 0..self.wins.len() {
                if self.wins[index].target == Some(hwnd) {
                    self.follow(index);
                }
            }
            return;
        }
        let top_level = unsafe { GetAncestor(hwnd, GA_ROOT) } == hwnd;
        if !is_target && (event == EVENT_OBJECT_DESTROY || !top_level) {
            return;
        }
        for index in 0..self.wins.len() {
            if self.wins[index].note.attach.is_some() {
                self.follow(index);
            }
        }
        self.sync_hooks();
    }

    fn flush(&mut self) {
        for win in &mut self.wins {
            if std::mem::take(&mut win.dirty) {
                store::write_text(win.note.id, &edit_text(win.edit));
            }
        }
        self.save_layout();
    }

    fn save_layout(&mut self) {
        for win in &mut self.wins {
            win.note.zoom = zoom_of(win.edit);
        }
        let notes: Vec<Note> = self.wins.iter().map(|w| w.note.clone()).collect();
        store::save(&notes);
    }

    fn delete(&mut self, id: u64) {
        let Some(index) = self.wins.iter().position(|w| w.note.id == id) else {
            return;
        };
        let win = self.wins.remove(index);
        unsafe {
            let _ = KillTimer(Some(win.hwnd), SAVE_TIMER);
        }
        drop(win);
        if let Some(path) = store::dir().map(|d| d.join(format!("{id}.txt"))) {
            let _ = std::fs::remove_file(path);
        }
        self.save_layout();
        self.sync_hooks();
    }

    fn button_at(win: &Win, x: i32, y: i32) -> Option<Button> {
        let mut rect = RECT::default();
        unsafe {
            let _ = GetClientRect(win.hwnd, &mut rect);
        }
        let size = px(BUTTON, win.dpi);
        let top = (px(HEADER, win.dpi) - size) / 2;
        if y < top || y >= top + size {
            return None;
        }
        let right = rect.right - px(4.0, win.dpi);
        let close_w = if win.armed { px(80.0, win.dpi) } else { size };
        if x >= right - close_w && x < right {
            return Some(Button::Close);
        }
        if x >= right - close_w - size && x < right - close_w {
            return Some(Button::Menu);
        }
        None
    }

    fn handle(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Reply {
        let Some(index) = self.index(hwnd) else {
            return Reply::Default;
        };
        let x = (lparam.0 & 0xFFFF) as i16 as i32;
        let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
        match msg {
            WM_NCHITTEST => {
                let win = &self.wins[index];
                let rect = window_rect(hwnd);
                let (lx, ly) = (x - rect.left, y - rect.top);
                let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
                let edge = px(EDGE, win.dpi);
                let (left, right) = (lx < edge, lx >= w - edge);
                let (top, bottom) = (ly < edge, ly >= h - edge);
                let code = match (left, right, top, bottom) {
                    (true, _, true, _) => HTTOPLEFT,
                    (_, true, true, _) => HTTOPRIGHT,
                    (true, _, _, true) => HTBOTTOMLEFT,
                    (_, true, _, true) => HTBOTTOMRIGHT,
                    (true, ..) => HTLEFT,
                    (_, true, ..) => HTRIGHT,
                    (_, _, true, _) => HTTOP,
                    (.., true) => HTBOTTOM,
                    _ if ly < px(HEADER, win.dpi) && Self::button_at(win, lx, ly).is_none() => {
                        HTCAPTION
                    }
                    _ => HTCLIENT,
                };
                Reply::Done(LRESULT(code as isize))
            }
            WM_MOUSEMOVE => {
                let win = &mut self.wins[index];
                if !win.tracking {
                    let mut track = TRACKMOUSEEVENT {
                        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    unsafe {
                        let _ = TrackMouseEvent(&mut track);
                    }
                    win.tracking = true;
                }
                let hover = Self::button_at(win, x, y);
                if hover != win.hover {
                    win.hover = hover;
                    invalidate_header(win);
                }
                Reply::Done(LRESULT(0))
            }
            WM_MOUSELEAVE => {
                let win = &mut self.wins[index];
                win.tracking = false;
                if win.hover.take().is_some() {
                    invalidate_header(win);
                }
                Reply::Done(LRESULT(0))
            }
            WM_LBUTTONUP | WM_RBUTTONUP => {
                let button = if msg == WM_RBUTTONUP {
                    Some(Button::Menu)
                } else {
                    Self::button_at(&self.wins[index], x, y)
                };
                match button {
                    Some(Button::Menu) => {
                        let mut at = POINT { x, y };
                        unsafe {
                            let _ = windows::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut at);
                        }
                        Reply::Menu(self.menu(index), at)
                    }
                    Some(Button::Close) => {
                        self.close_clicked(index);
                        Reply::Done(LRESULT(0))
                    }
                    None => Reply::Done(LRESULT(0)),
                }
            }
            WM_NCRBUTTONUP if wparam.0 == HTCAPTION as usize => {
                Reply::Menu(self.menu(index), POINT { x, y })
            }
            WM_PAINT => {
                paint(&self.wins[index]);
                Reply::Done(LRESULT(0))
            }
            WM_ERASEBKGND => Reply::Done(LRESULT(1)),
            WM_TIMER if wparam.0 == SAVE_TIMER => {
                let win = &mut self.wins[index];
                unsafe {
                    let _ = KillTimer(Some(hwnd), SAVE_TIMER);
                }
                if std::mem::take(&mut win.dirty) {
                    store::write_text(win.note.id, &edit_text(win.edit));
                }
                Reply::Done(LRESULT(0))
            }
            WM_TIMER if wparam.0 == ARM_TIMER => {
                let win = &mut self.wins[index];
                unsafe {
                    let _ = KillTimer(Some(hwnd), ARM_TIMER);
                }
                win.armed = false;
                invalidate_header(win);
                Reply::Done(LRESULT(0))
            }
            WM_COMMAND if (wparam.0 >> 16) as u32 & 0xFFFF == EN_CHANGE => {
                let win = &mut self.wins[index];
                win.dirty = true;
                if win.armed {
                    win.armed = false;
                    invalidate_header(win);
                }
                unsafe {
                    SetTimer(Some(hwnd), SAVE_TIMER, SAVE_DELAY_MS, None);
                }
                Reply::Done(LRESULT(0))
            }
            WM_NOTIFY => {
                let header = unsafe { &*(lparam.0 as *const NMHDR) };
                if header.code != EN_LINK {
                    return Reply::Default;
                }
                let link = unsafe { &*(lparam.0 as *const ENLINK) };
                if link.msg != WM_LBUTTONUP {
                    return Reply::Done(LRESULT(0));
                }
                let edit = self.wins[index].edit;
                let mut selection = CHARRANGE::default();
                send(
                    edit,
                    EM_EXGETSEL,
                    0,
                    &mut selection as *mut CHARRANGE as isize,
                );
                if selection.cpMin != selection.cpMax {
                    return Reply::Done(LRESULT(0));
                }
                let range = link.chrg;
                let length = (range.cpMax - range.cpMin).clamp(0, 2048) as usize;
                let mut buffer = vec![0u16; length + 1];
                let mut request = TEXTRANGEW {
                    chrg: range,
                    lpstrText: PWSTR(buffer.as_mut_ptr()),
                };
                let copied = send(
                    edit,
                    EM_GETTEXTRANGE,
                    0,
                    &mut request as *mut TEXTRANGEW as isize,
                );
                let url = String::from_utf16_lossy(&buffer[..(copied.max(0) as usize).min(length)]);
                if url.trim().is_empty() {
                    Reply::Done(LRESULT(0))
                } else {
                    Reply::Open(url.trim().to_string())
                }
            }
            WM_ACTIVATE => {
                let win = &self.wins[index];
                if (wparam.0 & 0xFFFF) as u32 == WA_INACTIVE {
                    if zoom_of(win.edit) != win.note.zoom {
                        self.save_layout();
                    }
                    Reply::Default
                } else {
                    unsafe {
                        let _ = SetFocus(Some(win.edit));
                    }
                    Reply::Done(LRESULT(0))
                }
            }
            WM_EXITSIZEMOVE => {
                self.moved(index);
                Reply::Done(LRESULT(0))
            }
            WM_GETMINMAXINFO => {
                let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
                let info = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
                info.ptMinTrackSize = POINT {
                    x: px(MIN_W, dpi),
                    y: px(MIN_H, dpi),
                };
                Reply::Done(LRESULT(0))
            }
            WM_DPICHANGED => {
                let suggested = unsafe { &*(lparam.0 as *const RECT) };
                let win = &mut self.wins[index];
                win.dpi = ((wparam.0 >> 16) as u32 & 0xFFFF).max(96);
                let (glyphs, small) = header_fonts(win.dpi);
                unsafe {
                    let _ = DeleteObject(HGDIOBJ(win.glyphs.0));
                    let _ = DeleteObject(HGDIOBJ(win.small.0));
                }
                win.glyphs = glyphs;
                win.small = small;
                apply_text_format(win.edit, win.dpi);
                unsafe {
                    let _ = SetWindowPos(
                        hwnd,
                        None,
                        suggested.left,
                        suggested.top,
                        suggested.right - suggested.left,
                        suggested.bottom - suggested.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                Reply::Done(LRESULT(0))
            }
            WM_QUERYENDSESSION => {
                self.flush();
                Reply::Done(LRESULT(1))
            }
            WM_ENDSESSION => {
                self.flush();
                Reply::Done(LRESULT(0))
            }
            _ => Reply::Default,
        }
    }

    fn moved(&mut self, index: usize) {
        let win = &mut self.wins[index];
        let rect = window_rect(win.hwnd);
        win.note.x = rect.left;
        win.note.y = rect.top;
        win.note.w = rect.right - rect.left;
        win.note.h = rect.bottom - rect.top;
        if let (Some(attach), Some(target)) = (&mut win.note.attach, win.target)
            && let Some(bounds) = attach::bounds(target)
        {
            let (corner, dx, dy) = attach::anchor(rect, bounds);
            attach.corner = corner;
            attach.dx = dx;
            attach.dy = dy;
        }
        self.save_layout();
    }

    fn close_clicked(&mut self, index: usize) {
        let win = &mut self.wins[index];
        if win.armed || edit_text(win.edit).trim().is_empty() {
            let id = win.note.id;
            self.delete(id);
            return;
        }
        win.armed = true;
        invalidate_header(win);
        unsafe {
            SetTimer(Some(win.hwnd), ARM_TIMER, ARM_MS, None);
        }
    }

    fn menu(&mut self, index: usize) -> Vec<MenuItem> {
        let win = &self.wins[index];
        let attached = win.note.attach.is_some();
        let mode = win.note.mode;
        let mut items = vec![
            MenuItem::Radio(
                MENU_TOP,
                tr("Поверх окон", "Always on top"),
                !attached && mode == Mode::Top,
            ),
            MenuItem::Radio(
                MENU_DESKTOP,
                tr("На рабочем столе", "On the desktop"),
                !attached && mode == Mode::Desktop,
            ),
            MenuItem::Radio(
                MENU_NORMAL,
                tr("Обычное окно", "Normal window"),
                !attached && mode == Mode::Normal,
            ),
        ];
        self.candidate = None;
        if attached {
            items.push(MenuItem::Check(
                MENU_ATTACH,
                format!("{} {}", tr("Приклеена к окну:", "Stuck to:"), win.label),
                true,
            ));
        } else {
            let rect = window_rect(win.hwnd);
            let center = POINT {
                x: (rect.left + rect.right) / 2,
                y: (rect.top + rect.bottom) / 2,
            };
            match attach::window_at(center) {
                Some(hwnd) => {
                    let exe = attach::exe_of_process(attach::pid_of(hwnd)).unwrap_or_default();
                    items.push(MenuItem::Check(
                        MENU_ATTACH,
                        format!(
                            "{} {}",
                            tr("Приклеить к окну:", "Stick to window:"),
                            attach::label(hwnd, &exe)
                        ),
                        false,
                    ));
                    self.candidate = Some(hwnd);
                }
                None => items.push(MenuItem::Label(
                    0,
                    tr(
                        "Приклеить к окну: поставьте заметку на окно",
                        "Stick to window: put the note over a window",
                    )
                    .to_string(),
                )),
            }
        }
        items.extend([
            MenuItem::Separator,
            MenuItem::Action(MENU_NEW, tr("Новая заметка", "New note")),
            MenuItem::Action(MENU_DELETE, tr("Удалить заметку", "Delete note")),
        ]);
        items
    }

    fn menu_chosen(&mut self, hwnd: HWND, command: u32) {
        let Some(index) = self.index(hwnd) else {
            return;
        };
        let mode = match command {
            MENU_TOP => Some(Mode::Top),
            MENU_DESKTOP => Some(Mode::Desktop),
            MENU_NORMAL => Some(Mode::Normal),
            _ => None,
        };
        if let Some(mode) = mode {
            let win = &mut self.wins[index];
            win.note.attach = None;
            win.note.mode = mode;
            self.show(index);
        } else if command == MENU_ATTACH {
            let Some(target) = self.candidate.take() else {
                return;
            };
            let win = &mut self.wins[index];
            if win.note.attach.is_some() {
                return;
            }
            let Some(exe) = attach::exe_of_process(attach::pid_of(target)) else {
                return;
            };
            let Some(bounds) = attach::bounds(target) else {
                return;
            };
            let tab = if attach::is_browser(&exe) {
                attach::tab_title(&attach::title_of(target)).to_string()
            } else {
                String::new()
            };
            let (corner, dx, dy) = attach::anchor(window_rect(win.hwnd), bounds);
            win.label = attach::label(target, &exe);
            win.note.attach = Some(Attach {
                exe,
                tab,
                corner,
                dx,
                dy,
            });
            win.target = Some(target);
            self.show(index);
        } else if command == MENU_DELETE {
            let id = self.wins[index].note.id;
            self.delete(id);
            return;
        } else {
            return;
        }
        self.sync_hooks();
        self.save_layout();
        let win = &self.wins[index];
        unsafe {
            let _ = InvalidateRect(Some(win.hwnd), None, false);
        }
    }
}

fn keep_on_screen(win: &mut Win) {
    let rect = window_rect(win.hwnd);
    let monitor = unsafe { MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        let _ = GetMonitorInfoW(monitor, &mut info);
    }
    let work = info.rcWork;
    let w = rect.right - rect.left;
    let grip = px(HEADER, win.dpi);
    let x = rect
        .left
        .clamp(work.left - w + grip * 2, work.right - grip * 2);
    let y = rect.top.clamp(work.top, work.bottom - grip);
    if (x, y) != (rect.left, rect.top) {
        unsafe {
            let _ = SetWindowPos(
                win.hwnd,
                None,
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        win.note.x = x;
        win.note.y = y;
    }
}

fn invalidate_header(win: &Win) {
    let mut rect = RECT::default();
    unsafe {
        let _ = GetClientRect(win.hwnd, &mut rect);
    }
    rect.bottom = px(HEADER, win.dpi);
    unsafe {
        let _ = InvalidateRect(Some(win.hwnd), Some(&rect), false);
    }
}

fn paint(win: &Win) {
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let dc = BeginPaint(win.hwnd, &mut ps);
        let mut client = RECT::default();
        let _ = GetClientRect(win.hwnd, &mut client);
        let fill = |rect: &RECT, rgb: u32| {
            let brush = CreateSolidBrush(colorref(rgb));
            FillRect(dc, rect, brush);
            let _ = DeleteObject(HGDIOBJ(brush.0));
        };
        fill(&client, BACK);
        let border = CreateSolidBrush(colorref(BORDER));
        FrameRect(dc, &client, border);
        let _ = DeleteObject(HGDIOBJ(border.0));

        SetBkMode(dc, TRANSPARENT);
        let dpi = win.dpi;
        let header = px(HEADER, dpi);
        let size = px(BUTTON, dpi);
        let top = (header - size) / 2;
        let right = client.right - px(4.0, dpi);
        let old = SelectObject(dc, HGDIOBJ(win.glyphs.0));
        let draw = |text: &str, rect: &mut RECT, rgb: u32, flags| {
            SetTextColor(dc, colorref(rgb));
            let mut wide: Vec<u16> = text.encode_utf16().collect();
            DrawTextW(dc, &mut wide, rect, flags);
        };
        let center = DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX;

        let glyph = match (&win.note.attach, win.note.mode) {
            (Some(_), _) => GLYPH_ATTACH,
            (None, Mode::Top) => GLYPH_TOP,
            (None, Mode::Desktop) => GLYPH_DESKTOP,
            (None, Mode::Normal) => GLYPH_NORMAL,
        };
        let left = px(4.0, dpi);
        let mut rect = RECT {
            left,
            top,
            right: left + size,
            bottom: top + size,
        };
        draw(&glyph.to_string(), &mut rect, DIM, center);

        let close_w = if win.armed { px(80.0, dpi) } else { size };
        let close = RECT {
            left: right - close_w,
            top,
            right,
            bottom: top + size,
        };
        let more = RECT {
            left: close.left - size,
            top,
            right: close.left,
            bottom: top + size,
        };
        for (button, rect) in [(Button::Menu, more), (Button::Close, close)] {
            if win.hover == Some(button) {
                fill(&rect, HOVER);
            }
        }
        let mut more_text = more;
        draw(&GLYPH_MORE.to_string(), &mut more_text, DIM, center);
        if win.armed {
            SelectObject(dc, HGDIOBJ(win.small.0));
            let mut label = close;
            draw(tr("Удалить?", "Delete?"), &mut label, WARN, center);
        } else {
            let mut close_text = close;
            draw(&GLYPH_CLOSE.to_string(), &mut close_text, DIM, center);
        }

        if win.note.attach.is_some() && !win.label.is_empty() {
            SelectObject(dc, HGDIOBJ(win.small.0));
            let mut label = RECT {
                left: left + size,
                top,
                right: more.left - px(4.0, dpi),
                bottom: top + size,
            };
            draw(
                &win.label,
                &mut label,
                DIM,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
            );
        }
        SelectObject(dc, old);
        let _ = EndPaint(win.hwnd, &ps);
    }
}
