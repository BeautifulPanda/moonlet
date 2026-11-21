use std::f64::consts::TAU;

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::Graphics::DirectComposition::{
    DCOMPOSITION_OPACITY_MODE_MULTIPLY, IDCompositionScaleTransform, IDCompositionSurface,
    IDCompositionTranslateTransform, IDCompositionVisual2, IDCompositionVisual3,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, SetTimer};
use windows::core::{Interface, Result};

use super::motion::{Channel, Target, Vec2, now};
use super::scene;
use super::{Geometry, Goal, HOVER_SCALE, Orbit, SPREAD_SECS, info, inside};
use crate::app::WM_REMINDERS;
use crate::gfx::{self, Gfx};
use crate::lang::tr;
use crate::reminders::{self, Reminder, Request};

pub const TIMER_COLOR: u32 = 0xF687B3;
pub const MAX_SHOWN: usize = 3;
pub const RING_TIMER: usize = 9;
const LAUNCH_SECS: f64 = 0.7;
const DOT_RATIO: f64 = 0.6;
const ROW_GAP: f64 = 32.0;
const ROW_STEP: f64 = 50.0;
pub const ROW_REACH: f64 = ROW_GAP + ROW_STEP * (MAX_SHOWN - 1) as f64 + 32.0;
const MIN_STEP_MS: f64 = 1000.0;
const MAX_STEP_MS: f64 = 60000.0;

struct Sizes {
    dot: f64,
    ring: f64,
    ring_width: f64,
    glow: f64,
}

fn sizes(geo: &Geometry) -> Sizes {
    let s = geo.scale;
    let dot = geo.satellite_radius * DOT_RATIO;
    Sizes {
        dot,
        ring: dot + 2.4 * s,
        ring_width: 1.3 * s,
        glow: dot * 2.8,
    }
}

fn even(radius: f64) -> u32 {
    2 * (radius + 2.0).ceil() as u32
}

pub struct TimerSat {
    pub reminder: Reminder,
    shift: IDCompositionTranslateTransform,
    scale: IDCompositionScaleTransform,
    pub body: IDCompositionVisual3,
    glow: IDCompositionVisual3,
    ring_surface: IDCompositionSurface,
    _surfaces: Vec<IDCompositionSurface>,
    _visuals: Vec<IDCompositionVisual2>,
    step: i32,
    pub x: Channel,
    pub y: Channel,
    pub size: Channel,
    pub alpha: Channel,
    pub shine: Channel,
}

fn ring_length(geo: &Geometry) -> f64 {
    TAU * sizes(geo).ring
}

fn steps(reminder: &Reminder, geo: &Geometry, now: i64) -> i32 {
    (reminder.left_fraction(now) as f64 * ring_length(geo)).ceil() as i32
}

fn build(
    gfx: &Gfx,
    geo: &Geometry,
    layer: &IDCompositionVisual2,
    reminder: &Reminder,
) -> Result<TimerSat> {
    unsafe {
        let z = sizes(geo);
        let (dot_size, ring_size, glow_size) =
            (even(z.dot), even(z.ring + z.ring_width), even(z.glow));
        let r = z.dot as f32;
        let color = TIMER_COLOR;
        let glow_surface = gfx.paint(glow_size, |p| {
            p.fill_radial(
                z.glow as f32,
                (0.0, 0.0),
                &[
                    (0.0, gfx::rgb(color, 0.55)),
                    (0.35, gfx::rgb(color, 0.25)),
                    (1.0, gfx::rgb(color, 0.0)),
                ],
            );
        })?;
        let dot_surface = gfx.paint(dot_size, |p| {
            p.fill_circle(r + 0.75, gfx::rgb(0x000000, 0.22));
            p.fill_radial(
                r,
                (-0.35 * r, -0.4 * r),
                &[
                    (0.0, gfx::rgb(gfx::mix(color, 0xFFFFFF, 0.55), 1.0)),
                    (0.6, gfx::rgb(color, 1.0)),
                    (1.0, gfx::rgb(gfx::mix(color, 0x000000, 0.15), 1.0)),
                ],
            );
        })?;
        let ring_surface = gfx.surface(ring_size, ring_size)?;

        let glow = scene::centered_visual(gfx, &glow_surface, glow_size, glow_size)?;
        let ring = scene::centered_visual(gfx, &ring_surface, ring_size, ring_size)?;
        let dot = scene::centered_visual(gfx, &dot_surface, dot_size, dot_size)?;
        let body = gfx.dcomp.CreateVisual()?;
        body.SetOpacityMode(DCOMPOSITION_OPACITY_MODE_MULTIPLY)?;
        body.AddVisual(&glow, false, None)?;
        body.AddVisual(&ring, true, &glow)?;
        body.AddVisual(&dot, true, &ring)?;
        let scale = gfx.dcomp.CreateScaleTransform()?;
        let shift = gfx.dcomp.CreateTranslateTransform()?;
        body.SetTransform(&scene::group(gfx, &scale, &shift)?)?;
        layer.AddVisual(&body, false, None)?;

        let mut timer = TimerSat {
            reminder: reminder.clone(),
            shift,
            scale,
            body: body.cast()?,
            glow: glow.cast()?,
            ring_surface,
            _surfaces: vec![glow_surface, dot_surface],
            _visuals: vec![body, glow, ring, dot],
            step: -1,
            x: Channel::fixed(geo.center.x),
            y: Channel::fixed(geo.center.y),
            size: Channel::fixed(0.0),
            alpha: Channel::fixed(0.0),
            shine: Channel::fixed(0.0),
        };
        timer.paint_ring(gfx, geo, reminders::now())?;
        Ok(timer)
    }
}

impl TimerSat {
    fn paint_ring(&mut self, gfx: &Gfx, geo: &Geometry, now: i64) -> Result<bool> {
        let step = steps(&self.reminder, geo, now);
        if step == self.step {
            return Ok(false);
        }
        self.step = step;
        let z = sizes(geo);
        let size = even(z.ring + z.ring_width);
        let fraction = (step as f64 / ring_length(geo)).min(1.0) as f32;
        gfx.draw(&self.ring_surface, size, size, |p| {
            p.stroke_circle(
                z.ring as f32,
                z.ring_width as f32,
                gfx::rgb(TIMER_COLOR, 0.22),
            );
            p.stroke_arc(
                z.ring as f32,
                z.ring_width as f32,
                fraction,
                gfx::rgb(TIMER_COLOR, 0.95),
            );
        })?;
        Ok(true)
    }
}

pub fn gap_phase(j: usize, k: usize, n: usize) -> f64 {
    if n == 0 {
        return TAU * j as f64 / k.max(1) as f64;
    }
    let gap = TAU / n as f64;
    if k <= n {
        return gap * ((j * n / k) as f64 + 0.5);
    }
    let (base, extra) = (k / n, k % n);
    let mut index = j;
    for g in 0..n {
        let count = base + usize::from(g < extra);
        if index < count {
            return gap * (g as f64 + (index + 1) as f64 / (count + 1) as f64);
        }
        index -= count;
    }
    0.0
}

impl Orbit {
    pub fn set_timers(&mut self, list: &[Reminder]) -> Result<()> {
        self.timer_total = list.len();
        let shown = &list[..list.len().min(MAX_SHOWN)];
        let mut old = std::mem::take(&mut self.timers);
        let mut changed = old.len() != shown.len();
        for reminder in shown {
            match old.iter().position(|v| v.reminder.id == reminder.id) {
                Some(index) => {
                    let mut view = old.remove(index);
                    changed |= index != 0 || view.reminder != *reminder;
                    view.reminder = reminder.clone();
                    self.timers.push(view);
                }
                None => {
                    changed = true;
                    let view = build(&self.gfx, &self.geo, &self.sat_layer, reminder)?;
                    self.timers.push(view);
                }
            }
        }
        for view in old {
            unsafe {
                let _ = self.sat_layer.RemoveVisual(&view.body);
            }
        }
        if !changed {
            return Ok(());
        }
        let sats = self.satellites.len();
        if self
            .card
            .as_ref()
            .and_then(|c| c.owner)
            .is_some_and(|o| o >= sats)
        {
            self.hide_card(now());
        }
        self.retarget_timers(now(), LAUNCH_SECS, false);
        if self.expanded {
            self.build_timer_labels()?;
            self.set_input_region();
        }
        self.schedule_ring();
        self.gfx.trim();
        self.apply_all()
    }

    fn timer_goal(&self, j: usize, arrive: f64) -> Goal {
        if self.expanded {
            let slot = self.timer_slot(j);
            return Goal {
                x: Target::hold(slot.x),
                y: Target::hold(slot.y),
                size: Target::hold(HOVER_SCALE),
                alpha: Target::hold(1.0),
            };
        }
        let phase = gap_phase(j, self.timers.len(), self.satellites.len());
        self.orbit_goal(phase, arrive)
    }

    pub(super) fn retarget_timers(&mut self, t: f64, dur: f64, kick: bool) {
        for j in 0..self.timers.len() {
            let goal = self.timer_goal(j, t + dur);
            let shine = self.twinkle_goal(self.satellites.len() + j);
            let v = &mut self.timers[j];
            v.x = v.x.to(t, 0.0, dur, goal.x, kick);
            v.y = v.y.to(t, 0.0, dur, goal.y, kick);
            v.size = v.size.to(t, 0.0, dur, goal.size, kick);
            v.alpha = v.alpha.to(t, 0.0, dur, goal.alpha, kick);
            v.shine = v.shine.to(t, 0.0, dur, shine, false);
        }
        if let Some(labels) = &mut self.timer_labels {
            let shown = if self.expanded { 1.0 } else { 0.0 };
            labels.alpha =
                labels
                    .alpha
                    .to(t, 0.0, dur.min(SPREAD_SECS), Target::hold(shown), false);
        }
    }

    pub(super) fn timer_slot(&self, j: usize) -> Vec2 {
        let s = self.geo.scale;
        let x = self.geo.layout_radius + (ROW_GAP + ROW_STEP * j as f64) * s;
        self.geo.center + Vec2::new(x, 0.0)
    }

    pub(super) fn timer_at(&self, point: Vec2) -> Option<usize> {
        let reach = sizes(&self.geo).ring * HOVER_SCALE + 8.0 * self.geo.scale;
        (0..self.timers.len())
            .find(|&j| {
                let d = self.timer_slot(j) - point;
                d.x * d.x + d.y * d.y <= reach * reach
            })
            .map(|j| self.satellites.len() + j)
    }

    pub(super) fn build_timer_labels(&mut self) -> Result<()> {
        let alpha = self.timer_labels.as_ref().map(|l| l.alpha);
        self.remove_timer_labels();
        if self.timers.is_empty() {
            return Ok(());
        }
        let now = reminders::now();
        let below = sizes(&self.geo).ring * HOVER_SCALE + self.geo.label_gap;
        let texts: Vec<String> = self
            .timers
            .iter()
            .map(|v| label_text(&v.reminder, now))
            .collect();
        let items: Vec<(&str, (f64, f64))> = texts
            .iter()
            .enumerate()
            .map(|(j, text)| {
                let offset = self.timer_slot(j) - self.geo.center;
                (text.as_str(), (offset.x, offset.y + below))
            })
            .collect();
        let mut labels = scene::build_labels(&self.gfx, &self.geo, &self.labels_holder, &items)?;
        labels.alpha = alpha.unwrap_or(Channel::fixed(0.0));
        self.timer_labels = Some(labels);
        Ok(())
    }

    fn remove_timer_labels(&mut self) {
        if let Some(labels) = self.timer_labels.take() {
            unsafe {
                let _ = self.labels_holder.RemoveVisual(&labels.layer);
            }
        }
    }

    pub(super) fn paint_timer_card(&self, index: usize) -> Result<()> {
        let (Some(card), Some(view)) = (&self.card, self.timers.get(index - self.satellites.len()))
        else {
            return Ok(());
        };
        let r = &view.reminder;
        let now = reminders::now();
        let title = if r.timer {
            tr("Таймер", "Timer")
        } else {
            tr("Напоминание", "Reminder")
        };
        let when = format!("{} {}", tr("в", "at"), reminders::due_text(r.due, now));
        let detail = match (r.timer, r.text.is_empty()) {
            (true, true) => format!(
                "{} {} · {when}",
                tr("из", "of"),
                reminders::duration_text(r.due - r.created)
            ),
            (_, false) => format!("{} · {when}", r.text),
            (false, true) => when,
        };
        let value = reminders::left_text(r.due - now);
        card.paint_lines(
            &self.gfx,
            &self.geo,
            title,
            &value,
            &detail,
            TIMER_COLOR,
            true,
        )
    }

    pub(super) fn timer_click(&mut self, point: Vec2) -> bool {
        let sats = self.satellites.len();
        let Some((card, owner)) = self
            .card
            .as_ref()
            .and_then(|c| c.owner.filter(|&o| o >= sats).map(|o| (c, o)))
        else {
            return false;
        };
        let local = point - Vec2::new(card.rect.0, card.rect.1);
        if !inside(local, scene::cancel_button(&self.geo)) {
            return false;
        }
        if let Some(view) = self.timers.get(owner - sats) {
            reminders::remove(view.reminder.id);
            self.notify_host();
        }
        true
    }

    pub(super) fn add_reminder(&self, request: &Request) {
        if let Some(reminder) = request.resolve(reminders::now())
            && reminders::add(reminder)
        {
            self.notify_host();
        }
    }

    fn notify_host(&self) {
        unsafe {
            let _ = PostMessageW(Some(self.host), WM_REMINDERS, WPARAM(0), LPARAM(0));
        }
    }

    pub(super) fn reminders_full(&self) -> bool {
        self.timer_total >= reminders::MAX_REMINDERS
    }

    pub fn timer_fired(&mut self, reminder: &Reminder, text: String) -> Result<()> {
        let t = now();
        let from = self
            .timers
            .iter()
            .find(|v| v.reminder.id == reminder.id)
            .map(|v| Vec2::new(v.x.sample(t).0, v.y.sample(t).0));
        let notice = info::Notice {
            kind: None,
            text,
            look: info::Look {
                color: TIMER_COLOR,
                progress: 0.0,
            },
            long: true,
        };
        self.show_notice_at(notice, from, true)
    }

    pub(super) fn schedule_ring(&self) {
        if self.timers.is_empty() {
            self.stop_timer(RING_TIMER);
            return;
        }
        let length = ring_length(&self.geo);
        let sats = self.satellites.len();
        let card_open = self
            .card
            .as_ref()
            .and_then(|c| c.owner)
            .is_some_and(|o| o >= sats);
        let ms = if self.expanded || card_open {
            MIN_STEP_MS
        } else {
            self.timers
                .iter()
                .map(|v| (v.reminder.due - v.reminder.created) as f64 * 1000.0 / length)
                .fold(MAX_STEP_MS, f64::min)
                .max(MIN_STEP_MS)
        };
        unsafe {
            SetTimer(Some(self.input_hwnd), RING_TIMER, ms as u32, None);
        }
    }

    pub(super) fn tick_timers(&mut self) -> Result<()> {
        let now = reminders::now();
        let mut changed = false;
        if !self.hidden {
            for view in &mut self.timers {
                changed |= view.paint_ring(&self.gfx, &self.geo, now)?;
            }
            if self.expanded {
                self.build_timer_labels()?;
                changed = true;
            }
            let sats = self.satellites.len();
            if let Some(owner) = self
                .card
                .as_ref()
                .and_then(|c| c.owner)
                .filter(|&o| o >= sats)
            {
                self.paint_timer_card(owner)?;
                changed = true;
            }
        }
        self.schedule_ring();
        if changed {
            self.gfx.trim();
            self.apply_all()?;
        }
        Ok(())
    }

    pub(super) fn apply_timers(&self) -> Result<()> {
        unsafe {
            for v in &self.timers {
                self.bind(&v.x, |a| v.shift.SetOffsetX(a), |x| v.shift.SetOffsetX2(x))?;
                self.bind(&v.y, |a| v.shift.SetOffsetY(a), |y| v.shift.SetOffsetY2(y))?;
                self.bind(&v.size, |a| v.scale.SetScaleX(a), |s| v.scale.SetScaleX2(s))?;
                self.bind(&v.size, |a| v.scale.SetScaleY(a), |s| v.scale.SetScaleY2(s))?;
                self.bind(
                    &v.alpha,
                    |a| v.body.SetOpacity(a),
                    |o| v.body.SetOpacity2(o),
                )?;
                self.bind(
                    &v.shine,
                    |a| v.glow.SetOpacity(a),
                    |o| v.glow.SetOpacity2(o),
                )?;
            }
            if let Some(labels) = &self.timer_labels {
                let core = &self.core;
                let shift = &labels.shift;
                self.bind(&core.x, |a| shift.SetOffsetX(a), |v| shift.SetOffsetX2(v))?;
                self.bind(&core.y, |a| shift.SetOffsetY(a), |v| shift.SetOffsetY2(v))?;
                self.bind(
                    &labels.alpha,
                    |a| labels.layer.SetOpacity(a),
                    |v| labels.layer.SetOpacity2(v),
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn timer_region(&self) -> Option<(i32, i32, i32, i32)> {
        let last = self.timers.len().checked_sub(1)?;
        let s = self.geo.scale;
        let c = self.geo.center;
        let left = c.x + self.geo.layout_radius;
        let right = self.timer_slot(last).x + 30.0 * s;
        let top = c.y - 14.0 * s;
        let bottom = c.y + 40.0 * s;
        Some((
            left as i32,
            top as i32,
            right.ceil() as i32,
            bottom.ceil() as i32,
        ))
    }
}

fn label_text(r: &Reminder, now: i64) -> String {
    if r.timer {
        reminders::left_text(r.due - now)
    } else {
        format!("{} {}", tr("в", "at"), reminders::due_text(r.due, now))
    }
}

#[cfg(test)]
mod tests {
    use super::gap_phase;
    use std::f64::consts::TAU;

    fn deg(j: usize, k: usize, n: usize) -> i64 {
        (gap_phase(j, k, n) * 360.0 / TAU).round() as i64
    }

    #[test]
    fn timers_take_the_middle_of_gaps() {
        assert_eq!(deg(0, 1, 3), 60);
        assert_eq!([deg(0, 3, 3), deg(1, 3, 3), deg(2, 3, 3)], [60, 180, 300]);
        assert_eq!([deg(0, 3, 6), deg(1, 3, 6), deg(2, 3, 6)], [30, 150, 270]);
        assert_eq!([deg(0, 3, 2), deg(1, 3, 2), deg(2, 3, 2)], [60, 120, 270]);
        assert_eq!([deg(0, 2, 0), deg(1, 2, 0)], [0, 180]);
    }
}
