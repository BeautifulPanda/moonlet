use std::cell::RefCell;
use std::path::Path;

use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, LPARAM, LRESULT, WAIT_OBJECT_0,
    WPARAM,
};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{CreateMutexW, INFINITE};
use windows::Win32::UI::HiDpi::GetSystemMetricsForDpi;
#[cfg(any(feature = "orbit", feature = "settings"))]
use windows::Win32::UI::Shell::ShellExecuteW;
#[cfg(any(feature = "orbit", feature = "settings"))]
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
#[cfg(feature = "settings")]
use windows::Win32::UI::WindowsAndMessaging::{ASFW_ANY, AllowSetForegroundWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, HICON, KillTimer, MSG,
    MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE, PeekMessageW, PostQuitMessage,
    QS_ALLINPUT, RegisterClassW, RegisterWindowMessageW, SM_CXSMICON, SetTimer, TranslateMessage,
    WINDOW_EX_STYLE, WM_APP, WM_COMMAND, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_QUIT, WM_TIMER,
    WNDCLASSW, WS_OVERLAPPED,
};
#[cfg(feature = "settings")]
use windows::core::HSTRING;
use windows::core::{Error, PCWSTR, Result, w};

use crate::autostart;
use crate::config::{self, Config};
use crate::icon;
use crate::lang::{self, Language, tr};
use crate::screen;
use crate::tray::{self, MenuItem, Tray};
use crate::watch::DirWatch;

#[cfg(feature = "orbit")]
use crate::config::OrbitConfig;
#[cfg(feature = "orbit")]
use crate::fullscreen::{self, FullscreenWatch};
#[cfg(feature = "launcher")]
use crate::hotkey::Hotkey;
#[cfg(feature = "orbit")]
use crate::orbit::Orbit;
#[cfg(feature = "orbit")]
use crate::power::PowerEvent;
#[cfg(feature = "orbit")]
use crate::power::{self, PowerWatch};
#[cfg(feature = "update")]
use crate::update::{self, Mode as UpdateMode, Outcome, Updater};
#[cfg(feature = "update")]
use std::path::PathBuf;
#[cfg(feature = "launcher")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey,
};
#[cfg(feature = "launcher")]
use windows::Win32::UI::WindowsAndMessaging::WM_HOTKEY;
#[cfg(feature = "orbit")]
use windows::Win32::UI::WindowsAndMessaging::{WM_POWERBROADCAST, WM_TIMECHANGE};

const HOST_CLASS: PCWSTR = w!("MoonletHost");

const RELOAD_TIMER: usize = 1;
const RELOAD_DELAY_MS: u32 = 200;

#[cfg(feature = "orbit")]
const RESTART_TIMER: usize = 2;
#[cfg(feature = "orbit")]
const RESTART_DELAY_MS: u32 = 2000;
#[cfg(feature = "orbit")]
const RESTART_TRIES: u32 = 5;

pub const WM_SCREEN_CHANGED: u32 = WM_APP + 4;

const CMD_EXIT: u32 = 100;
const CMD_AUTOSTART: u32 = 105;
pub const CMD_OPEN_CONFIG: u32 = 106;
#[cfg(feature = "settings")]
const CMD_OPEN_CONFIG_FILE: u32 = 116;
#[cfg(feature = "orbit")]
pub const CMD_SHAPE_CIRCLE: u32 = 101;
#[cfg(feature = "orbit")]
pub const CMD_SHAPE_OVAL: u32 = 102;
#[cfg(feature = "orbit")]
pub const CMD_ROTATION: u32 = 103;
#[cfg(feature = "orbit")]
const CMD_PAUSE_ON_BATTERY: u32 = 104;
#[cfg(feature = "orbit")]
const CMD_MODULE_ORBIT: u32 = 107;
#[cfg(feature = "orbit")]
pub const CMD_SCREENSHOT: u32 = 108;
#[cfg(feature = "orbit")]
pub const CMD_SHAPE_TOGGLE: u32 = 109;
#[cfg(feature = "update")]
const CMD_UPDATE_AUTO: u32 = 110;
#[cfg(feature = "update")]
const CMD_UPDATE_CHECK: u32 = 111;
#[cfg(feature = "update")]
const CMD_UPDATE_OFF: u32 = 112;
#[cfg(feature = "update")]
pub const CMD_UPDATE_NOW: u32 = 113;
#[cfg(feature = "update")]
const CMD_UPDATE_INSTALL: u32 = 114;
#[cfg(feature = "update")]
const UPDATE_TIMER: usize = 3;
#[cfg(feature = "update")]
const FIRST_CHECK_MS: u32 = 60_000;
#[cfg(feature = "update")]
const DAY_MS: u32 = 24 * 60 * 60 * 1000;
#[cfg(feature = "launcher")]
const CMD_MODULE_LAUNCHER: u32 = 115;
#[cfg(feature = "launcher")]
const HOTKEY_ID: i32 = 1;
#[cfg(all(feature = "orbit", feature = "dev"))]
const CMD_DEV_DEVICE_LOST: u32 = 900;
#[cfg(all(feature = "orbit", feature = "dev"))]
const CMD_DEV_NOTICE: u32 = 901;

pub struct SingleInstance(HANDLE);

impl SingleInstance {
    pub fn acquire() -> Option<Self> {
        Self::named(w!("Local\\Moonlet.SingleInstance"))
    }

    pub fn named(name: PCWSTR) -> Option<Self> {
        unsafe {
            let handle = CreateMutexW(None, true, name).ok()?;
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let _ = CloseHandle(handle);
                return None;
            }
            Some(Self(handle))
        }
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

struct App {
    hwnd: HWND,
    taskbar_created: u32,
    tray: Tray,
    tray_dpi: u32,
    config: Config,
    #[cfg(feature = "orbit")]
    _power: PowerWatch,
    #[cfg(feature = "orbit")]
    fullscreen_watch: FullscreenWatch,
    #[cfg(feature = "orbit")]
    on_battery: bool,
    #[cfg(feature = "orbit")]
    fullscreen: bool,
    #[cfg(feature = "orbit")]
    orbit: Option<Orbit>,
    #[cfg(feature = "orbit")]
    restart_tries: u32,
    #[cfg(feature = "launcher")]
    hotkey: Option<Hotkey>,
    #[cfg(feature = "update")]
    updater: Updater,
    #[cfg(feature = "update")]
    update_available: Option<String>,
    #[cfg(feature = "update")]
    update_ready: Option<PathBuf>,
    #[cfg(feature = "update")]
    update_manual: bool,
    #[cfg(feature = "update")]
    update_install: bool,
    #[cfg(feature = "update")]
    just_updated: bool,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

pub fn run(updated: bool) -> Result<()> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
    let hwnd = create_host_window()?;
    let tray_dpi = screen::dpi();
    let tray = Tray::new(hwnd, tray_icon(tray_dpi)?)?;

    lang::set(Language::Auto);
    let config = Config::load().unwrap_or_else(|error| {
        tray.notify(
            tr("Moonlet: ошибка в настройках", "Moonlet: settings error"),
            &format!(
                "config.toml, {error}. {}",
                tr(
                    "Используются настройки по умолчанию.",
                    "Using the default settings."
                )
            ),
        );
        Config::default()
    });
    lang::set(config.language);
    autostart::apply(config.autostart);

    #[allow(unused_mut)]
    let mut app = App {
        hwnd,
        taskbar_created: unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
        tray,
        tray_dpi,
        config,
        #[cfg(feature = "orbit")]
        _power: PowerWatch::register(hwnd),
        #[cfg(feature = "orbit")]
        fullscreen_watch: FullscreenWatch::register(hwnd),
        #[cfg(feature = "orbit")]
        on_battery: power::on_battery(),
        #[cfg(feature = "orbit")]
        fullscreen: fullscreen::active_now(),
        #[cfg(feature = "orbit")]
        orbit: None,
        #[cfg(feature = "orbit")]
        restart_tries: 0,
        #[cfg(feature = "launcher")]
        hotkey: None,
        #[cfg(feature = "update")]
        updater: Updater::new(hwnd),
        #[cfg(feature = "update")]
        update_available: None,
        #[cfg(feature = "update")]
        update_ready: None,
        #[cfg(feature = "update")]
        update_manual: false,
        #[cfg(feature = "update")]
        update_install: false,
        #[cfg(feature = "update")]
        just_updated: updated,
    };
    #[cfg(feature = "orbit")]
    app.start_orbit();
    #[cfg(feature = "launcher")]
    app.sync_hotkey();
    #[cfg(feature = "update")]
    {
        update::clean_up();
        if updated {
            app.tray.inform(
                tr("Moonlet обновлён", "Moonlet updated"),
                &format!("{} {}", tr("Версия", "Version"), update::current_version()),
            );
        }
        app.schedule_update_check(FIRST_CHECK_MS);
    }
    #[cfg(not(feature = "update"))]
    let _ = updated;
    APP.with(|cell| *cell.borrow_mut() = Some(app));

    let watch = config::path()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .and_then(|dir| DirWatch::new(&dir));
    message_loop(watch.as_ref());

    drop(APP.with(|cell| cell.borrow_mut().take()));
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
    Ok(())
}

// Sleeps until a window message, a change in the config folder or a lost
// GPU device. Nothing is polled.
fn message_loop(watch: Option<&DirWatch>) {
    let mut msg = MSG::default();
    loop {
        let mut handles = [HANDLE::default(); 2];
        let mut count = 0;
        if let Some(watch) = watch {
            handles[count] = watch.handle();
            count += 1;
        }
        #[cfg(feature = "orbit")]
        if let Some(Some(lost)) = with_app(|app| app.device_lost_event()) {
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
                with_app(|app| app.config_file_changed());
            }
            #[cfg(feature = "orbit")]
            (_, Some(_)) => {
                with_app(|app| app.device_lost());
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

fn create_host_window() -> Result<HWND> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: HOST_CLASS,
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            return Err(Error::from_thread());
        }
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            HOST_CLASS,
            w!("Moonlet"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )
    }
}

pub unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // TrackPopupMenu runs a nested message loop that re-enters window_proc,
    // so the menu is shown while App is not borrowed.
    if msg == tray::WM_TRAY {
        if let Some(at) = tray::menu_request(wparam, lparam)
            && let Some(items) = with_app(|app| app.menu())
        {
            let command = tray::show_menu(hwnd, &items, at);
            with_app(|app| app.command(command));
        }
        #[cfg(feature = "update")]
        if tray::balloon_clicked(lparam) {
            with_app(|app| app.balloon_clicked());
        }
        return LRESULT(0);
    }

    match with_app(|app| app.handle(hwnd, msg, wparam, lparam)) {
        Some(Some(result)) => result,
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|cell| {
        let mut slot = cell.try_borrow_mut().ok()?;
        slot.as_mut().map(f)
    })
}

fn tray_icon(dpi: u32) -> Result<HICON> {
    icon::moon(unsafe { GetSystemMetricsForDpi(SM_CXSMICON, dpi) })
}

#[cfg(feature = "orbit")]
fn orbit_failed() -> &'static str {
    tr(
        "Moonlet: орбита не запустилась",
        "Moonlet: the orbit failed to start",
    )
}

fn yes_no(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

impl App {
    fn handle(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        #[cfg(feature = "orbit")]
        if hwnd != self.hwnd {
            return self.orbit.as_mut()?.handle(hwnd, msg, wparam, lparam);
        }
        #[cfg(not(feature = "orbit"))]
        let _ = (hwnd, lparam);

        match msg {
            WM_COMMAND => {
                self.command((wparam.0 & 0xFFFF) as u32);
                Some(LRESULT(0))
            }
            WM_TIMER if wparam.0 == RELOAD_TIMER => {
                unsafe {
                    let _ = KillTimer(Some(self.hwnd), RELOAD_TIMER);
                }
                self.reload_config();
                Some(LRESULT(0))
            }
            #[cfg(feature = "update")]
            WM_TIMER if wparam.0 == UPDATE_TIMER => {
                self.update_timer();
                Some(LRESULT(0))
            }
            #[cfg(feature = "update")]
            update::WM_UPDATE => {
                self.update_finished();
                Some(LRESULT(0))
            }
            #[cfg(feature = "orbit")]
            WM_TIMER if wparam.0 == RESTART_TIMER => {
                unsafe {
                    let _ = KillTimer(Some(self.hwnd), RESTART_TIMER);
                }
                self.restart_orbit();
                Some(LRESULT(0))
            }
            #[cfg(feature = "orbit")]
            WM_POWERBROADCAST => {
                match power::parse(wparam, lparam) {
                    Some(PowerEvent::Source(on_battery)) => {
                        // The power source is also reported right after registering; only a real
                        // change shows a notice.
                        let changed = on_battery != self.on_battery;
                        self.on_battery = on_battery;
                        self.update_rotation();
                        self.with_orbit(|orbit| {
                            orbit.refresh_battery()?;
                            if changed {
                                orbit.power_changed(on_battery)?;
                            }
                            Ok(())
                        });
                    }
                    Some(PowerEvent::Battery) => self.with_orbit(|orbit| {
                        orbit.refresh_battery()?;
                        orbit.check_low_battery()
                    }),
                    Some(PowerEvent::Resumed) => self.with_orbit(|orbit| {
                        orbit.refresh_battery()?;
                        orbit.refresh_clock()
                    }),
                    None => {}
                }
                Some(LRESULT(1))
            }
            #[cfg(feature = "launcher")]
            WM_HOTKEY if wparam.0 == HOTKEY_ID as usize => {
                if !self.fullscreen {
                    let search = self.config.launcher.search.clone();
                    self.with_orbit(|orbit| orbit.toggle_launcher(&search));
                }
                Some(LRESULT(0))
            }
            #[cfg(feature = "orbit")]
            WM_TIMECHANGE => {
                self.with_orbit(|orbit| orbit.refresh_clock());
                Some(LRESULT(0))
            }
            #[cfg(feature = "orbit")]
            fullscreen::WM_APPBAR => {
                if let Some(fullscreen) = fullscreen::parse(wparam, lparam) {
                    self.set_fullscreen(fullscreen);
                }
                Some(LRESULT(0))
            }
            WM_DISPLAYCHANGE | WM_DPICHANGED | WM_SCREEN_CHANGED => {
                self.screen_changed();
                Some(LRESULT(0))
            }
            _ if msg == self.taskbar_created => {
                // Explorer restarted: the tray icon and the appbar registration are gone.
                self.update_tray_icon();
                let _ = self.tray.add();
                #[cfg(feature = "orbit")]
                {
                    self.fullscreen_watch.renew();
                    let fullscreen = fullscreen::active_now();
                    if fullscreen != self.fullscreen {
                        self.set_fullscreen(fullscreen);
                    }
                }
                Some(LRESULT(0))
            }
            _ => None,
        }
    }

    #[allow(clippy::vec_init_then_push)]
    fn menu(&self) -> Vec<MenuItem> {
        let mut items = Vec::new();
        #[cfg(feature = "orbit")]
        {
            if self.orbit.is_some() {
                let o = &self.config.orbit;
                let radii = (o.radius_x, o.radius_y);
                items.extend([
                    MenuItem::Radio(
                        CMD_SHAPE_CIRCLE,
                        tr("Орбита: круг", "Orbit: circle"),
                        radii == OrbitConfig::CIRCLE,
                    ),
                    MenuItem::Radio(
                        CMD_SHAPE_OVAL,
                        tr("Орбита: овал", "Orbit: oval"),
                        radii == OrbitConfig::OVAL,
                    ),
                    MenuItem::Separator,
                    MenuItem::Check(CMD_ROTATION, tr("Вращение", "Rotation").into(), o.rotate),
                ]);
                if power::has_battery() {
                    items.push(MenuItem::Check(
                        CMD_PAUSE_ON_BATTERY,
                        tr("Пауза от батареи", "Pause on battery").into(),
                        self.config.pause_on_battery,
                    ));
                }
                items.push(MenuItem::Separator);
            }
            items.push(MenuItem::Check(
                CMD_MODULE_ORBIT,
                tr("Модуль «Орбита»", "Orbit module").into(),
                self.config.orbit_enabled,
            ));
        }
        #[cfg(feature = "launcher")]
        if self.config.orbit_enabled {
            items.push(MenuItem::Check(
                CMD_MODULE_LAUNCHER,
                format!(
                    "{}\t{}",
                    tr("Командная строка", "Command line"),
                    self.config.launcher.hotkey.name()
                ),
                self.config.launcher_enabled,
            ));
        }
        items.push(MenuItem::Check(
            CMD_AUTOSTART,
            tr("Запускать с Windows", "Start with Windows").into(),
            self.config.autostart,
        ));
        #[cfg(feature = "update")]
        items.push(self.update_menu());
        items.push(MenuItem::Action(
            CMD_OPEN_CONFIG,
            tr("Открыть настройки", "Open settings"),
        ));
        #[cfg(feature = "settings")]
        items.push(MenuItem::Action(
            CMD_OPEN_CONFIG_FILE,
            tr("Открыть config.toml", "Open config.toml"),
        ));
        items.extend([
            MenuItem::Separator,
            MenuItem::Action(CMD_EXIT, tr("Выход", "Exit")),
        ]);
        items
    }

    fn command(&mut self, id: u32) {
        match id {
            CMD_EXIT => unsafe { PostQuitMessage(0) },
            CMD_AUTOSTART => {
                let on = !self.config.autostart;
                self.config.autostart = on;
                autostart::apply(on);
                self.save("general", "autostart", yes_no(on));
            }
            CMD_OPEN_CONFIG => self.open_settings(false),
            #[cfg(feature = "settings")]
            CMD_OPEN_CONFIG_FILE => config::open_in_notepad(),
            #[cfg(feature = "orbit")]
            CMD_SCREENSHOT => open_screen_snip(),
            #[cfg(feature = "orbit")]
            CMD_SHAPE_TOGGLE => {
                let o = &self.config.orbit;
                let circle = (o.radius_x, o.radius_y) == OrbitConfig::CIRCLE;
                self.set_shape(if circle {
                    OrbitConfig::OVAL
                } else {
                    OrbitConfig::CIRCLE
                });
            }
            #[cfg(feature = "orbit")]
            CMD_SHAPE_CIRCLE => self.set_shape(OrbitConfig::CIRCLE),
            #[cfg(feature = "orbit")]
            CMD_SHAPE_OVAL => self.set_shape(OrbitConfig::OVAL),
            #[cfg(feature = "orbit")]
            CMD_ROTATION => {
                let on = !self.config.orbit.rotate;
                self.config.orbit.rotate = on;
                self.save("orbit", "rotate", yes_no(on));
                self.update_rotation();
            }
            #[cfg(feature = "orbit")]
            CMD_PAUSE_ON_BATTERY => {
                let on = !self.config.pause_on_battery;
                self.config.pause_on_battery = on;
                self.save("general", "pause_on_battery", yes_no(on));
                self.update_rotation();
            }
            #[cfg(feature = "orbit")]
            CMD_MODULE_ORBIT => {
                let on = !self.config.orbit_enabled;
                self.config.orbit_enabled = on;
                self.save("modules", "orbit", yes_no(on));
                if on {
                    self.start_orbit();
                } else {
                    self.orbit = None;
                }
                #[cfg(feature = "launcher")]
                self.sync_hotkey();
            }
            #[cfg(feature = "launcher")]
            CMD_MODULE_LAUNCHER => {
                let on = !self.config.launcher_enabled;
                self.config.launcher_enabled = on;
                self.save("modules", "launcher", yes_no(on));
                self.sync_hotkey();
                self.sync_hub();
            }
            #[cfg(feature = "update")]
            CMD_UPDATE_AUTO => self.set_update_mode(UpdateMode::Auto),
            #[cfg(feature = "update")]
            CMD_UPDATE_CHECK => self.set_update_mode(UpdateMode::Check),
            #[cfg(feature = "update")]
            CMD_UPDATE_OFF => self.set_update_mode(UpdateMode::Off),
            #[cfg(feature = "update")]
            CMD_UPDATE_NOW => self.check_updates(false),
            #[cfg(feature = "update")]
            CMD_UPDATE_INSTALL => self.check_updates(true),
            #[cfg(all(feature = "orbit", feature = "dev"))]
            CMD_DEV_DEVICE_LOST => {
                if let Some(orbit) = &self.orbit {
                    orbit.simulate_device_lost();
                }
            }
            #[cfg(all(feature = "orbit", feature = "dev"))]
            CMD_DEV_NOTICE => {
                let on_battery = self.on_battery;
                self.with_orbit(|orbit| orbit.power_changed(on_battery));
            }
            _ => {}
        }
    }

    fn update_tray_icon(&mut self) {
        let dpi = screen::dpi();
        if dpi != self.tray_dpi
            && let Ok(icon) = tray_icon(dpi)
        {
            self.tray.set_icon(icon);
            self.tray_dpi = dpi;
        }
    }

    fn screen_changed(&mut self) {
        self.update_tray_icon();
        #[cfg(feature = "orbit")]
        match self.orbit.as_ref().map(Orbit::scale_changed) {
            Some(true) => self.rebuild_orbit(),
            Some(false) => self.with_orbit(Orbit::follow_screen),
            None => {}
        }
    }

    fn save(&self, section: &str, key: &str, value: &str) {
        if let Err(error) = Config::save_value(section, key, value) {
            self.tray.notify(
                tr(
                    "Moonlet: не удалось сохранить настройки",
                    "Moonlet: could not save the settings",
                ),
                &error.to_string(),
            );
        }
    }

    #[cfg(feature = "settings")]
    fn open_settings(&self, news: bool) {
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        unsafe {
            let _ = AllowSetForegroundWindow(ASFW_ANY);
            ShellExecuteW(
                Some(self.hwnd),
                w!("open"),
                &HSTRING::from(exe.as_os_str()),
                if news {
                    w!("--settings --news")
                } else {
                    w!("--settings")
                },
                PCWSTR::null(),
                SW_SHOWNORMAL,
            );
        }
    }

    #[cfg(not(feature = "settings"))]
    fn open_settings(&self, _news: bool) {
        config::open_in_notepad();
    }

    fn config_file_changed(&self) {
        unsafe {
            SetTimer(Some(self.hwnd), RELOAD_TIMER, RELOAD_DELAY_MS, None);
        }
    }

    fn reload_config(&mut self) {
        match Config::load() {
            Ok(config) => self.apply_config(config),
            Err(error) => self.tray.notify(
                tr("Moonlet: ошибка в настройках", "Moonlet: settings error"),
                &format!(
                    "config.toml, {error}. {}",
                    tr("Изменения не применены.", "The changes were not applied.")
                ),
            ),
        }
    }

    fn apply_config(&mut self, config: Config) {
        if config == self.config {
            return;
        }
        let old = std::mem::replace(&mut self.config, config);
        if old.autostart != self.config.autostart {
            autostart::apply(self.config.autostart);
        }
        #[cfg(feature = "orbit")]
        self.sync_orbit(&old.orbit);
        #[cfg(feature = "launcher")]
        self.sync_hotkey();
        #[cfg(feature = "update")]
        if old.update != self.config.update {
            self.schedule_update_check(FIRST_CHECK_MS);
        }
        if old.language != self.config.language {
            lang::set(self.config.language);
            #[cfg(feature = "orbit")]
            if self.orbit.is_some() {
                self.rebuild_orbit();
            }
        }
    }

    #[cfg(feature = "orbit")]
    fn sync_orbit(&mut self, old: &OrbitConfig) {
        if !self.config.orbit_enabled {
            self.orbit = None;
            return;
        }
        if self.orbit.is_none() || old.needs_rebuild(&self.config.orbit) {
            self.rebuild_orbit();
            return;
        }
        let (rx, ry) = (self.config.orbit.radius_x, self.config.orbit.radius_y);
        if let Some(orbit) = &mut self.orbit {
            orbit.set_notices(self.config.orbit.notices);
            if (old.radius_x, old.radius_y) != (rx, ry) {
                let _ = orbit.set_radii(rx, ry);
            }
        }
        self.sync_hub();
        self.update_rotation();
    }

    #[cfg(feature = "orbit")]
    fn sync_hub(&mut self) {
        #[cfg(feature = "launcher")]
        let (launcher, search) = (
            self.config.launcher_enabled,
            self.config.launcher.search.as_str(),
        );
        #[cfg(not(feature = "launcher"))]
        let (launcher, search) = (false, "");
        if let Some(orbit) = &mut self.orbit {
            orbit.set_hub(&self.config.orbit.tiles, launcher, search);
        }
    }

    #[cfg(feature = "orbit")]
    fn with_orbit(&mut self, action: impl FnOnce(&mut Orbit) -> Result<()>) {
        if let Some(orbit) = &mut self.orbit {
            let _ = action(orbit);
        }
    }

    #[cfg(feature = "orbit")]
    fn start_orbit(&mut self) {
        if let Err(error) = self.try_start_orbit() {
            self.tray.notify(orbit_failed(), &error.message());
        }
    }

    #[cfg(feature = "orbit")]
    fn try_start_orbit(&mut self) -> Result<()> {
        if !self.config.orbit_enabled || self.orbit.is_some() {
            return Ok(());
        }
        let mut orbit = Orbit::new(self.config.orbit.clone(), self.should_rotate(), self.hwnd)?;
        if self.fullscreen {
            let _ = orbit.set_hidden(true);
        }
        self.orbit = Some(orbit);
        self.sync_hub();
        Ok(())
    }

    #[cfg(feature = "orbit")]
    fn rebuild_orbit(&mut self) {
        self.orbit = None;
        self.start_orbit();
    }

    #[cfg(feature = "orbit")]
    fn device_lost_event(&self) -> Option<HANDLE> {
        self.orbit.as_ref()?.device_lost_event()
    }

    // Give the driver time to come back before creating a new device.
    #[cfg(feature = "orbit")]
    fn device_lost(&mut self) {
        self.orbit = None;
        self.restart_tries = 0;
        self.schedule_restart();
    }

    #[cfg(feature = "orbit")]
    fn schedule_restart(&self) {
        unsafe {
            SetTimer(Some(self.hwnd), RESTART_TIMER, RESTART_DELAY_MS, None);
        }
    }

    #[cfg(feature = "orbit")]
    fn restart_orbit(&mut self) {
        match self.try_start_orbit() {
            Ok(()) => {}
            Err(_) if self.restart_tries < RESTART_TRIES => {
                self.restart_tries += 1;
                self.schedule_restart();
            }
            Err(error) => self.tray.notify(orbit_failed(), &error.message()),
        }
    }

    #[cfg(feature = "orbit")]
    fn set_shape(&mut self, (radius_x, radius_y): (f32, f32)) {
        self.config.orbit.radius_x = radius_x;
        self.config.orbit.radius_y = radius_y;
        self.save("orbit", "radius_x", &config::number(radius_x));
        self.save("orbit", "radius_y", &config::number(radius_y));
        if let Some(orbit) = &mut self.orbit {
            let _ = orbit.set_radii(radius_x, radius_y);
        }
    }

    #[cfg(feature = "orbit")]
    fn set_fullscreen(&mut self, fullscreen: bool) {
        self.fullscreen = fullscreen;
        if !fullscreen {
            self.screen_changed();
        }
        if let Some(orbit) = &mut self.orbit {
            let _ = orbit.set_hidden(fullscreen);
        }
        self.update_rotation();
        #[cfg(feature = "update")]
        if !fullscreen {
            self.apply_update();
        }
    }

    #[cfg(feature = "orbit")]
    fn should_rotate(&self) -> bool {
        let battery_pause = self.config.pause_on_battery && self.on_battery;
        self.config.orbit.rotate && !battery_pause && !self.fullscreen
    }

    #[cfg(feature = "orbit")]
    fn update_rotation(&mut self) {
        let rotate = self.should_rotate();
        let setting = self.config.orbit.rotate;
        if let Some(orbit) = &mut self.orbit {
            orbit.set_rotate_setting(setting);
            let _ = orbit.set_rotating(rotate);
        }
    }
}

#[cfg(feature = "launcher")]
impl App {
    fn sync_hotkey(&mut self) {
        let wanted = (self.config.orbit_enabled && self.config.launcher_enabled)
            .then_some(self.config.launcher.hotkey);
        if wanted == self.hotkey {
            return;
        }
        unsafe {
            if self.hotkey.take().is_some() {
                let _ = UnregisterHotKey(Some(self.hwnd), HOTKEY_ID);
            }
            let Some(key) = wanted else {
                return;
            };
            let modifiers = HOT_KEY_MODIFIERS(key.modifiers) | MOD_NOREPEAT;
            if RegisterHotKey(Some(self.hwnd), HOTKEY_ID, modifiers, key.key).is_ok() {
                self.hotkey = Some(key);
            } else {
                self.tray.notify(
                    tr(
                        "Moonlet: клавиша командной строки занята",
                        "Moonlet: the command line hotkey is taken",
                    ),
                    &format!(
                        "{} {}. {}",
                        key.name(),
                        tr(
                            "уже использует другая программа",
                            "is already used by another program",
                        ),
                        tr(
                            "Выберите другую в config.toml, раздел [launcher].",
                            "Pick another one in config.toml, section [launcher].",
                        )
                    ),
                );
            }
        }
    }
}

#[cfg(feature = "update")]
impl App {
    fn update_menu(&self) -> MenuItem {
        let mode = self.config.update;
        let mut items = vec![
            MenuItem::Label(0, format!("Moonlet {}", update::current_version())),
            MenuItem::Separator,
            MenuItem::Radio(
                CMD_UPDATE_AUTO,
                tr("Обновлять автоматически", "Update automatically"),
                mode == UpdateMode::Auto,
            ),
            MenuItem::Radio(
                CMD_UPDATE_CHECK,
                tr("Только сообщать", "Only notify"),
                mode == UpdateMode::Check,
            ),
            MenuItem::Radio(
                CMD_UPDATE_OFF,
                tr("Не проверять", "Don't check"),
                mode == UpdateMode::Off,
            ),
            MenuItem::Separator,
            MenuItem::Action(CMD_UPDATE_NOW, tr("Проверить сейчас", "Check now")),
        ];
        if let Some(version) = &self.update_available {
            items.push(MenuItem::Label(
                CMD_UPDATE_INSTALL,
                format!("{} {version}", tr("Установить", "Install")),
            ));
        }
        MenuItem::Submenu(tr("Обновления", "Updates"), items)
    }

    fn set_update_mode(&mut self, mode: UpdateMode) {
        self.config.update = mode;
        self.save("update", "mode", &format!("\"{}\"", mode.name()));
        self.schedule_update_check(FIRST_CHECK_MS);
    }

    fn schedule_update_check(&self, delay_ms: u32) {
        unsafe {
            if self.config.update == UpdateMode::Off {
                let _ = KillTimer(Some(self.hwnd), UPDATE_TIMER);
            } else {
                SetTimer(Some(self.hwnd), UPDATE_TIMER, delay_ms, None);
            }
        }
    }

    fn update_timer(&mut self) {
        self.schedule_update_check(DAY_MS);
        if self.config.update != UpdateMode::Off {
            self.updater.start(self.config.update == UpdateMode::Auto);
        }
    }

    fn check_updates(&mut self, install: bool) {
        self.update_manual = true;
        self.update_install |= install;
        self.updater
            .start(install || self.config.update == UpdateMode::Auto);
    }

    fn balloon_clicked(&mut self) {
        if self.update_available.is_some() {
            self.check_updates(true);
        } else if std::mem::take(&mut self.just_updated) {
            self.open_settings(true);
        }
    }

    fn update_finished(&mut self) {
        let Some(outcome) = self.updater.take() else {
            return;
        };
        let manual = std::mem::take(&mut self.update_manual);
        match outcome {
            Outcome::UpToDate => {
                self.update_available = None;
                if manual {
                    self.tray.inform(
                        "Moonlet",
                        &format!(
                            "{} {}",
                            tr(
                                "Установлена последняя версия:",
                                "You have the latest version:"
                            ),
                            update::current_version()
                        ),
                    );
                }
            }
            Outcome::Available(version) => {
                if manual || self.update_available.as_ref() != Some(&version) {
                    self.tray.inform(
                        &format!("{} Moonlet {version}", tr("Вышел", "New version:")),
                        tr("Нажмите, чтобы установить.", "Click to install it."),
                    );
                }
                self.update_available = Some(version);
            }
            Outcome::Ready { staged } => {
                self.update_ready = Some(staged);
                self.apply_update();
            }
            Outcome::Failed(reason) => {
                self.update_install = false;
                if manual {
                    self.tray.notify(
                        tr("Moonlet: обновление не удалось", "Moonlet: update failed"),
                        reason,
                    );
                }
            }
        }
    }

    // Never restart in the middle of a full-screen game.
    fn apply_update(&mut self) {
        #[cfg(feature = "orbit")]
        if self.fullscreen {
            return;
        }
        let Some(staged) = self.update_ready.take() else {
            return;
        };
        self.update_install = false;
        match update::install(&staged) {
            Ok(()) => unsafe { PostQuitMessage(0) },
            Err(reason) => self.tray.notify(
                tr("Moonlet: обновление не удалось", "Moonlet: update failed"),
                reason,
            ),
        }
    }
}

#[cfg(feature = "orbit")]
fn open_screen_snip() {
    unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            w!("ms-screenclip:"),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}
