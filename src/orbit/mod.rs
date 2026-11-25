// The orbit: a core at the top of the screen with satellites around it.
// All motion is DirectComposition animation evaluated by DWM; the process
// only wakes up to redraw when the data changes.

#[cfg(feature = "bluetooth")]
mod bluetooth;
mod info;
#[cfg(feature = "launcher")]
mod launcher;
#[cfg(feature = "music")]
mod media;
mod motion;
mod scene;
mod shell;
#[cfg(feature = "timers")]
mod timers;
#[cfg(feature = "music")]
mod volume;

use std::f64::consts::{PI, TAU};

use windows::Win32::Foundation::{COLORREF, HANDLE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::DirectComposition::{
    IDCompositionAnimation, IDCompositionTarget, IDCompositionVisual2, IDCompositionVisual3,
};
use windows::Win32::Graphics::Gdi::{
    CombineRgn, CreateEllipticRgn, CreateRectRgn, DeleteObject, RGN_OR, SetWindowRgn,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetLastInputInfo, LASTINPUTINFO, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, HTTRANSPARENT, HWND_TOPMOST, IDC_ARROW, KillTimer, LWA_ALPHA,
    LoadCursorW, MA_NOACTIVATE, RegisterClassW, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE,
    SetLayeredWindowAttributes, SetTimer, SetWindowPos, ShowWindow, WINDOW_EX_STYLE, WM_DPICHANGED,
    WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_NCHITTEST, WM_TIMER, WNDCLASSW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{Interface, PCWSTR, Result, w};

use crate::app::{
    CMD_OPEN_CONFIG, CMD_ROTATION, CMD_SCREENSHOT, CMD_SHAPE_TOGGLE, WM_SCREEN_CHANGED,
};
use crate::config::{NoticeConfig, OrbitConfig, SatelliteKind};
use crate::gfx::Gfx;
use crate::lang::tr;
use crate::screen;
use crate::tile::{self, Builtin, Kind, Tile};
use motion::{Channel, Clock, Ellipse, Tail, Target, Vec2, Wave, now};
use scene::{CardView, Core, HubTile, HubView, Labels, NoticeView, Satellite, TileIcon};
#[cfg(feature = "music")]
use windows::Win32::Graphics::Imaging::IWICBitmap;
#[cfg(feature = "music")]
use windows::Win32::UI::WindowsAndMessaging::WM_MOUSEWHEEL;
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, PostMessageW, WM_COMMAND, WindowFromPoint,
};

const STARTUP_SECS: f64 = 0.9;
const STARTUP_STAGGER_SECS: f64 = 0.12;
const SPREAD_SECS: f64 = 0.26;
const RETURN_SECS: f64 = 0.45;
const STOP_SECS: f64 = 0.6;
const RESHAPE_SECS: f64 = 0.6;
const CARD_SHOW_SECS: f64 = 0.18;
const CARD_HIDE_SECS: f64 = 0.12;
const FADE_SECS: f64 = 0.3;
const NOTICE_IN_SECS: f64 = 0.45;
const NOTICE_OUT_SECS: f64 = 0.35;
const NOTICE_PULSE_SECS: f64 = 1.6;
const SPREAD_ARC_DEG: f64 = 140.0;

const DEPTH_SCALE: f64 = 0.22;
const DEPTH_ALPHA_MID: f64 = 0.8;
const DEPTH_ALPHA: f64 = 0.2;
const HOVER_SCALE: f64 = 1.3;
const GLOW_RATIO: f64 = 2.4;
const HALO_RATIO: f64 = 2.0;
const BREATH_SECS: f64 = 5.0;
const TWINKLE_SECS: [f64; 6] = [2.6, 3.3, 4.1, 2.9, 3.7, 4.6];

const WM_MOUSELEAVE: u32 = 0x02A3;
const HOVER_TIMER: usize = 1;
const HIDE_TIMER: usize = 2;
const CLOCK_TIMER: usize = 3;
#[cfg(feature = "music")]
const MUSIC_TIMER: usize = 4;
const NOTICE_TIMER: usize = 5;
const NOTICE_GONE_TIMER: usize = 6;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_RBUTTONUP: u32 = 0x0205;
const HOLD_TIMER: usize = 8;
const HOLD_MS: u32 = 500;

#[derive(Clone, Copy)]
enum MusicKey {
    PlayPause,
    Next,
    Previous,
}

const RENDER_CLASS: PCWSTR = w!("MoonletOrbit");
const INPUT_CLASS: PCWSTR = w!("MoonletOrbitInput");

pub struct Orbit {
    config: OrbitConfig,
    render_hwnd: HWND,
    input_hwnd: HWND,
    _target: IDCompositionTarget,
    core: Core,
    satellites: Vec<Satellite>,
    labels: Option<Labels>,
    card: Option<CardView>,
    #[cfg(feature = "music")]
    music: Option<Music>,
    #[cfg(feature = "music")]
    volume: Option<volume::Volume>,
    hub: Option<HubView>,
    pressed: Option<usize>,
    held: bool,
    #[cfg(feature = "bluetooth")]
    bluetooth: Option<bluetooth::Bluetooth>,
    notice: Option<NoticeView>,
    #[cfg(feature = "timers")]
    sat_layer: IDCompositionVisual2,
    #[cfg(feature = "timers")]
    timers: Vec<timers::TimerSat>,
    #[cfg(feature = "timers")]
    timer_total: usize,
    #[cfg(feature = "timers")]
    timer_labels: Option<Labels>,
    #[cfg(feature = "timers")]
    pending_notice: Option<info::Notice>,
    #[cfg(feature = "launcher")]
    launcher: Option<launcher::Launcher>,
    #[cfg(feature = "launcher")]
    launcher_view: Option<launcher::view::View>,
    #[cfg(feature = "launcher")]
    launcher_on: bool,
    #[cfg(feature = "launcher")]
    search: String,
    low_warned: Option<u8>,
    host: HWND,
    labels_holder: IDCompositionVisual2,
    overlay: IDCompositionVisual2,
    geo: Geometry,
    clock: Clock,
    expanded: bool,
    hover_armed: bool,
    root: IDCompositionVisual3,
    fade: Channel,
    hidden: bool,
    gfx: Gfx,
}

struct Goal {
    x: Target,
    y: Target,
    size: Target,
    alpha: Target,
}

#[derive(Clone, Copy)]
pub struct Geometry {
    scale: f64,
    window: RECT,
    center: Vec2,
    orbit: Ellipse,
    core_radius: f64,
    satellite_radius: f64,
    glow_radius: f64,
    ring_radius: f64,
    ring_width: f64,
    halo_radius: f64,
    layout_radius: f64,
    hover_radius: f64,
    label_height: f64,
    label_gap: f64,
    card_width: f64,
    card_height: f64,
    card_gap: f64,
    margin: f64,
    core_size: u32,
    satellite_size: u32,
    glow_size: u32,
    ring_size: u32,
    halo_size: u32,
}

impl Geometry {
    fn new(config: &OrbitConfig) -> Self {
        let scale = screen::dpi() as f64 / 96.0;
        let px = |value: f32| value as f64 * scale;
        let (rx, ry) = (px(config.radius_x), px(config.radius_y));
        let core_radius = px(config.core_radius);
        let satellite_radius = px(config.satellite_radius);
        let glow_radius = satellite_radius * GLOW_RATIO;
        let ring_radius = satellite_radius + 3.2 * scale;
        let ring_width = 1.6 * scale;
        let halo_radius = core_radius * HALO_RATIO;
        let layout_radius = px(config.layout_radius);
        let hover_radius = core_radius + px(config.hover_padding);
        let label_height = (18.0 * scale).round();
        let label_gap = 4.0 * scale;
        let card_width = (232.0 * scale).round();
        let card_height = (124.0 * scale).round();
        let card_gap = 6.0 * scale;
        let margin = (4.0 * scale).ceil();
        let max_scale = HOVER_SCALE.max(1.0 + DEPTH_SCALE);
        let reach = glow_radius.max(ring_radius + ring_width) * max_scale;
        let slot_reach = layout_radius * (SPREAD_ARC_DEG.to_radians() / 2.0).sin();

        let (max_rx, max_ry) = [
            OrbitConfig::CIRCLE,
            OrbitConfig::OVAL,
            (config.radius_x, config.radius_y),
        ]
        .iter()
        .fold((0.0f64, 0.0f64), |(a, b), &(x, y)| {
            (a.max(px(x)), b.max(px(y)))
        });
        let hub_width = (2.0 * 8.0 + 4.0 * 72.0 + 3.0 * 4.0) * scale;
        let hub_height = (2.0 * 8.0 + 4.0 * 64.0 + 3.0 * 4.0) * scale;
        let half_width = ((max_rx.max(layout_radius) + reach)
            .max(slot_reach + card_width / 2.0)
            .max(hub_width / 2.0)
            + margin)
            .ceil();
        let lowest_center = margin + (max_ry + satellite_radius).max(core_radius);
        let below_slot =
            ring_radius * HOVER_SCALE + label_gap + label_height + card_gap + card_height;
        let height =
            (lowest_center + (max_ry + reach).max(layout_radius + below_slot) + margin).ceil();
        let hub_bottom = lowest_center + hover_radius + 6.0 * scale + hub_height + margin;
        let height = height.max(hub_bottom.ceil());

        #[cfg(feature = "launcher")]
        let (half_width, height) = {
            let (panel_width, panel_height) = launcher::view::extent(scale);
            let top = lowest_center + hover_radius + launcher::view::GAP * scale;
            (
                half_width.max((panel_width / 2.0 + margin).ceil()),
                height.max((top + panel_height + margin).ceil()),
            )
        };
        #[cfg(feature = "timers")]
        let half_width =
            half_width.max((layout_radius + timers::ROW_REACH * scale + margin).ceil());
        let monitor = screen::rect();
        let width = 2 * half_width as i32;
        let left = monitor.left + (monitor.right - monitor.left - width) / 2;
        let window = RECT {
            left,
            top: monitor.top,
            right: left + width,
            bottom: monitor.top + height as i32,
        };

        let center = Vec2::new(
            half_width,
            (margin + (ry + satellite_radius).max(core_radius)).ceil(),
        );
        let orbit = Ellipse {
            center,
            rx,
            ry,
            dir: if config.clockwise { 1.0 } else { -1.0 },
        };

        let even = |radius: f64| 2 * (radius + 2.0).ceil() as u32;
        Self {
            scale,
            window,
            center,
            orbit,
            core_radius,
            satellite_radius,
            glow_radius,
            ring_radius,
            ring_width,
            halo_radius,
            layout_radius,
            hover_radius,
            label_height,
            label_gap,
            card_width,
            card_height,
            card_gap,
            margin,
            core_size: even(core_radius),
            satellite_size: even(satellite_radius),
            glow_size: even(glow_radius),
            ring_size: even(ring_radius + ring_width),
            halo_size: even(halo_radius),
        }
    }

    fn width(&self) -> f64 {
        (self.window.right - self.window.left) as f64
    }
}

impl Orbit {
    pub fn new(config: OrbitConfig, rotating: bool, host: HWND) -> Result<Self> {
        let geo = Geometry::new(&config);
        let (render_hwnd, input_hwnd) = create_windows(&geo)?;
        let gfx = Gfx::new()?;

        unsafe {
            let dcomp = &gfx.dcomp;
            let target = dcomp.CreateTargetForHwnd(render_hwnd, true)?;
            let root = dcomp.CreateVisual()?;
            target.SetRoot(&root)?;

            let halo_holder = dcomp.CreateVisual()?;
            let layer = dcomp.CreateVisual()?;
            let disk_holder = dcomp.CreateVisual()?;
            let labels_holder = dcomp.CreateVisual()?;
            let overlay = dcomp.CreateVisual()?;
            root.AddVisual(&halo_holder, false, None)?;
            root.AddVisual(&layer, true, &halo_holder)?;
            root.AddVisual(&disk_holder, true, &layer)?;
            root.AddVisual(&labels_holder, true, &disk_holder)?;
            root.AddVisual(&overlay, true, &labels_holder)?;

            let core = scene::build_core(&gfx, &geo, halo_holder, disk_holder)?;

            let kinds: Vec<SatelliteKind> = config
                .satellites
                .iter()
                .copied()
                .filter(|&kind| match kind {
                    SatelliteKind::Battery => info::battery_present(),
                    SatelliteKind::Music => cfg!(feature = "music"),
                    SatelliteKind::Clock => true,
                })
                .collect();
            let mut satellites = Vec::with_capacity(kinds.len());
            for (index, &kind) in kinds.iter().enumerate() {
                let phase = TAU * index as f64 / kinds.len() as f64;
                satellites.push(scene::build_satellite(&gfx, &geo, &layer, kind, phase)?);
            }

            let t = now();
            let speed = TAU / config.period_secs.max(1.0) as f64;
            let mut orbit = Self {
                config,
                render_hwnd,
                input_hwnd,
                _target: target,
                core,
                satellites,
                labels: None,
                card: None,
                labels_holder,
                overlay,
                geo,
                clock: Clock::new(speed, rotating, t),
                expanded: false,
                hover_armed: false,
                root: root.cast()?,
                fade: Channel::fixed(1.0),
                hidden: false,
                #[cfg(feature = "music")]
                music: None,
                #[cfg(feature = "music")]
                volume: None,
                hub: None,
                pressed: None,
                held: false,
                #[cfg(feature = "bluetooth")]
                bluetooth: None,
                notice: None,
                #[cfg(feature = "timers")]
                sat_layer: layer,
                #[cfg(feature = "timers")]
                timers: Vec::new(),
                #[cfg(feature = "timers")]
                timer_total: 0,
                #[cfg(feature = "timers")]
                timer_labels: None,
                #[cfg(feature = "timers")]
                pending_notice: None,
                #[cfg(feature = "launcher")]
                launcher: None,
                #[cfg(feature = "launcher")]
                launcher_view: None,
                #[cfg(feature = "launcher")]
                launcher_on: false,
                #[cfg(feature = "launcher")]
                search: String::new(),
                low_warned: None,
                host,
                gfx,
            };

            orbit.retarget(t, STARTUP_SECS, false, STARTUP_STAGGER_SECS);
            orbit.apply_all()?;
            orbit.set_input_region();
            orbit.schedule_clock();
            #[cfg(feature = "music")]
            orbit.start_music();
            orbit.need_bluetooth(false);
            orbit.gfx.trim();

            let _ = ShowWindow(render_hwnd, SW_SHOWNOACTIVATE);
            let _ = ShowWindow(input_hwnd, SW_SHOWNOACTIVATE);
            Ok(orbit)
        }
    }

    pub fn handle(
        &mut self,
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<LRESULT> {
        if hwnd == self.render_hwnd {
            return (msg == WM_NCHITTEST).then_some(LRESULT(HTTRANSPARENT as isize));
        }
        #[cfg(feature = "launcher")]
        if self.launcher.as_ref().is_some_and(|l| l.hwnd == hwnd) {
            return self.launcher_message(msg, wparam, lparam);
        }
        if hwnd != self.input_hwnd {
            return None;
        }
        match msg {
            WM_MOUSEMOVE => {
                self.track_mouse();
                let point = point_from(lparam);
                if self.hub_open() {
                    let _ = self.hub_hover(point);
                } else if self.expanded {
                    let _ = self.hover_at(point);
                }
                Some(LRESULT(0))
            }
            WM_TIMER if wparam.0 == HOVER_TIMER => {
                self.disarm_hover();
                if self.cursor_over_input()
                    && !self.expanded
                    && !self.hub_open()
                    && !self.launcher_is_open()
                {
                    let _ = self.expand();
                }
                Some(LRESULT(0))
            }
            WM_TIMER if wparam.0 == HIDE_TIMER => {
                self.stop_timer(HIDE_TIMER);
                if self.hidden {
                    unsafe {
                        let _ = ShowWindow(self.render_hwnd, SW_HIDE);
                    }
                }
                Some(LRESULT(0))
            }
            WM_TIMER if wparam.0 == CLOCK_TIMER => {
                let _ = self.refresh_clock();
                Some(LRESULT(0))
            }
            WM_TIMER if wparam.0 == NOTICE_TIMER => {
                let sticky = self.notice.as_ref().and_then(|n| n.sticky);
                let again = match sticky {
                    Some(since) if last_input() == since => Some(1000),
                    Some(_) => {
                        if let Some(notice) = &mut self.notice {
                            notice.sticky = None;
                        }
                        Some(self.config.notices.seconds as u32 * 2000)
                    }
                    None if self.cursor_on_notice() => Some(1000),
                    None => None,
                };
                match again {
                    Some(ms) => unsafe {
                        SetTimer(Some(self.input_hwnd), NOTICE_TIMER, ms, None);
                    },
                    None => {
                        self.dismiss_notice(now());
                        let _ = self.apply_all();
                    }
                }
                Some(LRESULT(0))
            }
            #[cfg(feature = "timers")]
            WM_TIMER if wparam.0 == timers::RING_TIMER => {
                let _ = self.tick_timers();
                Some(LRESULT(0))
            }
            WM_TIMER if wparam.0 == NOTICE_GONE_TIMER => {
                self.stop_timer(NOTICE_GONE_TIMER);
                if self.notice.as_ref().is_some_and(|n| n.hiding) {
                    self.remove_notice();
                    unsafe {
                        let _ = self.gfx.dcomp.Commit();
                    }
                }
                Some(LRESULT(0))
            }
            #[cfg(feature = "launcher")]
            WM_TIMER if wparam.0 == launcher::SLEEP_TIMER => {
                self.stop_timer(launcher::SLEEP_TIMER);
                launcher::system::sleep(self.input_hwnd);
                Some(LRESULT(0))
            }
            #[cfg(feature = "launcher")]
            WM_TIMER if wparam.0 == launcher::GONE_TIMER => {
                self.stop_timer(launcher::GONE_TIMER);
                if self.launcher_view.as_ref().is_some_and(|v| v.hiding) {
                    self.remove_launcher_view();
                }
                Some(LRESULT(0))
            }
            WM_MOUSELEAVE => {
                // Changing the window region with SetWindowRgn posts WM_MOUSELEAVE even
                // though the cursor did not move.
                if self.cursor_over_input() {
                    self.track_mouse();
                    return Some(LRESULT(0));
                }
                self.disarm_hover();
                if self.hub_open() {
                    let _ = self.close_hub();
                }
                if self.expanded {
                    let _ = self.collapse();
                }
                Some(LRESULT(0))
            }
            WM_DPICHANGED => {
                unsafe {
                    let _ = PostMessageW(Some(self.host), WM_SCREEN_CHANGED, WPARAM(0), LPARAM(0));
                }
                Some(LRESULT(0))
            }
            WM_MOUSEACTIVATE => Some(LRESULT(MA_NOACTIVATE as isize)),
            WM_LBUTTONDOWN => {
                self.press(point_from(lparam));
                Some(LRESULT(0))
            }
            WM_LBUTTONUP => {
                self.stop_timer(HOLD_TIMER);
                self.pressed = None;
                if !std::mem::take(&mut self.held) {
                    let _ = self.click(point_from(lparam));
                }
                Some(LRESULT(0))
            }
            WM_TIMER if wparam.0 == HOLD_TIMER => {
                self.stop_timer(HOLD_TIMER);
                let _ = self.hold();
                Some(LRESULT(0))
            }
            WM_RBUTTONUP => {
                if self.hub_open() {
                    self.pressed = self.hub_tile_at(point_from(lparam));
                    let _ = self.hold();
                    self.held = false;
                }
                Some(LRESULT(0))
            }
            #[cfg(feature = "bluetooth")]
            bluetooth::WM_BLUETOOTH => {
                let _ = self.on_bluetooth(wparam.0 == 1);
                Some(LRESULT(0))
            }
            #[cfg(feature = "music")]
            WM_MOUSEWHEEL => {
                let screen = point_from(lparam);
                let window = Vec2::new(self.geo.window.left as f64, self.geo.window.top as f64);
                let notches = ((wparam.0 >> 16) & 0xFFFF) as i16 as f32 / 120.0;
                let _ = self.wheel(screen - window, notches);
                Some(LRESULT(0))
            }
            #[cfg(feature = "music")]
            media::WM_MEDIA => {
                let _ = self.on_media();
                Some(LRESULT(0))
            }
            #[cfg(feature = "music")]
            WM_TIMER if wparam.0 == MUSIC_TIMER => {
                let _ = self.refresh(SatelliteKind::Music);
                self.schedule_music();
                Some(LRESULT(0))
            }
            _ => None,
        }
    }

    pub fn refresh_battery(&mut self) -> Result<()> {
        self.refresh(SatelliteKind::Battery)
    }

    pub fn refresh_clock(&mut self) -> Result<()> {
        self.stop_timer(CLOCK_TIMER);
        self.schedule_clock();
        self.refresh(SatelliteKind::Clock)
    }

    // A continuously running animation would make DWM render every frame,
    // so the clock ring is redrawn once a minute instead.
    fn schedule_clock(&self) {
        if self
            .satellites
            .iter()
            .any(|s| s.kind == SatelliteKind::Clock)
        {
            unsafe {
                SetTimer(
                    Some(self.input_hwnd),
                    CLOCK_TIMER,
                    info::ms_to_next_minute(),
                    None,
                );
            }
        }
    }

    fn refresh(&mut self, kind: SatelliteKind) -> Result<()> {
        let look = self.current_look(kind);
        let mut changed = false;
        for satellite in self.satellites.iter_mut().filter(|s| s.kind == kind) {
            if look.differs(&satellite.look) {
                satellite.paint(&self.gfx, &self.geo, look)?;
                changed = true;
            }
        }
        let owner = self.card.as_ref().and_then(|card| card.owner);
        if let Some(owner) = owner
            && self.kind_at(owner) == Some(kind)
            && (kind != SatelliteKind::Music || changed)
        {
            self.paint_card(owner)?;
            changed = true;
        }
        if changed {
            self.gfx.trim();
            unsafe { self.gfx.dcomp.Commit()? };
        }
        Ok(())
    }

    fn current_look(&self, kind: SatelliteKind) -> info::Look {
        #[cfg(feature = "music")]
        if kind == SatelliteKind::Music {
            let track = self.music.as_ref().and_then(|music| music.track.as_ref());
            return info::music_look(track, now());
        }
        info::look(kind)
    }

    fn paint_card(&self, index: usize) -> Result<()> {
        #[cfg(feature = "timers")]
        if index >= self.satellites.len() {
            return self.paint_timer_card(index);
        }
        let satellite = &self.satellites[index];
        let (kind, color) = (satellite.kind, satellite.look.color);
        let Some(card) = &self.card else {
            return Ok(());
        };
        #[cfg(feature = "music")]
        if kind == SatelliteKind::Music {
            let music = self.music.as_ref();
            let cover = music.and_then(|music| music.cover.as_ref());
            let track = music.and_then(|music| music.track.as_ref());
            let app = music.map_or("", |music| music.app.as_str());
            let volume = self.volume.as_ref().and_then(|volume| volume.get(app));
            return card.paint_music(&self.gfx, &self.geo, track, cover, volume, color);
        }
        card.paint(&self.gfx, &self.geo, kind, color)
    }

    pub fn set_rotating(&mut self, on: bool) -> Result<()> {
        if on == self.clock.running {
            return Ok(());
        }
        let t = now();
        if on {
            let spin_up = if self.expanded { 0.0 } else { STOP_SECS / 2.0 };
            self.clock.resume(t, spin_up);
        } else {
            let coast = if self.expanded {
                0.0
            } else {
                self.clock.speed * STOP_SECS / 2.0
            };
            self.clock.pause(t, coast);
        }
        self.retarget(t, STOP_SECS, false, 0.0);
        self.apply_all()
    }

    pub fn set_hidden(&mut self, hidden: bool) -> Result<()> {
        if hidden == self.hidden {
            return Ok(());
        }
        self.hidden = hidden;
        let t = now();
        unsafe {
            if hidden {
                #[cfg(feature = "launcher")]
                let _ = self.close_launcher(false);
                self.disarm_hover();
                self.dismiss_notice(t);
                if self.expanded {
                    self.expanded = false;
                    self.hide_card(t);
                    self.retarget(t, FADE_SECS, false, 0.0);
                    self.set_input_region();
                }
                let _ = ShowWindow(self.input_hwnd, SW_HIDE);
                self.fade = self.fade.to(t, 0.0, FADE_SECS, Target::hold(0.0), false);
                SetTimer(
                    Some(self.input_hwnd),
                    HIDE_TIMER,
                    (FADE_SECS * 1000.0) as u32 + 50,
                    None,
                );
            } else {
                self.stop_timer(HIDE_TIMER);
                let _ = ShowWindow(self.render_hwnd, SW_SHOWNOACTIVATE);
                let _ = ShowWindow(self.input_hwnd, SW_SHOWNOACTIVATE);
                self.fade = self.fade.to(t, 0.0, FADE_SECS, Target::hold(1.0), false);
            }
        }
        self.apply_all()?;
        self.flush_notice()
    }

    pub fn set_radii(&mut self, radius_x: f32, radius_y: f32) -> Result<()> {
        self.config.radius_x = radius_x;
        self.config.radius_y = radius_y;
        self.relayout()
    }

    pub fn set_notices(&mut self, notices: NoticeConfig) {
        self.config.notices = notices;
    }

    pub fn power_changed(&mut self, on_battery: bool) -> Result<()> {
        if self.config.notices.charger
            && let Some(notice) = info::power_notice(on_battery)
        {
            self.show_notice(notice)?;
        }
        self.check_low_battery()
    }

    pub fn check_low_battery(&mut self) -> Result<()> {
        let notice = info::low_battery_notice(&mut self.low_warned);
        match notice {
            Some(notice) if self.config.notices.low_battery => self.show_notice(notice),
            _ => Ok(()),
        }
    }

    #[cfg(feature = "scenarios")]
    pub fn message_notice(&mut self, text: String) -> Result<()> {
        self.show_notice(info::Notice {
            kind: None,
            text,
            look: info::Look {
                color: 0xD9DCE3,
                progress: 0.0,
            },
            long: true,
        })
    }

    fn show_notice(&mut self, notice: info::Notice) -> Result<()> {
        self.show_notice_at(notice, None, false)
    }

    fn show_notice_at(
        &mut self,
        notice: info::Notice,
        from: Option<Vec2>,
        sticky: bool,
    ) -> Result<()> {
        if self.hidden || self.expanded || self.hub_open() || self.launcher_is_open() {
            #[cfg(feature = "timers")]
            if sticky {
                self.pending_notice = Some(notice);
            }
            return Ok(());
        }
        self.stop_timer(NOTICE_GONE_TIMER);
        self.remove_notice();
        let geo = self.geo;
        let mut view =
            scene::build_notice(&self.gfx, &geo, &self.overlay, &notice.text, notice.look)?;
        view.owner = notice.kind;
        view.sticky = sticky.then(last_input);
        let x = (geo.center.x - view.width / 2.0 + view.reach).round();
        let below_orbit = geo.orbit.ry + geo.ring_radius * (1.0 + DEPTH_SCALE);
        let y = (geo.center.y + below_orbit + 6.0 * geo.scale + view.height / 2.0).round();
        view.rect = (
            x - view.reach,
            y - view.height / 2.0,
            view.width,
            view.height,
        );
        let t = now();
        let from = from.unwrap_or_else(|| {
            notice
                .kind
                .map_or(self.geo.center, |kind| self.owner_position(kind, t))
        });
        view.x = Channel::fixed(from.x).to(t, 0.0, NOTICE_IN_SECS, Target::hold(x), false);
        view.y = Channel::fixed(from.y).to(t, 0.0, NOTICE_IN_SECS, Target::hold(y), true);
        view.size = Channel::fixed(0.3).to(t, 0.0, NOTICE_IN_SECS, Target::hold(1.0), true);
        view.alpha = view
            .alpha
            .to(t, 0.0, NOTICE_IN_SECS / 2.0, Target::hold(1.0), false);
        view.shine = Channel::fixed(0.0).to(
            t,
            0.0,
            NOTICE_IN_SECS,
            Target::wave(Wave {
                bias: 0.75,
                amp: 0.25,
                omega: TAU / NOTICE_PULSE_SECS,
                phase: PI / 2.0,
            }),
            false,
        );
        self.notice = Some(view);

        let seconds = self.config.notices.seconds as f64 * if notice.long { 2.0 } else { 1.0 };
        unsafe {
            SetTimer(
                Some(self.input_hwnd),
                NOTICE_TIMER,
                ((seconds + NOTICE_IN_SECS) * 1000.0) as u32,
                None,
            );
        }
        self.gfx.trim();
        self.apply_all()
    }

    fn dismiss_notice(&mut self, t: f64) {
        self.stop_timer(NOTICE_TIMER);
        let Some(owner) = self.notice.as_ref().filter(|v| !v.hiding).map(|v| v.owner) else {
            return;
        };
        let to = match owner {
            Some(kind) => self.owner_position(kind, t + NOTICE_OUT_SECS),
            None => self.geo.center,
        };
        let Some(view) = &mut self.notice else {
            return;
        };
        view.hiding = true;
        view.x = view
            .x
            .to(t, 0.0, NOTICE_OUT_SECS, Target::hold(to.x), false);
        view.y = view
            .y
            .to(t, 0.0, NOTICE_OUT_SECS, Target::hold(to.y), false);
        view.shine = view
            .shine
            .to(t, 0.0, NOTICE_OUT_SECS, Target::hold(0.0), false);
        view.size = view
            .size
            .to(t, 0.0, NOTICE_OUT_SECS, Target::hold(0.3), false);
        view.alpha = view
            .alpha
            .to(t, 0.0, NOTICE_OUT_SECS, Target::hold(0.0), false);
        unsafe {
            SetTimer(
                Some(self.input_hwnd),
                NOTICE_GONE_TIMER,
                (NOTICE_OUT_SECS * 1000.0) as u32 + 50,
                None,
            );
        }
    }

    fn owner_position(&self, kind: SatelliteKind, t: f64) -> Vec2 {
        self.satellites
            .iter()
            .find(|s| s.kind == kind)
            .map_or(self.geo.center, |s| {
                Vec2::new(s.x.sample(t).0, s.y.sample(t).0)
            })
    }

    fn flush_notice(&mut self) -> Result<()> {
        #[cfg(feature = "timers")]
        if let Some(notice) = self.pending_notice.take() {
            return self.show_notice_at(notice, None, true);
        }
        Ok(())
    }

    fn remove_notice(&mut self) {
        if let Some(view) = self.notice.take() {
            unsafe {
                let _ = self.overlay.RemoveVisual(&view.body);
            }
        }
    }

    fn cursor_on_notice(&self) -> bool {
        let Some(view) = &self.notice else {
            return false;
        };
        let mut cursor = POINT::default();
        if unsafe { GetCursorPos(&mut cursor) }.is_err() {
            return false;
        }
        let window = Vec2::new(self.geo.window.left as f64, self.geo.window.top as f64);
        let point = Vec2::new(cursor.x as f64, cursor.y as f64) - window;
        inside(point, view.rect)
    }

    pub fn device_lost_event(&self) -> Option<HANDLE> {
        self.gfx.lost_event()
    }

    #[cfg(feature = "dev")]
    pub fn simulate_device_lost(&self) {
        self.gfx.simulate_device_lost();
    }

    pub fn scale_changed(&self) -> bool {
        screen::dpi() as f64 / 96.0 != self.geo.scale
    }

    pub fn follow_screen(&mut self) -> Result<()> {
        if Geometry::new(&self.config).window != self.geo.window {
            self.relayout()?;
        }
        Ok(())
    }

    pub fn relayout(&mut self) -> Result<()> {
        #[cfg(feature = "launcher")]
        {
            let _ = self.close_launcher(false);
            self.remove_launcher_view();
        }
        let old_window = self.geo.window;
        self.geo = Geometry::new(&self.config);
        if self.geo.window != old_window {
            self.move_windows();
        }
        let t = now();
        self.dismiss_notice(t);
        self.retarget(t, RESHAPE_SECS, false, 0.0);
        if self.hub_open() {
            self.paint_hub()?;
            let count = self.hub.as_ref().map_or(0, |hub| hub.count);
            let (x, y, w, h) = self.hub_position(count);
            if let Some(hub) = &mut self.hub {
                hub.x = hub.x.to(t, 0.0, RESHAPE_SECS, Target::hold(x), false);
                hub.y = hub.y.to(t, 0.0, RESHAPE_SECS, Target::hold(y), false);
                hub.rect = (x, y, w, h);
            }
        }
        self.set_input_region();
        self.apply_all()
    }

    fn expand(&mut self) -> Result<()> {
        self.expanded = true;
        self.ensure_labels()?;
        #[cfg(feature = "timers")]
        {
            self.build_timer_labels()?;
            self.schedule_ring();
        }
        let t = now();
        self.dismiss_notice(t);
        self.retarget(t, SPREAD_SECS, true, 0.0);
        self.set_input_region();
        self.apply_all()
    }

    fn collapse(&mut self) -> Result<()> {
        self.expanded = false;
        let t = now();
        self.hide_card(t);
        self.retarget(t, RETURN_SECS, false, 0.0);
        self.set_input_region();
        #[cfg(feature = "timers")]
        self.schedule_ring();
        self.apply_all()?;
        self.flush_notice()
    }

    fn ensure_labels(&mut self) -> Result<()> {
        if self.labels.is_some() {
            return Ok(());
        }
        let items: Vec<(&str, (f64, f64))> = (0..self.satellites.len())
            .map(|index| {
                let offset = self.slot(index) - self.geo.center;
                let below = self.geo.ring_radius * HOVER_SCALE + self.geo.label_gap;
                (
                    info::label(self.satellites[index].kind),
                    (offset.x, offset.y + below),
                )
            })
            .collect();
        let mut labels = scene::build_labels(&self.gfx, &self.geo, &self.labels_holder, &items)?;
        labels.alpha = Channel::fixed(0.0);
        self.labels = Some(labels);
        self.gfx.trim();
        Ok(())
    }

    fn hover_at(&mut self, point: Vec2) -> Result<()> {
        let current = self.card.as_ref().and_then(|card| card.owner);
        let hovered = self.satellite_at(point);
        #[cfg(feature = "timers")]
        let hovered = hovered.or_else(|| self.timer_at(point));
        // A satellite under the cursor wins over the open card: a wide card
        // can lie under a neighbouring satellite.
        if hovered.is_none() && current.is_some() && self.over_card(point) {
            return Ok(());
        }
        if hovered == current {
            return Ok(());
        }
        let t = now();
        match hovered {
            Some(index) => self.show_card(t, index)?,
            None => self.hide_card(t),
        }
        #[cfg(feature = "timers")]
        self.schedule_ring();
        self.set_input_region();
        self.apply_all()
    }

    fn satellite_at(&self, point: Vec2) -> Option<usize> {
        let reach = self.geo.ring_radius * HOVER_SCALE + 6.0 * self.geo.scale;
        (0..self.satellites.len()).find(|&index| {
            let d = self.slot(index) - point;
            d.x * d.x + d.y * d.y <= reach * reach
        })
    }

    fn over_card(&self, point: Vec2) -> bool {
        let Some((card, owner)) = self.card.as_ref().and_then(|c| c.owner.map(|o| (c, o))) else {
            return false;
        };
        let (x, y, w, h) = card.rect;
        let top = self.place(owner).y;
        point.x >= x && point.x <= x + w && point.y >= top && point.y <= y + h
    }

    fn click(&mut self, point: Vec2) -> Result<()> {
        if self.launcher_is_open() {
            return Ok(());
        }
        let d = point - self.geo.center;
        if d.x * d.x + d.y * d.y <= self.geo.hover_radius * self.geo.hover_radius {
            return if self.hub_open() {
                self.close_hub()
            } else {
                self.open_hub()
            };
        }
        if self.hub_open() {
            return self.hub_click(point);
        }
        #[cfg(feature = "timers")]
        if self.expanded && self.timer_click(point) {
            return Ok(());
        }
        #[cfg(feature = "music")]
        if self.expanded {
            self.music_click(point)?;
        }
        Ok(())
    }

    fn show_card(&mut self, t: f64, index: usize) -> Result<()> {
        if self.card.is_none() {
            self.card = Some(scene::build_card(&self.gfx, &self.geo, &self.overlay)?);
        }
        let geo = self.geo;
        let slot = self.place(index);
        let kind = self.kind_at(index).unwrap_or(SatelliteKind::Clock);
        let (width, height) = scene::card_size(&geo, kind);
        let x = (slot.x - width / 2.0)
            .clamp(geo.margin, geo.width() - width - geo.margin)
            .round();
        let y = (slot.y
            + geo.ring_radius * HOVER_SCALE
            + geo.label_gap
            + geo.label_height
            + geo.card_gap)
            .round();

        if let Some(card) = &mut self.card {
            card.rect = (x, y, width, height);
        }
        self.paint_card(index)?;
        let card = self.card.as_mut().expect("card was just created");
        if card.owner.is_none() {
            card.x = Channel::fixed(x);
            card.y = Channel::fixed(y - 6.0 * geo.scale);
        }
        card.x = card.x.to(t, 0.0, CARD_SHOW_SECS, Target::hold(x), true);
        card.y = card.y.to(t, 0.0, CARD_SHOW_SECS, Target::hold(y), true);
        card.alpha = card
            .alpha
            .to(t, 0.0, CARD_SHOW_SECS, Target::hold(1.0), false);
        card.owner = Some(index);
        self.gfx.trim();
        Ok(())
    }

    fn hide_card(&mut self, t: f64) {
        if let Some(card) = &mut self.card
            && card.owner.is_some()
        {
            card.alpha = card
                .alpha
                .to(t, 0.0, CARD_HIDE_SECS, Target::hold(0.0), false);
            card.owner = None;
        }
    }

    fn retarget(&mut self, t: f64, dur: f64, kick: bool, stagger: f64) {
        for index in 0..self.satellites.len() {
            let delay = stagger * index as f64;
            let goal = self.satellite_goal(index, t + delay + dur);
            let shine = self.twinkle_goal(index);
            let s = &mut self.satellites[index];
            s.x = s.x.to(t, delay, dur, goal.x, kick);
            s.y = s.y.to(t, delay, dur, goal.y, kick);
            s.size = s.size.to(t, delay, dur, goal.size, kick);
            s.alpha = s.alpha.to(t, delay, dur, goal.alpha, kick);
            s.shine = s.shine.to(t, delay, dur, shine, false);
        }
        #[cfg(feature = "timers")]
        self.retarget_timers(t, dur, kick);
        let (glow, glow_size) = self.halo_goal();
        let center = self.geo.center;
        let core = &mut self.core;
        core.x = core.x.to(t, 0.0, dur, Target::hold(center.x), false);
        core.y = core.y.to(t, 0.0, dur, Target::hold(center.y), false);
        core.glow = core.glow.to(t, 0.0, dur, glow, false);
        core.glow_size = core.glow_size.to(t, 0.0, dur, glow_size, false);
        if let Some(labels) = &mut self.labels {
            let shown = if self.expanded { 1.0 } else { 0.0 };
            labels.alpha =
                labels
                    .alpha
                    .to(t, 0.0, dur.min(SPREAD_SECS), Target::hold(shown), false);
        }
    }

    fn satellite_goal(&self, index: usize, arrive: f64) -> Goal {
        if self.expanded {
            let slot = self.slot(index);
            return Goal {
                x: Target::hold(slot.x),
                y: Target::hold(slot.y),
                size: Target::hold(HOVER_SCALE),
                alpha: Target::hold(1.0),
            };
        }

        self.orbit_goal(self.satellites[index].phase, arrive)
    }

    fn orbit_goal(&self, phase: f64, arrive: f64) -> Goal {
        let e = self.geo.orbit;
        let angle = self.clock.angle(arrive) + phase;
        if !self.clock.running {
            let pos = e.pos(angle);
            let depth = e.depth(angle);
            return Goal {
                x: Target::hold(pos.x),
                y: Target::hold(pos.y),
                size: Target::hold(1.0 + DEPTH_SCALE * depth),
                alpha: Target::hold(DEPTH_ALPHA_MID + DEPTH_ALPHA * depth),
            };
        }

        // x = cx + rx·cos(a) = cx + rx·sin(a + 90°), y = cy ± ry·sin(a).
        let omega = self.clock.speed;
        let y_phase = if e.dir > 0.0 { angle } else { angle + PI };
        let wave = |bias, amp, phase| {
            Target::wave(Wave {
                bias,
                amp,
                omega,
                phase,
            })
        };
        Goal {
            x: wave(e.center.x, e.rx, angle + PI / 2.0),
            y: wave(e.center.y, e.ry, y_phase),
            size: wave(1.0, DEPTH_SCALE, y_phase),
            alpha: wave(DEPTH_ALPHA_MID, DEPTH_ALPHA, y_phase),
        }
    }

    fn twinkle_goal(&self, index: usize) -> Target {
        if !self.clock.running {
            return Target::hold(0.8);
        }
        let period = TWINKLE_SECS[index % TWINKLE_SECS.len()];
        Target::wave(Wave {
            bias: 0.75,
            amp: 0.25,
            omega: TAU / period,
            phase: 1.7 * index as f64,
        })
    }

    fn halo_goal(&self) -> (Target, Target) {
        if self.expanded {
            return (Target::hold(1.0), Target::hold(1.2));
        }
        if !self.clock.running {
            return (Target::hold(0.55), Target::hold(1.0));
        }
        let omega = TAU / BREATH_SECS;
        (
            Target::wave(Wave {
                bias: 0.55,
                amp: 0.25,
                omega,
                phase: 0.0,
            }),
            Target::wave(Wave {
                bias: 1.0,
                amp: 0.06,
                omega,
                phase: 0.0,
            }),
        )
    }

    fn slot(&self, index: usize) -> Vec2 {
        let count = self.satellites.len();
        let arc = SPREAD_ARC_DEG.to_radians();
        let angle = if count < 2 {
            PI / 2.0
        } else {
            PI / 2.0 + arc / 2.0 - arc * index as f64 / (count - 1) as f64
        };
        self.geo.center + Vec2::new(angle.cos(), angle.sin()) * self.geo.layout_radius
    }

    fn place(&self, index: usize) -> Vec2 {
        #[cfg(feature = "timers")]
        if index >= self.satellites.len() {
            return self.timer_slot(index - self.satellites.len());
        }
        self.slot(index)
    }

    fn kind_at(&self, index: usize) -> Option<SatelliteKind> {
        self.satellites.get(index).map(|s| s.kind)
    }

    // Own one-shot hover timer: TME_HOVER waits until the cursor rests inside
    // a 4x4 px box, which takes seconds on a touchpad.
    fn track_mouse(&mut self) {
        let mut request = TRACKMOUSEEVENT {
            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: self.input_hwnd,
            dwHoverTime: 0,
        };
        let tracking = unsafe { TrackMouseEvent(&mut request) }.is_ok();
        if tracking && !self.expanded && !self.hub_open() && !self.hover_armed {
            self.hover_armed = true;
            unsafe {
                SetTimer(
                    Some(self.input_hwnd),
                    HOVER_TIMER,
                    self.config.hover_delay_ms,
                    None,
                );
            }
        }
    }

    fn cursor_over_input(&self) -> bool {
        let mut cursor = POINT::default();
        unsafe { GetCursorPos(&mut cursor).is_ok() && WindowFromPoint(cursor) == self.input_hwnd }
    }

    fn launcher_is_open(&self) -> bool {
        #[cfg(feature = "launcher")]
        return self.launcher_open();
        #[cfg(not(feature = "launcher"))]
        false
    }

    fn disarm_hover(&mut self) {
        self.stop_timer(HOVER_TIMER);
        self.hover_armed = false;
    }

    fn stop_timer(&self, id: usize) {
        unsafe {
            let _ = KillTimer(Some(self.input_hwnd), id);
        }
    }

    fn apply_all(&self) -> Result<()> {
        unsafe {
            for s in &self.satellites {
                self.bind(&s.x, |a| s.shift.SetOffsetX(a), |v| s.shift.SetOffsetX2(v))?;
                self.bind(&s.y, |a| s.shift.SetOffsetY(a), |v| s.shift.SetOffsetY2(v))?;
                self.bind(&s.size, |a| s.scale.SetScaleX(a), |v| s.scale.SetScaleX2(v))?;
                self.bind(&s.size, |a| s.scale.SetScaleY(a), |v| s.scale.SetScaleY2(v))?;
                self.bind(
                    &s.alpha,
                    |a| s.body.SetOpacity(a),
                    |v| s.body.SetOpacity2(v),
                )?;
                self.bind(
                    &s.shine,
                    |a| s.glow.SetOpacity(a),
                    |v| s.glow.SetOpacity2(v),
                )?;
            }
            let core = &self.core;
            let label_shift = self.labels.as_ref().map(|labels| &labels.shift);
            for shift in core.shifts.iter().chain(label_shift) {
                self.bind(&core.x, |a| shift.SetOffsetX(a), |v| shift.SetOffsetX2(v))?;
                self.bind(&core.y, |a| shift.SetOffsetY(a), |v| shift.SetOffsetY2(v))?;
            }
            self.bind(
                &core.glow,
                |a| core.halo.SetOpacity(a),
                |v| core.halo.SetOpacity2(v),
            )?;
            let scale = &core.halo_scale;
            self.bind(
                &core.glow_size,
                |a| scale.SetScaleX(a),
                |v| scale.SetScaleX2(v),
            )?;
            self.bind(
                &core.glow_size,
                |a| scale.SetScaleY(a),
                |v| scale.SetScaleY2(v),
            )?;
            if let Some(labels) = &self.labels {
                self.bind(
                    &labels.alpha,
                    |a| labels.layer.SetOpacity(a),
                    |v| labels.layer.SetOpacity2(v),
                )?;
            }
            if let Some(card) = &self.card {
                self.bind(
                    &card.x,
                    |a| card.shift.SetOffsetX(a),
                    |v| card.shift.SetOffsetX2(v),
                )?;
                self.bind(
                    &card.y,
                    |a| card.shift.SetOffsetY(a),
                    |v| card.shift.SetOffsetY2(v),
                )?;
                self.bind(
                    &card.alpha,
                    |a| card.visual.SetOpacity(a),
                    |v| card.visual.SetOpacity2(v),
                )?;
            }
            if let Some(hub) = &self.hub {
                let (shift, light) = (&hub.shift, &hub.highlight_shift);
                self.bind(&hub.x, |a| shift.SetOffsetX(a), |v| shift.SetOffsetX2(v))?;
                self.bind(&hub.y, |a| shift.SetOffsetY(a), |v| shift.SetOffsetY2(v))?;
                self.bind(
                    &hub.alpha,
                    |a| hub.visual.SetOpacity(a),
                    |v| hub.visual.SetOpacity2(v),
                )?;
                self.bind(&hub.hl_x, |a| light.SetOffsetX(a), |v| light.SetOffsetX2(v))?;
                self.bind(&hub.hl_y, |a| light.SetOffsetY(a), |v| light.SetOffsetY2(v))?;
                self.bind(
                    &hub.hl_alpha,
                    |a| hub.highlight.SetOpacity(a),
                    |v| hub.highlight.SetOpacity2(v),
                )?;
            }
            if let Some(n) = &self.notice {
                self.bind(&n.x, |a| n.shift.SetOffsetX(a), |v| n.shift.SetOffsetX2(v))?;
                self.bind(&n.y, |a| n.shift.SetOffsetY(a), |v| n.shift.SetOffsetY2(v))?;
                self.bind(&n.size, |a| n.scale.SetScaleX(a), |v| n.scale.SetScaleX2(v))?;
                self.bind(&n.size, |a| n.scale.SetScaleY(a), |v| n.scale.SetScaleY2(v))?;
                self.bind(
                    &n.alpha,
                    |a| n.body.SetOpacity(a),
                    |v| n.body.SetOpacity2(v),
                )?;
                self.bind(
                    &n.shine,
                    |a| n.glow.SetOpacity(a),
                    |v| n.glow.SetOpacity2(v),
                )?;
            }
            self.bind(
                &self.fade,
                |a| self.root.SetOpacity(a),
                |v| self.root.SetOpacity2(v),
            )?;
            #[cfg(feature = "launcher")]
            self.bind_launcher()?;
            #[cfg(feature = "timers")]
            self.apply_timers()?;
            self.gfx.dcomp.Commit()
        }
    }

    fn bind(
        &self,
        channel: &Channel,
        animated: impl FnOnce(&IDCompositionAnimation) -> Result<()>,
        fixed: impl FnOnce(f32) -> Result<()>,
    ) -> Result<()> {
        if let Some(value) = channel.fixed_value() {
            return fixed(value as f32);
        }
        // DirectComposition starts an animation at Commit, so rebase it on now.
        animated(&self.animation(&channel.rebase(now()))?)
    }

    fn animation(&self, channel: &Channel) -> Result<IDCompositionAnimation> {
        unsafe {
            let animation = self.gfx.dcomp.CreateAnimation()?;
            let mut offset = 0.0;
            if let Some(lead) = &channel.lead {
                let [c0, c1, c2, c3] = lead.c.map(|c| c as f32);
                if channel.delay > 0.0 {
                    animation.AddCubic(0.0, c0, 0.0, 0.0, 0.0)?;
                    offset = channel.delay;
                }
                animation.AddCubic(offset, c0, c1, c2, c3)?;
                offset += lead.dur;
            }
            match channel.tail {
                Tail::Hold(value) => animation.End(offset, value as f32)?,
                Tail::Wave(wave) => animation.AddSinusoidal(
                    offset,
                    wave.bias as f32,
                    wave.amp as f32,
                    (wave.omega / TAU) as f32,
                    degrees(wave.phase),
                )?,
            }
            Ok(animation)
        }
    }

    fn set_input_region(&self) {
        let c = self.geo.center;
        let r = if self.expanded {
            self.geo.layout_radius + self.geo.satellite_radius + 10.0 * self.geo.scale
        } else {
            self.geo.hover_radius
        };
        unsafe {
            let region = CreateEllipticRgn(
                (c.x - r).floor() as i32,
                (c.y - r).floor() as i32,
                (c.x + r).ceil() as i32,
                (c.y + r).ceil() as i32,
            );
            if self.expanded
                && let Some(card) = &self.card
                && let Some(owner) = card.owner
            {
                let (x, y, w, h) = card.rect;
                let top = self.place(owner).y;
                let strip = CreateRectRgn(x as i32, top as i32, (x + w) as i32, (y + h) as i32);
                CombineRgn(Some(region), Some(region), Some(strip), RGN_OR);
                let _ = DeleteObject(strip.into());
            }
            #[cfg(feature = "timers")]
            if self.expanded
                && let Some((left, top, right, bottom)) = self.timer_region()
            {
                let strip = CreateRectRgn(left, top, right, bottom);
                CombineRgn(Some(region), Some(region), Some(strip), RGN_OR);
                let _ = DeleteObject(strip.into());
            }
            if let Some(hub) = &self.hub
                && hub.open
            {
                let (x, y, w, h) = hub.rect;
                let strip = CreateRectRgn(x as i32, c.y as i32, (x + w) as i32, (y + h) as i32);
                CombineRgn(Some(region), Some(region), Some(strip), RGN_OR);
                let _ = DeleteObject(strip.into());
            }
            SetWindowRgn(self.input_hwnd, Some(region), true);
        }
    }

    fn move_windows(&self) {
        let r = self.geo.window;
        for hwnd in [self.render_hwnd, self.input_hwnd] {
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOACTIVATE,
                );
            }
        }
    }
}

impl Orbit {
    fn hub_open(&self) -> bool {
        self.hub.as_ref().is_some_and(|hub| hub.open)
    }

    fn is_circle(&self) -> bool {
        (self.config.radius_x, self.config.radius_y) == OrbitConfig::CIRCLE
    }

    fn tile_available(&self, tile: &Tile) -> bool {
        match tile {
            Tile::Builtin(Builtin::Launcher) => self.launcher_on(),
            Tile::Builtin(Builtin::PlayPause | Builtin::Next | Builtin::Previous) => {
                self.music_on()
            }
            Tile::Builtin(Builtin::Bluetooth) => self.bluetooth_state().is_some(),
            Tile::Builtin(Builtin::Note) => notes_on(),
            Tile::Custom {
                kind: Kind::Command,
                ..
            } => cfg!(feature = "launcher"),
            Tile::Custom {
                kind: Kind::Scenario,
                ..
            } => scenarios_on(),
            _ => true,
        }
    }

    fn launcher_on(&self) -> bool {
        #[cfg(feature = "launcher")]
        return self.launcher_on;
        #[cfg(not(feature = "launcher"))]
        false
    }

    fn music_on(&self) -> bool {
        #[cfg(feature = "music")]
        return self.music.is_some();
        #[cfg(not(feature = "music"))]
        false
    }

    fn hub_tiles(&self) -> Vec<Tile> {
        let tiles: Vec<Tile> = self
            .config
            .tiles
            .iter()
            .filter(|tile| self.tile_available(tile))
            .cloned()
            .collect();
        if tiles.is_empty() {
            vec![Tile::Builtin(Builtin::Settings)]
        } else {
            tiles
        }
    }

    fn hub_items(&self) -> Vec<HubTile> {
        self.hub_tiles()
            .into_iter()
            .map(|tile| {
                let dim = tile == Tile::Builtin(Builtin::Bluetooth)
                    && self.bluetooth_state() == Some(false);
                let (icon, label) = match tile {
                    Tile::Builtin(builtin) => {
                        let (glyph, label) = self.builtin_look(builtin);
                        (TileIcon::Glyph(glyph), label.to_string())
                    }
                    Tile::Custom { kind, name, .. } => match kind.glyph() {
                        Some(glyph) => (TileIcon::Glyph(glyph), name),
                        None => (TileIcon::Letter(tile::letter(&name)), name),
                    },
                };
                HubTile { icon, label, dim }
            })
            .collect()
    }

    fn builtin_look(&self, builtin: Builtin) -> (char, &'static str) {
        match builtin {
            Builtin::Screenshot => ('\u{E722}', tr("Скриншот", "Screenshot")),
            Builtin::Rotation if self.config.rotate => ('\u{E769}', tr("Пауза", "Pause")),
            Builtin::Rotation => ('\u{E768}', tr("Вращать", "Rotate")),
            Builtin::Shape => (
                '\u{EA3A}',
                if self.is_circle() {
                    tr("Овал", "Oval")
                } else {
                    tr("Круг", "Circle")
                },
            ),
            Builtin::Settings => ('\u{E713}', tr("Настройки", "Settings")),
            Builtin::Launcher => ('\u{E721}', tr("Строка", "Launcher")),
            Builtin::PlayPause if self.music_playing() => ('\u{E769}', tr("Музыка", "Music")),
            Builtin::PlayPause => ('\u{E768}', tr("Музыка", "Music")),
            Builtin::Next => ('\u{E893}', tr("Дальше", "Next")),
            Builtin::Previous => ('\u{E892}', tr("Назад", "Back")),
            Builtin::Bluetooth => ('\u{E702}', "Bluetooth"),
            Builtin::Note => ('\u{E70B}', tr("Заметка", "Note")),
        }
    }

    fn music_playing(&self) -> bool {
        #[cfg(feature = "music")]
        return self
            .music
            .as_ref()
            .and_then(|music| music.track.as_ref())
            .is_some_and(|track| track.playing);
        #[cfg(not(feature = "music"))]
        false
    }

    pub fn set_hub(&mut self, tiles: &[Tile], launcher: bool, search: &str) {
        #[cfg(feature = "launcher")]
        let launcher_same = self.launcher_on == launcher;
        #[cfg(not(feature = "launcher"))]
        let launcher_same = {
            let _ = (launcher, search);
            true
        };
        #[cfg(feature = "launcher")]
        {
            self.launcher_on = launcher;
            self.search = search.to_string();
        }
        if self.config.tiles == tiles && launcher_same {
            return;
        }
        self.config.tiles = tiles.to_vec();
        self.remove_hub();
        self.need_bluetooth(false);
    }

    fn remove_hub(&mut self) {
        if let Some(hub) = self.hub.take() {
            unsafe {
                let _ = self.overlay.RemoveVisual(&hub.visual);
            }
            drop(hub);
            self.set_input_region();
            unsafe {
                let _ = self.gfx.dcomp.Commit();
            }
            self.gfx.trim();
        }
    }

    fn hub_position(&self, count: usize) -> (f64, f64, f64, f64) {
        let ((w, h), _) = scene::hub_layout(&self.geo, count);
        let geo = &self.geo;
        let x = (geo.center.x - w / 2.0)
            .clamp(geo.margin, geo.width() - w - geo.margin)
            .round();
        let y = (geo.center.y + geo.hover_radius + 6.0 * geo.scale).round();
        (x, y, w, h)
    }

    fn paint_hub(&self) -> Result<()> {
        match &self.hub {
            Some(hub) => hub.paint(&self.gfx, &self.geo, &self.hub_items()),
            None => Ok(()),
        }
    }

    fn open_hub(&mut self) -> Result<()> {
        let t = now();
        let count = self.hub_tiles().len();
        if self.hub.as_ref().is_some_and(|hub| hub.count != count) {
            self.remove_hub();
        }
        if self.hub.is_none() {
            self.hub = Some(scene::build_hub(
                &self.gfx,
                &self.geo,
                &self.overlay,
                count,
            )?);
        }
        self.disarm_hover();
        self.dismiss_notice(t);
        if self.expanded {
            self.expanded = false;
            self.hide_card(t);
            self.retarget(t, RETURN_SECS, false, 0.0);
        }
        self.paint_hub()?;
        let (x, y, w, h) = self.hub_position(count);
        let lift = 6.0 * self.geo.scale;
        let hub = self.hub.as_mut().expect("hub was just created");
        hub.x = Channel::fixed(x);
        hub.y = Channel::fixed(y - lift).to(t, 0.0, CARD_SHOW_SECS, Target::hold(y), true);
        hub.alpha = hub
            .alpha
            .to(t, 0.0, CARD_SHOW_SECS, Target::hold(1.0), false);
        hub.hl_alpha = Channel::fixed(0.0);
        hub.hovered = None;
        hub.rect = (x, y, w, h);
        hub.open = true;
        self.set_input_region();
        self.gfx.trim();
        self.apply_all()
    }

    fn close_hub(&mut self) -> Result<()> {
        let t = now();
        if let Some(hub) = &mut self.hub {
            hub.alpha = hub
                .alpha
                .to(t, 0.0, CARD_HIDE_SECS, Target::hold(0.0), false);
            hub.hl_alpha = hub
                .hl_alpha
                .to(t, 0.0, CARD_HIDE_SECS, Target::hold(0.0), false);
            hub.hovered = None;
            hub.open = false;
        }
        self.set_input_region();
        self.apply_all()?;
        self.flush_notice()
    }

    fn hub_tile_at(&self, point: Vec2) -> Option<usize> {
        let hub = self.hub.as_ref()?;
        let (hx, hy, _, _) = hub.rect;
        let (_, tiles) = scene::hub_layout(&self.geo, hub.count);
        tiles
            .iter()
            .position(|&(x, y, w, h)| inside(point, (hx + x, hy + y, w, h)))
    }

    fn hub_hover(&mut self, point: Vec2) -> Result<()> {
        let tile = self.hub_tile_at(point);
        let Some(hub) = &mut self.hub else {
            return Ok(());
        };
        if hub.hovered == tile {
            return Ok(());
        }
        let t = now();
        match tile {
            Some(index) => {
                let (_, tiles) = scene::hub_layout(&self.geo, hub.count);
                let (x, y, _, _) = tiles[index];
                if hub.hovered.is_none() {
                    hub.hl_x = Channel::fixed(x);
                    hub.hl_y = Channel::fixed(y);
                }
                hub.hl_x = hub.hl_x.to(t, 0.0, CARD_SHOW_SECS, Target::hold(x), true);
                hub.hl_y = hub.hl_y.to(t, 0.0, CARD_SHOW_SECS, Target::hold(y), true);
                hub.hl_alpha = hub
                    .hl_alpha
                    .to(t, 0.0, CARD_HIDE_SECS, Target::hold(1.0), false);
            }
            None => {
                hub.hl_alpha = hub
                    .hl_alpha
                    .to(t, 0.0, CARD_HIDE_SECS, Target::hold(0.0), false)
            }
        }
        hub.hovered = tile;
        self.apply_all()
    }

    fn hub_click(&mut self, point: Vec2) -> Result<()> {
        let Some(index) = self.hub_tile_at(point) else {
            return Ok(());
        };
        let Some(tile) = self.hub_tiles().into_iter().nth(index) else {
            return Ok(());
        };
        let command = match &tile {
            Tile::Builtin(Builtin::Screenshot) => CMD_SCREENSHOT,
            Tile::Builtin(Builtin::Rotation) => CMD_ROTATION,
            Tile::Builtin(Builtin::Shape) => CMD_SHAPE_TOGGLE,
            Tile::Builtin(Builtin::Settings) => CMD_OPEN_CONFIG,
            #[cfg(feature = "notes")]
            Tile::Builtin(Builtin::Note) => {
                self.close_hub()?;
                crate::notes::create("");
                return Ok(());
            }
            #[cfg(not(feature = "notes"))]
            Tile::Builtin(Builtin::Note) => return Ok(()),
            Tile::Builtin(Builtin::PlayPause) => return self.hub_music(MusicKey::PlayPause),
            Tile::Builtin(Builtin::Next) => return self.hub_music(MusicKey::Next),
            Tile::Builtin(Builtin::Previous) => return self.hub_music(MusicKey::Previous),
            Tile::Builtin(Builtin::Bluetooth) => {
                self.toggle_bluetooth();
                return Ok(());
            }
            #[cfg(feature = "launcher")]
            Tile::Builtin(Builtin::Launcher) => {
                let search = self.search.clone();
                return self.toggle_launcher(&search);
            }
            #[cfg(not(feature = "launcher"))]
            Tile::Builtin(Builtin::Launcher) => return Ok(()),
            #[cfg(feature = "launcher")]
            Tile::Custom {
                kind: Kind::Command,
                target,
                ..
            } => {
                self.close_hub()?;
                let search = self.search.clone();
                return self.run_text(target, &search);
            }
            #[cfg(feature = "scenarios")]
            Tile::Custom {
                kind: Kind::Scenario,
                target,
                ..
            } => {
                self.close_hub()?;
                if crate::scenarios::monitors().len() > 1 {
                    let search = self.search.clone();
                    return self.run_text(target, &search);
                }
                crate::scenarios::run(target, crate::scenarios::Place::AsSaved, self.host);
                return Ok(());
            }
            #[cfg(not(feature = "scenarios"))]
            Tile::Custom {
                kind: Kind::Scenario,
                ..
            } => return Ok(()),
            Tile::Custom { target, .. } => {
                shell::open(target, None);
                return self.close_hub();
            }
        };
        if matches!(tile, Tile::Builtin(Builtin::Screenshot | Builtin::Settings)) {
            self.close_hub()?;
        }
        unsafe {
            let _ = PostMessageW(
                Some(self.host),
                WM_COMMAND,
                WPARAM(command as usize),
                LPARAM(0),
            );
        }
        Ok(())
    }

    fn hub_music(&mut self, key: MusicKey) -> Result<()> {
        #[cfg(feature = "music")]
        if let Some(music) = &self.music {
            match key {
                MusicKey::PlayPause => music.media.toggle(),
                MusicKey::Next => music.media.next(),
                MusicKey::Previous => music.media.previous(),
            }
        }
        #[cfg(not(feature = "music"))]
        let _ = key;
        Ok(())
    }

    pub fn set_rotate_setting(&mut self, on: bool) {
        if self.config.rotate != on {
            self.config.rotate = on;
            if self.hub_open() && self.paint_hub().is_ok() {
                unsafe {
                    let _ = self.gfx.dcomp.Commit();
                }
            }
        }
    }
}

#[cfg(feature = "music")]
impl Orbit {
    fn music_click(&mut self, point: Vec2) -> Result<()> {
        let Some(music) = &self.music else {
            return Ok(());
        };
        if let Some(index) = self.satellite_at(point)
            && self.satellites[index].kind == SatelliteKind::Music
        {
            music.media.toggle();
            return Ok(());
        }
        let Some((cx, cy)) = self.music_card_origin() else {
            return Ok(());
        };
        let local = point - Vec2::new(cx, cy);
        let pressed = scene::music_buttons(&self.geo)
            .iter()
            .position(|&rect| inside(local, rect));
        match pressed {
            Some(0) => music.media.previous(),
            Some(1) => music.media.toggle(),
            Some(2) => music.media.next(),
            _ => {
                let (mute, bar) = scene::volume_layout(&self.geo);
                let Some(volume) = &self.volume else {
                    return Ok(());
                };
                if inside(local, mute) {
                    volume.toggle_mute(&music.app);
                } else if inside(local, bar) {
                    volume.set(&music.app, ((local.x - bar.0) / bar.2) as f32);
                } else {
                    return Ok(());
                }
                self.repaint_music_card()?;
            }
        }
        Ok(())
    }

    fn wheel(&mut self, point: Vec2, notches: f32) -> Result<()> {
        if !self.expanded {
            return Ok(());
        }
        let over_satellite = self
            .satellite_at(point)
            .is_some_and(|index| self.satellites[index].kind == SatelliteKind::Music);
        let over_card = self.music_card_origin().is_some() && self.over_card(point);
        if !(over_satellite || over_card) {
            return Ok(());
        }
        if let (Some(volume), Some(music)) = (&self.volume, &self.music) {
            volume.change(&music.app, notches * 0.02);
        }
        self.repaint_music_card()
    }

    fn music_card_origin(&self) -> Option<(f64, f64)> {
        let card = self.card.as_ref()?;
        let owner = card.owner?;
        (self.kind_at(owner) == Some(SatelliteKind::Music)).then_some((card.rect.0, card.rect.1))
    }

    fn repaint_music_card(&mut self) -> Result<()> {
        let owner = self.card.as_ref().and_then(|card| card.owner);
        if let Some(owner) = owner
            && self.kind_at(owner) == Some(SatelliteKind::Music)
        {
            self.paint_card(owner)?;
            unsafe { self.gfx.dcomp.Commit()? };
        }
        Ok(())
    }
}

pub(crate) fn notes_on() -> bool {
    #[cfg(feature = "notes")]
    return crate::notes::enabled();
    #[cfg(not(feature = "notes"))]
    false
}

pub(crate) fn scenarios_on() -> bool {
    #[cfg(feature = "scenarios")]
    return crate::scenarios::enabled();
    #[cfg(not(feature = "scenarios"))]
    false
}

fn inside(point: Vec2, (x, y, w, h): (f64, f64, f64, f64)) -> bool {
    point.x >= x && point.x <= x + w && point.y >= y && point.y <= y + h
}

fn last_input() -> u32 {
    let mut info = LASTINPUTINFO {
        cbSize: size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    unsafe {
        let _ = GetLastInputInfo(&mut info);
    }
    info.dwTime
}

fn point_from(lparam: LPARAM) -> Vec2 {
    let x = (lparam.0 & 0xFFFF) as i16 as f64;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f64;
    Vec2::new(x, y)
}

#[cfg(feature = "music")]
struct Music {
    media: media::Media,
    track: Option<media::Track>,
    cover: Option<IWICBitmap>,
    cover_id: u64,
    app: String,
}

#[cfg(feature = "music")]
impl Orbit {
    fn start_music(&mut self) {
        if self
            .satellites
            .iter()
            .any(|s| s.kind == SatelliteKind::Music)
        {
            let media = media::Media::start(self.input_hwnd);
            self.music = Some(Music {
                media,
                track: None,
                cover: None,
                cover_id: 0,
                app: String::new(),
            });
            self.volume = volume::Volume::new();
        }
    }

    fn on_media(&mut self) -> Result<()> {
        let Some(music) = &mut self.music else {
            return Ok(());
        };
        let snapshot = music.media.snapshot();
        let track_changed = snapshot.track != music.track;
        let cover_changed = snapshot.cover_id != music.cover_id;
        let app_changed = snapshot.app != music.app;
        music.app = snapshot.app.clone();
        let new_song = match (&music.track, &snapshot.track) {
            (Some(old), Some(new)) => old.title != new.title || old.artist != new.artist,
            _ => false,
        };
        let notice = snapshot
            .track
            .as_ref()
            .filter(|track| new_song && !track.title.is_empty())
            .map(info::track_notice);
        music.track = snapshot.track;
        if cover_changed {
            music.cover_id = snapshot.cover_id;
            let size = scene::cover_size(&self.geo);
            music.cover = snapshot
                .cover
                .as_deref()
                .and_then(|bytes| self.gfx.decode_image(bytes, size).ok());
        }
        if track_changed || cover_changed || app_changed {
            self.music_changed()?;
        }
        self.schedule_music();
        if let Some(notice) = notice
            && self.config.notices.track
        {
            self.show_notice(notice)?;
        }
        Ok(())
    }

    fn music_changed(&mut self) -> Result<()> {
        self.refresh(SatelliteKind::Music)?;
        if self.hub_open() {
            self.paint_hub()?;
        }
        let owner = self.card.as_ref().and_then(|card| card.owner);
        if let Some(owner) = owner
            && self.kind_at(owner) == Some(SatelliteKind::Music)
        {
            self.paint_card(owner)?;
            self.gfx.trim();
            unsafe { self.gfx.dcomp.Commit()? };
        }
        self.schedule_music();
        Ok(())
    }

    // Move the track ring about one pixel per tick; no timer while paused.
    fn schedule_music(&self) {
        self.stop_timer(MUSIC_TIMER);
        let track = self.music.as_ref().and_then(|music| music.track.as_ref());
        let Some(track) = track.filter(|t| t.playing && t.duration > 0.0) else {
            return;
        };
        let circumference = TAU * self.geo.ring_radius;
        let seconds = (track.duration / circumference).clamp(1.0, 10.0);
        unsafe {
            SetTimer(
                Some(self.input_hwnd),
                MUSIC_TIMER,
                (seconds * 1000.0) as u32,
                None,
            );
        }
    }
}

impl Orbit {
    fn need_bluetooth(&mut self, for_launcher: bool) {
        #[cfg(feature = "bluetooth")]
        {
            if let Some(bluetooth) = &self.bluetooth {
                bluetooth.refresh();
                return;
            }
            let tile = self.config.tiles.iter().any(|tile| {
                matches!(
                    tile,
                    Tile::Builtin(Builtin::Bluetooth)
                        | Tile::Custom {
                            kind: Kind::Command,
                            ..
                        }
                )
            });
            if tile || for_launcher {
                self.bluetooth = Some(bluetooth::Bluetooth::start(self.input_hwnd));
            }
        }
        #[cfg(not(feature = "bluetooth"))]
        let _ = for_launcher;
    }

    fn bluetooth_state(&self) -> Option<bool> {
        #[cfg(feature = "bluetooth")]
        if let Some(bluetooth) = &self.bluetooth {
            return match bluetooth.state() {
                bluetooth::State::On => Some(true),
                bluetooth::State::Off => Some(false),
                _ => None,
            };
        }
        None
    }

    fn toggle_bluetooth(&self) {
        #[cfg(feature = "bluetooth")]
        if let Some(bluetooth) = &self.bluetooth {
            bluetooth.toggle();
        }
    }

    fn press(&mut self, point: Vec2) {
        self.held = false;
        self.pressed = None;
        if !self.hub_open() {
            return;
        }
        let index = self.hub_tile_at(point);
        let bluetooth = index
            .and_then(|i| self.hub_tiles().into_iter().nth(i))
            .is_some_and(|tile| tile == Tile::Builtin(Builtin::Bluetooth));
        if bluetooth {
            self.pressed = index;
            unsafe {
                SetTimer(Some(self.input_hwnd), HOLD_TIMER, HOLD_MS, None);
            }
        }
    }

    fn hold(&mut self) -> Result<()> {
        let tile = self
            .pressed
            .take()
            .and_then(|i| self.hub_tiles().into_iter().nth(i));
        if tile != Some(Tile::Builtin(Builtin::Bluetooth)) {
            return Ok(());
        }
        self.held = true;
        self.close_hub()?;
        shell::open("ms-settings:bluetooth", None);
        Ok(())
    }

    #[cfg(feature = "bluetooth")]
    fn on_bluetooth(&mut self, denied: bool) -> Result<()> {
        if denied {
            shell::open("ms-settings:bluetooth", None);
            return Ok(());
        }
        if self.hub_open() {
            let count = self.hub_tiles().len();
            if self.hub.as_ref().is_some_and(|hub| hub.count != count) {
                self.open_hub()?;
            } else {
                self.paint_hub()?;
                unsafe {
                    self.gfx.dcomp.Commit()?;
                }
            }
        }
        #[cfg(feature = "launcher")]
        self.launcher_bluetooth_changed()?;
        Ok(())
    }
}

impl Drop for Orbit {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.input_hwnd);
            let _ = DestroyWindow(self.render_hwnd);
        }
    }
}

fn degrees(radians: f64) -> f32 {
    radians.to_degrees().rem_euclid(360.0) as f32
}

fn create_windows(geo: &Geometry) -> Result<(HWND, HWND)> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let cursor = LoadCursorW(None, IDC_ARROW)?;
        for class_name in [RENDER_CLASS, INPUT_CLASS] {
            let class = WNDCLASSW {
                lpfnWndProc: Some(crate::app::window_proc),
                hInstance: instance.into(),
                lpszClassName: class_name,
                hCursor: cursor,
                ..Default::default()
            };
            RegisterClassW(&class);
        }

        let RECT {
            left,
            top,
            right,
            bottom,
        } = geo.window;
        let create = |ex_style: WINDOW_EX_STYLE, class_name: PCWSTR| {
            CreateWindowExW(
                ex_style
                    | WS_EX_NOREDIRECTIONBITMAP
                    | WS_EX_TOPMOST
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE,
                class_name,
                w!("Moonlet"),
                WS_POPUP,
                left,
                top,
                right - left,
                bottom - top,
                None,
                None,
                Some(instance.into()),
                None,
            )
        };

        let render = create(WS_EX_LAYERED | WS_EX_TRANSPARENT, RENDER_CLASS)?;
        SetLayeredWindowAttributes(render, COLORREF(0), 255, LWA_ALPHA)?;

        let input = create(WINDOW_EX_STYLE::default(), INPUT_CLASS)?;
        Ok((render, input))
    }
}
