use std::cell::RefCell;
use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, POINT, RECT};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, CHILDID_SELF, DispatchMessageW, EVENT_OBJECT_NAMECHANGE,
    EVENT_OBJECT_SHOW, GA_ROOT, GetAncestor, GetMessageW, GetWindowPlacement, IsHungAppWindow,
    IsWindow, IsWindowVisible, KillTimer, MSG, OBJID_WINDOW, SW_SHOWMAXIMIZED, SW_SHOWMINIMIZED,
    SW_SHOWMINNOACTIVE, SW_SHOWNOACTIVATE, SW_SHOWNORMAL, SetTimer, SetWindowPlacement,
    TranslateMessage, WINDOWPLACEMENT, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_TIMER,
    WPF_ASYNCWINDOWPLACEMENT,
};
use windows::core::{HSTRING, PCWSTR, w};

use super::capture::{Open, open_windows};
use super::store::{Scenario, State, Window};
use super::{Monitor, Place, monitors, windows_text};
use crate::foreign::{is_browser, is_firefox_like, pid_of};
use crate::lang::tr;

const DEADLINE_MS: u32 = 30_000;
const SCAN_MS: u32 = 150;
const SETTLE_MS: u64 = 1500;
const MAX_TRIES: u8 = 3;
const SLACK: i32 = 8;

struct Target {
    window: Window,
    rect: [i32; 4],
    monitor: usize,
    hwnd: Option<HWND>,
    waiting: bool,
    placed_at: Option<Instant>,
    tries: u8,
    done: bool,
}

#[derive(Default)]
struct Tally {
    placed: usize,
    launched: usize,
    admin: usize,
    missing: usize,
}

struct Job {
    targets: Vec<Target>,
    monitors: Vec<Monitor>,
    before: HashSet<isize>,
    tally: Tally,
    scan_timer: usize,
    settle_timer: usize,
}

thread_local! {
    static JOB: RefCell<Option<Job>> = const { RefCell::new(None) };
    static DIRTY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn map_rect(rect: [i32; 4], from: [i32; 4], to: [i32; 4]) -> [i32; 4] {
    let scale = |value: i32, a0: i32, a1: i32, b0: i32, b1: i32| -> i32 {
        let (span_a, span_b) = ((a1 - a0).max(1) as i64, (b1 - b0) as i64);
        let offset = (value - a0) as i64 * span_b;
        b0 + ((offset + offset.signum() * span_a / 2) / span_a) as i32
    };
    [
        scale(rect[0], from[0], from[2], to[0], to[2]),
        scale(rect[1], from[1], from[3], to[1], to[3]),
        scale(rect[2], from[0], from[2], to[0], to[2]),
        scale(rect[3], from[1], from[3], to[1], to[3]),
    ]
}

pub fn target_monitor(
    scenario: &Scenario,
    window: &Window,
    monitors: &[Monitor],
    place: Place,
) -> usize {
    let primary = monitors.iter().position(|m| m.primary).unwrap_or(0);
    match place {
        Place::Screen(index) if index < monitors.len() => index,
        _ => scenario
            .screens
            .get(window.screen)
            .and_then(|s| monitors.iter().position(|m| m.device == s.device))
            .unwrap_or(primary),
    }
}

fn same(window: &Window, open: &Open, folders: bool) -> bool {
    if window.exe_name() != open.exe_name {
        return false;
    }
    if !window.app.is_empty() || !open.app.is_empty() {
        return window.app.eq_ignore_ascii_case(&open.app);
    }
    !folders
        || window.exe_name() != "explorer.exe"
        || window.folder.eq_ignore_ascii_case(&open.folder)
}

pub fn assign(windows: &[Window], open: &[Open], folders: bool) -> Vec<Option<usize>> {
    let mut taken = vec![false; open.len()];
    let mut out = vec![None; windows.len()];
    for exact in [true, false] {
        for (index, window) in windows.iter().enumerate() {
            if out[index].is_some() {
                continue;
            }
            let found = open.iter().enumerate().position(|(i, o)| {
                !taken[i] && same(window, o, folders) && (!exact || o.title == window.title)
            });
            if let Some(i) = found {
                taken[i] = true;
                out[index] = Some(i);
            }
        }
    }
    out
}

fn elevated(pid: u32) -> bool {
    fn token_elevated(process: HANDLE) -> Option<bool> {
        unsafe {
            let mut token = HANDLE::default();
            OpenProcessToken(process, TOKEN_QUERY, &mut token).ok()?;
            let mut info = TOKEN_ELEVATION::default();
            let mut size = 0;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                Some((&mut info as *mut TOKEN_ELEVATION).cast()),
                size_of::<TOKEN_ELEVATION>() as u32,
                &mut size,
            );
            let _ = CloseHandle(token);
            ok.ok()?;
            Some(info.TokenIsElevated != 0)
        }
    }
    if token_elevated(unsafe { GetCurrentProcess() }) == Some(true) {
        return false;
    }
    unsafe {
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return true;
        };
        let answer = token_elevated(process).unwrap_or(true);
        let _ = CloseHandle(process);
        answer
    }
}

fn to_workspace(rect: [i32; 4], monitor: &Monitor) -> RECT {
    let dx = monitor.work[0] - monitor.rect[0];
    let dy = monitor.work[1] - monitor.rect[1];
    RECT {
        left: rect[0] - dx,
        top: rect[1] - dy,
        right: rect[2] - dx,
        bottom: rect[3] - dy,
    }
}

fn show_command(state: State) -> u32 {
    match state {
        State::Normal => SW_SHOWNOACTIVATE.0 as u32,
        State::Maximized => SW_SHOWMAXIMIZED.0 as u32,
        State::Minimized => SW_SHOWMINNOACTIVE.0 as u32,
    }
}

fn place(hwnd: HWND, target: &Target, monitor: &Monitor) -> bool {
    unsafe {
        if IsHungAppWindow(hwnd).as_bool() {
            return false;
        }
        let wp = WINDOWPLACEMENT {
            length: size_of::<WINDOWPLACEMENT>() as u32,
            flags: WPF_ASYNCWINDOWPLACEMENT,
            showCmd: show_command(target.window.state),
            ptMinPosition: POINT { x: -1, y: -1 },
            ptMaxPosition: POINT { x: -1, y: -1 },
            rcNormalPosition: to_workspace(target.rect, monitor),
        };
        SetWindowPlacement(hwnd, &wp).is_ok()
    }
}

fn in_place(hwnd: HWND, target: &Target, monitor: &Monitor) -> bool {
    let mut wp = WINDOWPLACEMENT {
        length: size_of::<WINDOWPLACEMENT>() as u32,
        ..Default::default()
    };
    if unsafe { GetWindowPlacement(hwnd, &mut wp) }.is_err() {
        return true;
    }
    let state = match wp.showCmd {
        cmd if cmd == SW_SHOWMAXIMIZED.0 as u32 => State::Maximized,
        cmd if cmd == SW_SHOWMINIMIZED.0 as u32 => State::Minimized,
        _ => State::Normal,
    };
    let want = to_workspace(target.rect, monitor);
    let have = wp.rcNormalPosition;
    let close = |a: i32, b: i32| (a - b).abs() <= SLACK;
    state == target.window.state
        && close(want.left, have.left)
        && close(want.top, have.top)
        && close(want.right, have.right)
        && close(want.bottom, have.bottom)
}

fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "%22"))
}

fn launch(window: &Window) -> bool {
    let exe_name = window.exe_name();
    let (target, arguments): (String, Option<String>) = if !window.app.is_empty() {
        (format!("shell:AppsFolder\\{}", window.app), None)
    } else if exe_name == "explorer.exe" {
        let folder = Some(window.folder.as_str())
            .filter(|f| f.starts_with("::") || (!f.is_empty() && Path::new(f).exists()))
            .map(quote);
        ("explorer.exe".to_string(), folder)
    } else if !Path::new(&window.exe).exists() {
        return false;
    } else if is_browser(&exe_name) {
        let flag = if is_firefox_like(&exe_name) {
            "-new-window"
        } else {
            "--new-window"
        };
        let mut text = flag.to_string();
        for url in &window.urls {
            text.push(' ');
            text += &quote(url);
        }
        (window.exe.clone(), Some(text))
    } else {
        (window.exe.clone(), None)
    };
    let directory = Path::new(&window.exe)
        .parent()
        .filter(|_| window.app.is_empty() && exe_name != "explorer.exe")
        .map(|p| HSTRING::from(p.as_os_str()));
    let arguments = arguments.map(|a| HSTRING::from(a.as_str()));
    let result = unsafe {
        let _ = AllowSetForegroundWindow(u32::MAX);
        ShellExecuteW(
            None,
            w!("open"),
            &HSTRING::from(target.as_str()),
            arguments
                .as_ref()
                .map_or(PCWSTR::null(), |a| PCWSTR(a.as_ptr())),
            directory
                .as_ref()
                .map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr())),
            SW_SHOWNORMAL,
        )
    };
    result.0 as isize > 32
}

fn placed(job: &mut Job, index: usize, hwnd: HWND) {
    let monitor = job.monitors[job.targets[index].monitor].clone();
    let target = &mut job.targets[index];
    target.hwnd = Some(hwnd);
    target.waiting = false;
    if elevated(pid_of(hwnd)) {
        target.done = true;
        job.tally.admin += 1;
        return;
    }
    if place(hwnd, target, &monitor) {
        target.placed_at = Some(Instant::now());
        target.tries = 1;
    } else {
        target.done = true;
    }
}

fn finished(job: &Job) -> bool {
    job.targets.iter().all(|t| t.done)
}

fn scan(job: &mut Job) {
    if !job.targets.iter().any(|t| t.waiting) {
        return;
    }
    let claimed: HashSet<isize> = job
        .targets
        .iter()
        .filter_map(|t| t.hwnd.map(|h| h.0 as isize))
        .collect();
    let fresh: Vec<Open> = open_windows(false)
        .into_iter()
        .filter(|o| {
            let key = o.hwnd.0 as isize;
            !job.before.contains(&key) && !claimed.contains(&key)
        })
        .collect();
    let mut used = vec![false; fresh.len()];
    for index in 0..job.targets.len() {
        if !job.targets[index].waiting {
            continue;
        }
        let window = &job.targets[index].window;
        let found = fresh
            .iter()
            .enumerate()
            .position(|(i, o)| !used[i] && same(window, o, false));
        if let Some(i) = found {
            used[i] = true;
            placed(job, index, fresh[i].hwnd);
        }
    }
}

fn settle(job: &mut Job) {
    let now = Instant::now();
    for index in 0..job.targets.len() {
        let monitor = job.monitors[job.targets[index].monitor].clone();
        let target = &mut job.targets[index];
        let (Some(hwnd), Some(at)) = (target.hwnd, target.placed_at) else {
            continue;
        };
        if target.done || now.duration_since(at) < Duration::from_millis(SETTLE_MS - 300) {
            continue;
        }
        let gone = unsafe { !IsWindow(Some(hwnd)).as_bool() || !IsWindowVisible(hwnd).as_bool() };
        if gone {
            target.hwnd = None;
            target.placed_at = None;
            target.waiting = true;
            continue;
        }
        if in_place(hwnd, target, &monitor) || target.tries >= MAX_TRIES {
            target.done = true;
            job.tally.placed += 1;
            continue;
        }
        if place(hwnd, target, &monitor) {
            target.placed_at = Some(now);
            target.tries += 1;
        } else {
            target.done = true;
        }
    }
}

fn arm_settle(job: &mut Job) {
    let pending = job.targets.iter().any(|t| !t.done && t.placed_at.is_some());
    unsafe {
        if job.settle_timer != 0 {
            let _ = KillTimer(None, job.settle_timer);
            job.settle_timer = 0;
        }
        if pending {
            job.settle_timer = SetTimer(None, 0, SETTLE_MS as u32, None);
        }
    }
}

unsafe extern "system" fn on_event(
    _: HWINEVENTHOOK,
    _: u32,
    hwnd: HWND,
    object: i32,
    child: i32,
    _: u32,
    _: u32,
) {
    if object != OBJID_WINDOW.0 || child != CHILDID_SELF as i32 || hwnd.is_invalid() {
        return;
    }
    if unsafe { GetAncestor(hwnd, GA_ROOT) } != hwnd {
        return;
    }
    let armed = JOB.with(|cell| {
        let Ok(mut job) = cell.try_borrow_mut() else {
            return false;
        };
        let Some(job) = job.as_mut() else {
            return true;
        };
        unsafe {
            if job.scan_timer != 0 {
                let _ = KillTimer(None, job.scan_timer);
            }
            job.scan_timer = SetTimer(None, 0, SCAN_MS, None);
        }
        true
    });
    if !armed {
        DIRTY.with(|d| d.set(true));
    }
}

fn summary(name: &str, tally: &Tally) -> String {
    let mut text = format!("«{name}»: {}", windows_text(tally.placed));
    text += tr(" на местах", " in place");
    if tally.launched > 0 {
        text += &format!(", {} {}", tr("запущено", "started"), tally.launched);
    }
    if tally.admin > 0 {
        text += &format!(
            ", {} {}",
            tr(
                "от администратора, не сдвинуть:",
                "run as admin, can't move:"
            ),
            tally.admin
        );
    }
    if tally.missing > 0 {
        text += &format!(
            ", {} {}",
            tr("не открылось:", "didn't open:"),
            tally.missing
        );
    }
    text
}

pub fn run(scenario: &Scenario, where_to: Place) -> String {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
    }
    let text = run_inner(scenario, where_to);
    unsafe {
        CoUninitialize();
    }
    text
}

fn run_inner(scenario: &Scenario, where_to: Place) -> String {
    let monitors = monitors();
    if monitors.is_empty() {
        return format!("«{}»: {}", scenario.name, tr("нет экрана", "no screen"));
    }
    let primary = monitors.iter().position(|m| m.primary).unwrap_or(0);
    let targets: Vec<Target> = scenario
        .windows
        .iter()
        .map(|window| {
            let monitor = target_monitor(scenario, window, &monitors, where_to);
            let from = scenario
                .screens
                .get(window.screen)
                .map_or(monitors[primary].work, |s| s.work);
            Target {
                rect: map_rect(window.rect, from, monitors[monitor].work),
                monitor,
                window: window.clone(),
                hwnd: None,
                waiting: false,
                placed_at: None,
                tries: 0,
                done: false,
            }
        })
        .collect();

    let open = open_windows(true);
    let before: HashSet<isize> = open.iter().map(|o| o.hwnd.0 as isize).collect();
    let assigned = assign(&scenario.windows, &open, true);
    let mut job = Job {
        targets,
        monitors,
        before,
        tally: Tally::default(),
        scan_timer: 0,
        settle_timer: 0,
    };
    for (index, found) in assigned.iter().enumerate() {
        if let Some(i) = found {
            placed(&mut job, index, open[*i].hwnd);
        }
    }
    for (index, found) in assigned.iter().enumerate() {
        if found.is_some() {
            continue;
        }
        let target = &mut job.targets[index];
        if launch(&target.window) {
            target.waiting = true;
            job.tally.launched += 1;
        } else {
            target.done = true;
            job.tally.missing += 1;
        }
    }

    let hooks: Vec<HWINEVENTHOOK> = if job.targets.iter().any(|t| t.waiting) {
        [EVENT_OBJECT_SHOW, EVENT_OBJECT_NAMECHANGE]
            .into_iter()
            .map(|event| unsafe {
                SetWinEventHook(
                    event,
                    event,
                    None,
                    Some(on_event),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
                )
            })
            .filter(|h| !h.is_invalid())
            .collect()
    } else {
        Vec::new()
    };
    let deadline = unsafe { SetTimer(None, 0, DEADLINE_MS, None) };
    arm_settle(&mut job);
    job.scan_timer = unsafe { SetTimer(None, 0, SCAN_MS, None) };
    JOB.with(|cell| *cell.borrow_mut() = Some(job));

    let mut message = MSG::default();
    loop {
        let done = JOB.with(|cell| cell.borrow().as_ref().is_none_or(finished));
        if done {
            break;
        }
        if !unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {
            break;
        }
        if message.message == WM_TIMER && message.hwnd.is_invalid() {
            let id = message.wParam.0;
            if id == deadline {
                break;
            }
            JOB.with(|cell| {
                let mut slot = cell.borrow_mut();
                let Some(job) = slot.as_mut() else {
                    return;
                };
                if id == job.scan_timer || DIRTY.with(|d| d.replace(false)) {
                    unsafe {
                        let _ = KillTimer(None, job.scan_timer);
                    }
                    job.scan_timer = 0;
                    scan(job);
                }
                if id == job.settle_timer {
                    settle(job);
                }
                arm_settle(job);
            });
            continue;
        }
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }

    for hook in hooks {
        unsafe {
            let _ = UnhookWinEvent(hook);
        }
    }
    unsafe {
        let _ = KillTimer(None, deadline);
    }
    let Some(mut job) = JOB.with(|cell| cell.borrow_mut().take()) else {
        return String::new();
    };
    unsafe {
        let _ = KillTimer(None, job.scan_timer);
        let _ = KillTimer(None, job.settle_timer);
    }
    for target in &job.targets {
        if target.done {
            continue;
        }
        if target.placed_at.is_some() {
            job.tally.placed += 1;
        } else if target.waiting {
            job.tally.missing += 1;
        }
    }
    summary(&scenario.name, &job.tally)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::store::Screen;

    fn window(exe: &str, title: &str) -> Window {
        Window {
            screen: 0,
            state: State::Normal,
            rect: [0, 0, 100, 100],
            exe: format!("C:\\Apps\\{exe}"),
            title: title.into(),
            app: String::new(),
            folder: String::new(),
            urls: Vec::new(),
        }
    }

    fn opened(exe: &str, title: &str) -> Open {
        Open {
            hwnd: HWND::default(),
            exe: format!("D:\\Other\\{exe}"),
            exe_name: exe.to_lowercase(),
            title: title.into(),
            app: String::new(),
            folder: String::new(),
        }
    }

    #[test]
    fn maps_windows_between_screens() {
        let from = [0, 0, 1920, 1040];
        assert_eq!(
            map_rect([10, 20, 960, 1040], from, from),
            [10, 20, 960, 1040]
        );
        let to = [1920, 0, 5760, 2080];
        assert_eq!(map_rect([0, 0, 960, 1040], from, to), [1920, 0, 3840, 2080]);
        assert_eq!(map_rect([-5, 0, 100, 100], from, from)[0], -5);
    }

    #[test]
    fn picks_screens() {
        let monitor = |device: &str, left, primary| Monitor {
            device: device.into(),
            rect: [left, 0, left + 1920, 1080],
            work: [left, 0, left + 1920, 1040],
            dpi: 96,
            primary,
        };
        let monitors = [monitor("A", 0, true), monitor("B", 1920, false)];
        let scenario = Scenario {
            name: "x".into(),
            screens: vec![
                Screen {
                    device: "B".into(),
                    work: [0, 0, 1, 1],
                    dpi: 96,
                },
                Screen {
                    device: "C".into(),
                    work: [0, 0, 1, 1],
                    dpi: 96,
                },
            ],
            windows: Vec::new(),
        };
        let mut w = window("a.exe", "");
        assert_eq!(target_monitor(&scenario, &w, &monitors, Place::AsSaved), 1);
        w.screen = 1;
        assert_eq!(target_monitor(&scenario, &w, &monitors, Place::AsSaved), 0);
        assert_eq!(
            target_monitor(&scenario, &w, &monitors, Place::Screen(1)),
            1
        );
    }

    #[test]
    fn matches_open_windows() {
        let windows = [
            window("chrome.exe", "Почта"),
            window("chrome.exe", "GitHub"),
            window("telegram.exe", "Telegram"),
            window("code.exe", "x"),
        ];
        let open = [
            opened("Chrome.exe", "GitHub"),
            opened("chrome.exe", "YouTube"),
            opened("telegram.exe", "Telegram (3)"),
        ];
        assert_eq!(
            assign(&windows, &open, true),
            [Some(1), Some(0), Some(2), None]
        );
        let mut folder = window("explorer.exe", "Документы");
        folder.folder = "C:\\Docs".into();
        let mut other = opened("explorer.exe", "Загрузки");
        other.folder = "C:\\Downloads".into();
        assert_eq!(assign(&[folder.clone()], &[other.clone()], true), [None]);
        assert_eq!(assign(&[folder], &[other], false), [Some(0)]);
        let mut calc = window("ApplicationFrameHost.exe", "Калькулятор");
        calc.app = "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App".into();
        let mut settings = opened("applicationframehost.exe", "Параметры");
        settings.app =
            "windows.immersivecontrolpanel_cw5n1h2txyewy!microsoft.windows.immersivecontrolpanel"
                .into();
        assert_eq!(assign(&[calc], &[settings], true), [None]);
    }
}

// cargo test --release -- --ignored explorer_special_folder
#[cfg(test)]
mod live {
    use super::*;
    use crate::scenarios::store::{self, Screen};

    #[test]
    #[ignore]
    fn explorer_special_folder() {
        let dir = std::env::temp_dir().join("moonlet-scenario-test");
        unsafe { std::env::set_var("APPDATA", &dir) };
        let count = crate::scenarios::capture::capture("снимок").unwrap();
        let saved = store::find("снимок").unwrap();
        for w in &saved.windows {
            println!("{} | {} | {}", w.exe_name(), w.title, w.folder);
        }
        assert_eq!(saved.windows.len(), count);
        let monitor = &monitors()[0];
        let scenario = Scenario {
            name: "пк".into(),
            screens: vec![Screen {
                device: monitor.device.clone(),
                work: monitor.work,
                dpi: monitor.dpi,
            }],
            windows: vec![Window {
                screen: 0,
                state: State::Normal,
                rect: [200, 150, 1000, 700],
                exe: r"C:\Windows\explorer.exe".into(),
                title: String::new(),
                app: String::new(),
                folder: "::{20D04FE0-3AEA-1069-A2D8-08002B30309D}".into(),
                urls: Vec::new(),
            }],
        };
        println!("{}", run(&scenario, Place::AsSaved));
    }
}
