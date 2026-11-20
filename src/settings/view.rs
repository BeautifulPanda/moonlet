use std::time::Instant;

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::DirectComposition::{
    IDCompositionAnimation, IDCompositionSurface, IDCompositionTarget, IDCompositionVisual2,
    IDCompositionVisual3,
};
use windows::core::{Interface, Result};

use super::field::Field;
use super::page::{Control, Row, Section};
use crate::gfx::{self, Gfx, Painter, TextBlock};
use crate::lang::tr;

const WIDTH: f32 = 720.0;
const HEIGHT: f32 = 520.0;
const SIDEBAR: f32 = 184.0;
const HEADER: f32 = 56.0;
const BUTTON_W: f32 = 46.0;
const BUTTON_H: f32 = 36.0;
const ITEMS_TOP: f32 = 60.0;
const ITEM_H: f32 = 34.0;
const PAD: f32 = 28.0;
const ROW_H: f32 = 44.0;
const ROW_HINT_H: f32 = 52.0;
const CAPTION_H: f32 = 24.0;
const GROUP_GAP: f32 = 16.0;
const OPTION_H: f32 = 30.0;
const INSET: f32 = 14.0;
const CONTROL_H: f32 = 26.0;
const ANIM_SECS: f32 = 0.18;
const BLINK_SECS: f64 = 5.0;
const BLINK_HALF: f64 = 0.5;

const BACK: u32 = 0x16181D;
const SIDE: u32 = 0x121418;
const SIDE_LINE: u32 = 0x22252C;
const GROUP: u32 = 0x1C1F25;
const HOVER: u32 = 0x22262D;
const LINE: u32 = 0x272B32;
const TEXT: u32 = 0xE6E8EE;
const DIM: u32 = 0x8C92A0;
const FAINT: u32 = 0x6F7584;
const MOON: u32 = 0xF2D27A;
const ON_MOON: u32 = 0x16181D;
const CONTROL: u32 = 0x2A2E36;
const CONTROL_HOVER: u32 = 0x343943;
const FIELD: u32 = 0x131519;
const FIELD_LINE: u32 = 0x31353E;
const FIELD_HOVER: u32 = 0x454A55;
const SELECTION: u32 = 0x5A5236;
const WARN: u32 = 0xFFA28A;
const CLOSE_RED: u32 = 0xC42B1C;

pub type Rect = (f32, f32, f32, f32);

pub fn contains(rect: Rect, x: f32, y: f32) -> bool {
    x >= rect.0 && x < rect.0 + rect.2 && y >= rect.1 && y < rect.1 + rect.3
}

pub fn window_size(scale: f32) -> (i32, i32) {
    (
        (WIDTH * scale).round() as i32,
        (HEIGHT * scale).round() as i32,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chrome {
    Item(usize),
    Minimize,
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Row,
    Segment(usize),
    Option(usize),
    Up,
    Down,
    Remove,
    Slider,
    Field,
    Hotkey,
    Button,
}

pub struct RowBox {
    pub y: f32,
    pub h: f32,
    pub group: Rect,
    pub first: bool,
    pub parts: Vec<(Part, Rect)>,
    pub track: (f32, f32),
    pub knob: Option<(f32, f32, f32)>,
    pub text_box: Rect,
    pub text_max: f32,
}

impl RowBox {
    pub fn rect(&self) -> Rect {
        (self.group.0, self.y, self.group.2, self.h)
    }
}

pub struct Layout {
    pub rows: Vec<RowBox>,
    pub height: f32,
}

impl Layout {
    pub fn hit(&self, rows: &[Row], x: f32, y: f32) -> Option<(usize, Part)> {
        let index = self.rows.iter().position(|b| y >= b.y && y < b.y + b.h)?;
        let b = &self.rows[index];
        if let Some(&(part, _)) = b.parts.iter().find(|(_, r)| contains(*r, x, y)) {
            return Some((index, part));
        }
        let whole = matches!(
            rows[index].control,
            Control::Toggle(_) | Control::Order { .. }
        );
        (whole && contains(b.rect(), x, y)).then_some((index, Part::Row))
    }
}

pub struct PageState<'a> {
    pub hover: Option<(usize, Part)>,
    pub focus: Option<usize>,
    pub focus_visible: bool,
    pub field: Option<&'a Field>,
    pub recording: bool,
}

#[derive(Clone, Copy)]
pub struct Tween {
    from: f32,
    pub to: f32,
    start: Instant,
    dur: f32,
}

impl Tween {
    pub fn fixed(value: f32) -> Self {
        Self::new(value, value, 0.0)
    }

    fn new(from: f32, to: f32, dur: f32) -> Self {
        Self {
            from,
            to,
            start: Instant::now(),
            dur,
        }
    }

    pub fn value(&self) -> f32 {
        let t = self.start.elapsed().as_secs_f32();
        if t >= self.dur {
            return self.to;
        }
        let u = 1.0 - t / self.dur;
        self.from + (self.to - self.from) * (1.0 - u * u * u)
    }

    pub fn go(&self, to: f32) -> Self {
        Self::new(self.value(), to, ANIM_SECS)
    }

    fn animation(&self, gfx: &Gfx) -> Result<Option<IDCompositionAnimation>> {
        if self.dur <= 0.0 || self.from == self.to {
            return Ok(None);
        }
        let (d, t) = (self.to - self.from, self.dur);
        unsafe {
            let animation = gfx.dcomp.CreateAnimation()?;
            animation.AddCubic(
                0.0,
                self.from,
                3.0 * d / t,
                -3.0 * d / (t * t),
                d / (t * t * t),
            )?;
            animation.End(t as f64, self.to)?;
            Ok(Some(animation))
        }
    }
}

fn offset_y(gfx: &Gfx, visual: &IDCompositionVisual2, tween: &Tween) -> Result<()> {
    unsafe {
        match tween.animation(gfx)? {
            Some(animation) => visual.SetOffsetY(&animation),
            None => visual.SetOffsetY2(tween.to),
        }
    }
}

fn offset_x(gfx: &Gfx, visual: &IDCompositionVisual2, tween: &Tween) -> Result<()> {
    unsafe {
        match tween.animation(gfx)? {
            Some(animation) => visual.SetOffsetX(&animation),
            None => visual.SetOffsetX2(tween.to),
        }
    }
}

struct Knob {
    visual: IDCompositionVisual2,
    on: bool,
    x: Tween,
}

pub struct View {
    pub scale: f32,
    pub width: f32,
    pub height: f32,
    chrome: IDCompositionSurface,
    marker: IDCompositionVisual2,
    marker_y: Tween,
    content: IDCompositionVisual2,
    content3: IDCompositionVisual3,
    page_visual: IDCompositionVisual2,
    page: Option<(IDCompositionSurface, u32)>,
    knob_on: IDCompositionSurface,
    knob_off: IDCompositionSurface,
    knobs: Vec<Option<Knob>>,
    caret: IDCompositionVisual2,
    caret3: IDCompositionVisual3,
    thumb: IDCompositionVisual2,
    thumb_surface: Option<IDCompositionSurface>,
    pub scroll: Tween,
    _visuals: Vec<IDCompositionVisual2>,
    _surfaces: Vec<IDCompositionSurface>,
    _target: IDCompositionTarget,
}

impl View {
    pub fn new(gfx: &Gfx, hwnd: HWND, scale: f32) -> Result<Self> {
        let s = scale;
        let (w, h) = window_size(s);
        let (width, height) = (w as f32, h as f32);
        unsafe {
            let dcomp = &gfx.dcomp;
            let target = dcomp.CreateTargetForHwnd(hwnd, true)?;
            let root = dcomp.CreateVisual()?;
            target.SetRoot(&root)?;

            let chrome = gfx.opaque_surface(w as u32, h as u32)?;
            let chrome_visual = dcomp.CreateVisual()?;
            chrome_visual.SetContent(&chrome)?;

            let (mw, mh) = (((SIDEBAR - 16.0) * s).round(), ((ITEM_H - 2.0) * s).round());
            let marker_surface = gfx.surface(mw as u32, mh as u32)?;
            gfx.draw(&marker_surface, mw as u32, mh as u32, |p| {
                p.rounded_rect(
                    (0.0, 0.0, mw, mh),
                    7.0 * s,
                    gfx::rgb(0xFFFFFF, 0.05),
                    none(),
                );
            })?;
            let marker = dcomp.CreateVisual()?;
            marker.SetContent(&marker_surface)?;
            marker.SetOffsetX2((8.0 * s).round())?;

            let viewport = dcomp.CreateVisual()?;
            let (left, top) = ((SIDEBAR * s).round(), (HEADER * s).round());
            viewport.SetOffsetX2(left)?;
            viewport.SetOffsetY2(top)?;
            let clip = D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: width - left,
                bottom: height - top,
            };
            viewport.SetClip2(&clip)?;
            let content = dcomp.CreateVisual()?;
            let page_visual = dcomp.CreateVisual()?;

            let knob_size = (14.0 * s).ceil() as u32;
            let knob = |color: u32| -> Result<IDCompositionSurface> {
                gfx.paint(knob_size, |p| p.fill_circle(6.0 * s, gfx::rgb(color, 1.0)))
            };
            let knob_on = knob(ON_MOON)?;
            let knob_off = knob(DIM)?;

            let (caret_w, caret_h) = ((1.5 * s).ceil().max(1.0) as u32, (16.0 * s).round() as u32);
            let caret_surface = gfx.surface(caret_w, caret_h)?;
            gfx.draw(&caret_surface, caret_w, caret_h, |p| {
                p.rounded_rect(
                    (0.0, 0.0, caret_w as f32, caret_h as f32),
                    0.0,
                    gfx::rgb(TEXT, 1.0),
                    none(),
                );
            })?;
            let caret = dcomp.CreateVisual()?;
            caret.SetContent(&caret_surface)?;
            let caret3: IDCompositionVisual3 = caret.cast()?;
            caret3.SetOpacity2(0.0)?;

            let thumb = dcomp.CreateVisual()?;
            thumb.SetOffsetX2(clip.right - (6.0 * s).round())?;

            root.AddVisual(&chrome_visual, false, None)?;
            root.AddVisual(&marker, true, &chrome_visual)?;
            root.AddVisual(&viewport, true, &marker)?;
            viewport.AddVisual(&content, false, None)?;
            viewport.AddVisual(&thumb, true, &content)?;
            content.AddVisual(&page_visual, false, None)?;
            content.AddVisual(&caret, true, &page_visual)?;

            let root3: IDCompositionVisual3 = root.cast()?;
            if let Some(animation) = Tween::new(0.0, 1.0, 0.15).animation(gfx)? {
                root3.SetOpacity(&animation)?;
            }

            Ok(Self {
                scale: s,
                width,
                height,
                chrome,
                marker: marker.clone(),
                marker_y: Tween::fixed(0.0),
                content3: content.cast()?,
                content: content.clone(),
                page_visual: page_visual.clone(),
                page: None,
                knob_on,
                knob_off,
                knobs: Vec::new(),
                caret: caret.clone(),
                caret3,
                thumb: thumb.clone(),
                thumb_surface: None,
                scroll: Tween::fixed(0.0),
                _visuals: vec![
                    root,
                    chrome_visual,
                    marker,
                    viewport,
                    content,
                    page_visual,
                    caret,
                    thumb,
                ],
                _surfaces: vec![marker_surface, caret_surface],
                _target: target,
            })
        }
    }

    pub fn commit(&self, gfx: &Gfx) {
        unsafe {
            let _ = gfx.dcomp.Commit();
        }
    }

    pub fn viewport(&self) -> (f32, f32) {
        let s = self.scale;
        (
            self.width - (SIDEBAR * s).round(),
            self.height - (HEADER * s).round(),
        )
    }

    pub fn to_page(&self, x: f32, y: f32) -> Option<(f32, f32)> {
        let s = self.scale;
        let (left, top) = ((SIDEBAR * s).round(), (HEADER * s).round());
        (x >= left && y >= top).then_some((x - left, y - top + self.scroll.to))
    }

    pub fn chrome_at(&self, x: f32, y: f32, sections: usize) -> Option<Chrome> {
        if contains(self.button(Chrome::Close), x, y) {
            return Some(Chrome::Close);
        }
        if contains(self.button(Chrome::Minimize), x, y) {
            return Some(Chrome::Minimize);
        }
        (0..sections)
            .find(|&i| contains(self.item(i), x, y))
            .map(Chrome::Item)
    }

    pub fn is_caption(&self, x: f32, y: f32) -> bool {
        y < HEADER * self.scale && self.chrome_at(x, y, 0).is_none()
    }

    fn item(&self, index: usize) -> Rect {
        let s = self.scale;
        (
            (8.0 * s).round(),
            ((ITEMS_TOP + index as f32 * ITEM_H) * s).round(),
            ((SIDEBAR - 16.0) * s).round(),
            ((ITEM_H - 2.0) * s).round(),
        )
    }

    fn button(&self, which: Chrome) -> Rect {
        let s = self.scale;
        let w = (BUTTON_W * s).round();
        let x = match which {
            Chrome::Close => self.width - w,
            _ => self.width - 2.0 * w,
        };
        (x, 0.0, w, (BUTTON_H * s).round())
    }

    pub fn paint_chrome(
        &self,
        gfx: &Gfx,
        sections: &[Section],
        selected: usize,
        hover: Option<Chrome>,
        part: Option<Chrome>,
    ) -> Result<()> {
        let s = self.scale;
        let (w, h) = (self.width, self.height);
        let side = (SIDEBAR * s).round();
        let logo = gfx.text("Moonlet", 14.0 * s, true)?;
        let title = gfx.text(sections[selected].title(), 15.0 * s, true)?;
        let items: Vec<_> = sections
            .iter()
            .map(|section| gfx.text(section.title(), 13.0 * s, false))
            .collect();
        let minimize = gfx.icon('\u{E921}', 10.0 * s)?;
        let close = gfx.icon('\u{E8BB}', 10.0 * s)?;
        let header_y = ((HEADER * s - title.height) / 2.0).round();
        let paint = |p: &Painter| {
            p.rounded_rect((0.0, 0.0, w, h), 0.0, gfx::rgb(BACK, 1.0), none());
            p.rounded_rect((0.0, 0.0, side, h), 0.0, gfx::rgb(SIDE, 1.0), none());
            p.rounded_rect(
                (side - 1.0, 0.0, 1.0, h),
                0.0,
                gfx::rgb(SIDE_LINE, 1.0),
                none(),
            );
            let dot = 4.5 * s;
            let x = (22.0 * s).round();
            let cy = header_y + logo.height / 2.0;
            p.rounded_rect(
                (x, cy - dot, 2.0 * dot, 2.0 * dot),
                dot,
                gfx::rgb(MOON, 1.0),
                none(),
            );
            p.text(
                &logo,
                (x + 2.0 * dot + 9.0 * s).round(),
                header_y,
                gfx::rgb(TEXT, 1.0),
            );
            p.text(
                &title,
                side + (PAD * s).round(),
                header_y,
                gfx::rgb(TEXT, 1.0),
            );
            for (index, (section, label)) in sections.iter().zip(&items).enumerate() {
                let (ix, iy, iw, ih) = self.item(index);
                if hover == Some(Chrome::Item(index)) && index != selected {
                    p.rounded_rect((ix, iy, iw, ih), 7.0 * s, gfx::rgb(0xFFFFFF, 0.03), none());
                }
                let r = 3.5 * s;
                let dx = (ix + 14.0 * s).round();
                let dy = iy + ih / 2.0;
                p.rounded_rect(
                    (dx, dy - r, 2.0 * r, 2.0 * r),
                    r,
                    gfx::rgb(section.color(), 1.0),
                    none(),
                );
                if let Ok(label) = label {
                    let color = if index == selected { TEXT } else { DIM };
                    p.text(
                        label,
                        (dx + 2.0 * r + 12.0 * s).round(),
                        (iy + (ih - label.height) / 2.0).round(),
                        gfx::rgb(color, 1.0),
                    );
                }
            }
            for (which, icon) in [(Chrome::Minimize, &minimize), (Chrome::Close, &close)] {
                let (bx, by, bw, bh) = self.button(which);
                let lit = hover == Some(which);
                if lit {
                    let color = if which == Chrome::Close {
                        CLOSE_RED
                    } else {
                        CONTROL
                    };
                    p.rounded_rect((bx, by, bw, bh), 0.0, gfx::rgb(color, 1.0), none());
                }
                p.text(
                    icon,
                    (bx + (bw - icon.width) / 2.0).round(),
                    (by + (bh - icon.height) / 2.0).round(),
                    gfx::rgb(if lit { TEXT } else { DIM }, 1.0),
                );
            }
        };
        let size = (w as u32, h as u32);
        match part {
            None => gfx.draw(&self.chrome, size.0, size.1, paint),
            Some(which) => {
                let (x, y, pw, ph) = match which {
                    Chrome::Item(i) => self.item(i),
                    other => self.button(other),
                };
                gfx.draw_part(
                    &self.chrome,
                    size,
                    (x as i32, y as i32, pw.ceil() as i32, ph.ceil() as i32),
                    paint,
                )
            }
        }
    }

    pub fn select(&mut self, gfx: &Gfx, index: usize, animate: bool) -> Result<()> {
        let y = self.item(index).1;
        self.marker_y = if animate {
            self.marker_y.go(y)
        } else {
            Tween::fixed(y)
        };
        offset_y(gfx, &self.marker, &self.marker_y)
    }

    pub fn layout(&self, gfx: &Gfx, rows: &[Row]) -> Layout {
        let s = self.scale;
        let (width, _) = self.viewport();
        let (gx, gw) = ((PAD * s).round(), (width - 2.0 * PAD * s).round());
        let right = gx + gw - (INSET * s).round();
        let text_width =
            |text: &str, size: f32| gfx.text(text, size, false).map_or(0.0, |t| t.width);
        let ch = (CONTROL_H * s).round();
        let mut y = (2.0 * s).round();
        let mut boxes: Vec<RowBox> = Vec::with_capacity(rows.len());
        let mut first = true;
        for (index, row) in rows.iter().enumerate() {
            if let Control::Header = row.control {
                if index > 0 {
                    y += (GROUP_GAP * s).round();
                }
                let h = (CAPTION_H * s).round();
                boxes.push(RowBox {
                    y,
                    h,
                    group: (gx, y, gw, h),
                    first: true,
                    parts: Vec::new(),
                    track: (0.0, 0.0),
                    knob: None,
                    text_box: (0.0, 0.0, 0.0, 0.0),
                    text_max: gw,
                });
                y += h;
                first = true;
                continue;
            }
            let base = match &row.control {
                Control::List(..) if row.label.is_empty() => 8.0,
                _ if row.hint.is_empty() => ROW_H,
                _ => ROW_HINT_H,
            };
            let base = match &row.control {
                Control::Picks(_) if row.label.is_empty() => 8.0,
                _ => base,
            };
            let h = match &row.control {
                Control::List(options, _) => base + options.len() as f32 * OPTION_H,
                Control::Picks(options) => base + options.len() as f32 * OPTION_H,
                _ => base,
            };
            let mut h = (h * s).round();
            if let Control::Note { strong } = row.control {
                let size = 12.0 * s;
                let text_h = gfx
                    .text_wrap(&row.label, size, note_width(gw, strong, s))
                    .map_or(16.0 * s, |t| t.height);
                let top = if strong { 10.0 } else { 3.0 } * s;
                h = (top + text_h + 3.0 * s).round();
                if rows
                    .get(index + 1)
                    .is_none_or(|next| !matches!(next.control, Control::Note { .. }))
                {
                    h += (10.0 * s).round();
                }
            }
            let cy = y + (base * s / 2.0).round();
            let mut parts = Vec::new();
            let mut track = (0.0, 0.0);
            let mut knob = None;
            let mut text_box = (0.0, 0.0, 0.0, 0.0);
            let toggle_knob = Some((right - 26.0 * s, right - 10.0 * s, cy));
            match &row.control {
                Control::Toggle(_) => knob = toggle_knob,
                Control::Order { .. } => {
                    knob = toggle_knob;
                    let down = right - 36.0 * s - 14.0 * s - ch;
                    let up = down - 2.0 * s - ch;
                    parts.push((Part::Up, (up, cy - ch / 2.0, ch, ch)));
                    parts.push((Part::Down, (down, cy - ch / 2.0, ch, ch)));
                }
                Control::Choice(options, _) => {
                    let widths: Vec<f32> = options
                        .iter()
                        .map(|o| (text_width(o, 12.0 * s) + 20.0 * s).round())
                        .collect();
                    let gap = (2.0 * s).round();
                    let total: f32 = widths.iter().sum::<f32>() + gap * (widths.len() - 1) as f32;
                    let mut x = right - total;
                    for (i, w) in widths.iter().enumerate() {
                        parts.push((Part::Segment(i), (x, cy - ch / 2.0, *w, ch)));
                        x += w + gap;
                    }
                }
                Control::Tile { .. } => {
                    let remove = right - ch;
                    let down = remove - 2.0 * s - ch;
                    let up = down - 2.0 * s - ch;
                    parts.push((Part::Up, (up, cy - ch / 2.0, ch, ch)));
                    parts.push((Part::Down, (down, cy - ch / 2.0, ch, ch)));
                    parts.push((Part::Remove, (remove, cy - ch / 2.0, ch, ch)));
                }
                Control::List(options, _) => {
                    let top = y + (base * s).round() - (4.0 * s).round();
                    for i in 0..options.len() {
                        let oy = (top + i as f32 * OPTION_H * s).round();
                        parts.push((
                            Part::Option(i),
                            (gx + 6.0 * s, oy, gw - 12.0 * s, (OPTION_H * s).round()),
                        ));
                    }
                }
                Control::Picks(options) => {
                    let top = y + (base * s).round() - (4.0 * s).round();
                    for i in 0..options.len() {
                        let oy = (top + i as f32 * OPTION_H * s).round();
                        parts.push((
                            Part::Option(i),
                            (gx + 6.0 * s, oy, gw - 12.0 * s, (OPTION_H * s).round()),
                        ));
                    }
                }
                Control::Slider(_) => {
                    let w = (160.0 * s).round();
                    track = ((right - 7.0 * s - w).round(), w);
                    parts.push((
                        Part::Slider,
                        (track.0 - 10.0 * s, cy - 12.0 * s, w + 20.0 * s, 24.0 * s),
                    ));
                }
                Control::Button(label) => {
                    let w = (text_width(label, 12.0 * s) + 28.0 * s).round();
                    parts.push((Part::Button, (right - w, cy - ch / 2.0, w, ch)));
                }
                Control::Hotkey(name) => {
                    let longest = text_width(name, 12.0 * s).max(text_width(
                        tr("Нажмите сочетание…", "Press a combination…"),
                        12.0 * s,
                    ));
                    let w = (longest + 44.0 * s).round();
                    parts.push((Part::Hotkey, (right - w, cy - ch / 2.0, w, ch)));
                }
                Control::Field(_) => {
                    let w = (280.0 * s).round();
                    let rect = (right - w, cy - ch / 2.0, w, ch);
                    parts.push((Part::Field, rect));
                    text_box = (rect.0 + 10.0 * s, rect.1, w - 20.0 * s, rect.3);
                }
                Control::Header | Control::Info(_) | Control::Note { .. } => {}
            }
            let control_left = match &row.control {
                Control::Toggle(_) => right - 36.0 * s,
                Control::Slider(_) => track.0 - 64.0 * s,
                Control::Info(text) => right - text_width(text, 12.0 * s),
                Control::List(..) | Control::Picks(_) | Control::Header | Control::Note { .. } => {
                    right
                }
                _ => parts.first().map_or(right, |(_, r)| r.0),
            };
            let text_max = control_left - 16.0 * s - (gx + INSET * s);
            boxes.push(RowBox {
                y,
                h,
                group: (gx, y, gw, h),
                first,
                parts,
                track,
                knob,
                text_box,
                text_max,
            });
            first = false;
            y += h;
        }
        let mut start = 0;
        while start < boxes.len() {
            if matches!(rows[start].control, Control::Header) {
                start += 1;
                continue;
            }
            let mut end = start + 1;
            while end < boxes.len() && !boxes[end].first {
                end += 1;
            }
            let top = boxes[start].y;
            let bottom = boxes[end - 1].y + boxes[end - 1].h;
            for b in &mut boxes[start..end] {
                b.group = (gx, top, gw, bottom - top);
            }
            start = end;
        }
        Layout {
            rows: boxes,
            height: (y + PAD * s).round(),
        }
    }

    pub fn paint_page(
        &mut self,
        gfx: &Gfx,
        rows: &[Row],
        layout: &Layout,
        state: &PageState,
        part: Option<&[usize]>,
    ) -> Result<()> {
        let s = self.scale;
        let (width, view_h) = self.viewport();
        let height = layout.height.max(view_h).ceil() as u32;
        if self.page.as_ref().map(|(_, h)| *h) != Some(height) {
            let surface = gfx.opaque_surface(width as u32, height)?;
            unsafe { self.page_visual.SetContent(&surface)? };
            self.page = Some((surface, height));
            return self.paint_page(gfx, rows, layout, state, None);
        }
        let Some((surface, _)) = &self.page else {
            return Ok(());
        };
        let size = (width as u32, height);
        let bands: Vec<(f32, f32)> = match part {
            None => vec![(0.0, height as f32)],
            Some(list) => list
                .iter()
                .filter_map(|&i| layout.rows.get(i))
                .map(|b| ((b.y - 3.0).max(0.0), (b.y + b.h + 3.0).min(height as f32)))
                .collect(),
        };
        let touches = |b: &RowBox| {
            bands
                .iter()
                .any(|&(top, bottom)| b.y < bottom && b.y + b.h > top)
        };
        let texts: Vec<Option<RowText>> = rows
            .iter()
            .zip(&layout.rows)
            .map(|(row, b)| {
                if touches(b) {
                    RowText::new(gfx, row, b, s)
                } else {
                    None
                }
            })
            .collect();
        let paint = |p: &Painter| {
            p.rounded_rect(
                (0.0, 0.0, width, height as f32),
                0.0,
                gfx::rgb(BACK, 1.0),
                none(),
            );
            for (index, (row, b)) in rows.iter().zip(&layout.rows).enumerate() {
                if b.first && !matches!(row.control, Control::Header) {
                    p.rounded_rect(b.group, 8.0 * s, gfx::rgb(GROUP, 1.0), none());
                }
                if let Some(text) = &texts[index] {
                    paint_row(p, gfx, s, row, b, text, state, index);
                }
            }
        };
        if part.is_none() {
            return gfx.draw(surface, size.0, size.1, paint);
        }
        for (top, bottom) in bands {
            let rect = (0, top as i32, width as i32, (bottom - top).ceil() as i32);
            gfx.draw_part(surface, size, rect, paint)?;
        }
        Ok(())
    }

    pub fn show_page(&mut self, gfx: &Gfx) -> Result<()> {
        for knob in self.knobs.drain(..).flatten() {
            unsafe {
                let _ = self.content.RemoveVisual(&knob.visual);
            }
        }
        self.scroll = Tween::fixed(0.0);
        unsafe {
            self.content.SetOffsetY2(0.0)?;
            offset_y(
                gfx,
                &self.page_visual,
                &Tween::new(8.0 * self.scale, 0.0, ANIM_SECS),
            )?;
            if let Some(animation) = Tween::new(0.0, 1.0, ANIM_SECS).animation(gfx)? {
                self.content3.SetOpacity(&animation)?;
            }
        }
        Ok(())
    }

    pub fn place_knobs(&mut self, gfx: &Gfx, rows: &[Row], layout: &Layout) -> Result<()> {
        let s = self.scale;
        let half = (14.0 * s).ceil() / 2.0;
        let keep = rows.len().min(self.knobs.len());
        for knob in self.knobs.drain(keep..).flatten() {
            unsafe {
                let _ = self.content.RemoveVisual(&knob.visual);
            }
        }
        self.knobs.resize_with(rows.len(), || None);
        for (index, (row, b)) in rows.iter().zip(&layout.rows).enumerate() {
            let on = match row.control {
                Control::Toggle(on) | Control::Order { on, .. } => Some(on),
                _ => None,
            };
            let (Some(on), Some((off_x, on_x, y))) = (on, b.knob) else {
                if let Some(knob) = self.knobs[index].take() {
                    unsafe {
                        let _ = self.content.RemoveVisual(&knob.visual);
                    }
                }
                continue;
            };
            let x = (if on { on_x } else { off_x } - half).round();
            let y = (y - half).round();
            let surface = if on { &self.knob_on } else { &self.knob_off };
            unsafe {
                match &mut self.knobs[index] {
                    Some(knob) => {
                        if knob.on != on {
                            knob.x = knob.x.go(x);
                            knob.on = on;
                            knob.visual.SetContent(surface)?;
                            offset_x(gfx, &knob.visual, &knob.x)?;
                        }
                        knob.visual.SetOffsetY2(y)?;
                    }
                    slot => {
                        let visual = gfx.dcomp.CreateVisual()?;
                        visual.SetContent(surface)?;
                        visual.SetOffsetX2(x)?;
                        visual.SetOffsetY2(y)?;
                        self.content.AddVisual(&visual, true, &self.page_visual)?;
                        *slot = Some(Knob {
                            visual,
                            on,
                            x: Tween::fixed(x),
                        });
                    }
                }
                if let Some(knob) = &self.knobs[index] {
                    let visual: IDCompositionVisual3 = knob.visual.cast()?;
                    visual.SetOpacity2(if row.dim { 0.45 } else { 1.0 })?;
                }
            }
        }
        Ok(())
    }

    pub fn scroll_to(&mut self, gfx: &Gfx, y: f32, page_height: f32) -> Result<()> {
        let from = self.scroll.value();
        self.scroll = self.scroll.go(y);
        offset_y(gfx, &self.content, &Tween::new(-from, -y, ANIM_SECS))?;
        self.place_thumb(gfx, page_height, true)
    }

    pub fn place_thumb(&mut self, gfx: &Gfx, page_height: f32, animate: bool) -> Result<()> {
        let s = self.scale;
        let (_, view_h) = self.viewport();
        let w = (3.0 * s).ceil();
        unsafe {
            if page_height <= view_h + 1.0 {
                self.thumb.SetContent(None)?;
                self.thumb_surface = None;
                return Ok(());
            }
            let h = (view_h * view_h / page_height).round().max(24.0 * s);
            if self.thumb_surface.is_none() {
                let surface = gfx.surface(w as u32, h as u32)?;
                gfx.draw(&surface, w as u32, h as u32, |p| {
                    p.rounded_rect((0.0, 0.0, w, h), w / 2.0, gfx::rgb(0xFFFFFF, 0.18), none());
                })?;
                self.thumb.SetContent(&surface)?;
                self.thumb_surface = Some(surface);
            }
            let range = view_h - h;
            let max_scroll = page_height - view_h;
            let at = |scroll: f32| range * (scroll / max_scroll).clamp(0.0, 1.0);
            let from = if animate {
                at(self.scroll.value())
            } else {
                at(self.scroll.to)
            };
            let dur = if animate { ANIM_SECS } else { 0.0 };
            offset_y(gfx, &self.thumb, &Tween::new(from, at(self.scroll.to), dur))
        }
    }

    pub fn reset_thumb(&mut self) {
        self.thumb_surface = None;
    }

    pub fn place_caret(&self, gfx: &Gfx, at: Option<(f32, f32)>) -> Result<()> {
        unsafe {
            match at {
                Some((x, y)) => {
                    self.caret.SetOffsetX2(x.round())?;
                    self.caret.SetOffsetY2(y.round())?;
                    self.caret3.SetOpacity(&blink(gfx)?)?;
                }
                None => self.caret3.SetOpacity2(0.0)?,
            }
        }
        Ok(())
    }

    pub fn caret_at(&self, gfx: &Gfx, b: &RowBox, field: &Field) -> Option<(f32, f32)> {
        let s = self.scale;
        let (x, scroll) = field_scroll(gfx, b, field, s)?;
        let caret_h = (16.0 * s).round();
        Some((
            b.text_box.0 + x - scroll,
            b.text_box.1 + (b.text_box.3 - caret_h) / 2.0,
        ))
    }
}

fn field_scroll(gfx: &Gfx, b: &RowBox, field: &Field, s: f32) -> Option<(f32, f32)> {
    let text: String = field.text.iter().collect();
    let block = gfx.text(&text, 12.0 * s, false).ok()?;
    let before: usize = field.text[..field.caret]
        .iter()
        .map(|c| c.len_utf16())
        .sum();
    let x = block.caret_x(before as u32);
    Some((x, (x - b.text_box.2 + 2.0).max(0.0)))
}

fn none() -> (f32, D2D1_COLOR_F) {
    (0.0, gfx::rgb(0, 0.0))
}

struct RowText {
    label: TextBlock,
    hint: Option<TextBlock>,
    control: Vec<TextBlock>,
}

impl RowText {
    fn new(gfx: &Gfx, row: &Row, b: &RowBox, s: f32) -> Option<Self> {
        let caption = matches!(row.control, Control::Header);
        let size = if caption { 11.0 } else { 13.0 } * s;
        let max = Some(b.text_max.max(40.0));
        let label = match row.control {
            Control::Note { strong } => gfx
                .text_wrap(&row.label, 12.0 * s, note_width(b.group.2, strong, s))
                .ok()?,
            _ => gfx.text_fit(&row.label, size, false, max).ok()?,
        };
        let hint = (!row.hint.is_empty())
            .then(|| gfx.text_fit(&row.hint, 11.0 * s, false, max).ok())
            .flatten();
        let texts: Vec<String> = match &row.control {
            Control::Choice(options, _) | Control::List(options, _) => {
                options.iter().map(|o| o.to_string()).collect()
            }
            Control::Picks(options) => options.clone(),
            Control::Tile { .. } => vec![
                '\u{E70E}'.to_string(),
                '\u{E70D}'.to_string(),
                '\u{E711}'.to_string(),
            ],
            Control::Button(label) => vec![label.to_string()],
            Control::Hotkey(name) => vec![
                name.clone(),
                tr("Нажмите сочетание…", "Press a combination…").to_string(),
            ],
            Control::Info(text) => vec![text.clone()],
            Control::Order { .. } => vec!['\u{E70E}'.to_string(), '\u{E70D}'.to_string()],
            Control::Slider(slider) => vec![format!(
                "{} {}",
                crate::config::number(slider.value),
                slider.unit
            )],
            _ => Vec::new(),
        };
        let control = texts
            .iter()
            .filter_map(|text| match row.control {
                Control::Order { .. } | Control::Tile { .. } => {
                    gfx.icon(text.chars().next()?, 10.0 * s).ok()
                }
                Control::Picks(_) => gfx
                    .text_fit(text, 12.0 * s, false, Some(b.group.2 - 60.0 * s))
                    .ok(),
                _ => gfx.text(text, 12.0 * s, false).ok(),
            })
            .collect();
        Some(Self {
            label,
            hint,
            control,
        })
    }
}

const NOTE_INDENT: f32 = 14.0;

fn note_width(group_w: f32, strong: bool, s: f32) -> f32 {
    let indent = if strong { 0.0 } else { NOTE_INDENT };
    group_w - (2.0 * INSET + indent) * s
}

fn centered(p: &Painter, block: &TextBlock, r: Rect, color: D2D1_COLOR_F) {
    p.text(
        block,
        (r.0 + (r.2 - block.width) / 2.0).round(),
        (r.1 + (r.3 - block.height) / 2.0).round(),
        color,
    );
}

#[allow(clippy::too_many_arguments)]
fn paint_row(
    p: &Painter,
    gfx: &Gfx,
    s: f32,
    row: &Row,
    b: &RowBox,
    text: &RowText,
    state: &PageState,
    index: usize,
) {
    let alpha = if row.dim { 0.45 } else { 1.0 };
    let c = |hex: u32, a: f32| gfx::rgb(hex, a * alpha);
    let hover = state
        .hover
        .filter(|(i, _)| *i == index)
        .map(|(_, part)| part);
    let focused = state.focus == Some(index);
    let rect = b.rect();
    if let Control::Header = row.control {
        p.text(
            &text.label,
            rect.0 + 2.0 * s,
            (rect.1 + rect.3 - text.label.height - 6.0 * s).round(),
            gfx::rgb(FAINT, 1.0),
        );
        return;
    }
    let toggles = matches!(row.control, Control::Toggle(_) | Control::Order { .. });
    if toggles && hover.is_some() {
        p.clip(rect, || {
            p.rounded_rect(b.group, 8.0 * s, gfx::rgb(HOVER, 1.0), none());
        });
    }
    if let Control::Note { strong } = row.control {
        let top = (rect.1 + if strong { 10.0 } else { 3.0 } * s).round();
        let x = rect.0 + (INSET * s).round();
        if strong {
            p.text(&text.label, x, top, gfx::rgb(TEXT, 1.0));
        } else {
            if let Ok(bullet) = gfx.text("•", 12.0 * s, false) {
                p.text(&bullet, x, top, gfx::rgb(FAINT, 1.0));
            }
            p.text(
                &text.label,
                x + (NOTE_INDENT * s).round(),
                top,
                gfx::rgb(DIM, 1.0),
            );
        }
        return;
    }
    if !b.first {
        p.rounded_rect(
            (
                rect.0 + (INSET * s).round(),
                rect.1,
                rect.2 - 2.0 * (INSET * s).round(),
                1.0,
            ),
            0.0,
            gfx::rgb(LINE, 1.0),
            none(),
        );
    }
    if focused && state.focus_visible {
        let ring = gfx::rgb(MOON, 0.85);
        let line = (1.5 * s).round().max(1.0);
        p.clip(rect, || {
            let (gx, gy, gw, gh) = b.group;
            p.rounded_rect(
                (gx + 1.0, gy + 1.0, gw - 2.0, gh - 2.0),
                7.0 * s,
                gfx::rgb(0, 0.0),
                (line, ring),
            );
        });
        if !b.first {
            p.rounded_rect((rect.0, rect.1, rect.2, line), 0.0, ring, none());
        }
        if b.y + b.h < b.group.1 + b.group.3 - 0.5 {
            p.rounded_rect(
                (rect.0, rect.1 + rect.3 - line, rect.2, line),
                0.0,
                ring,
                none(),
            );
        }
    }
    let x = rect.0 + (INSET * s).round();
    let base = (if row.hint.is_empty() {
        ROW_H
    } else {
        ROW_HINT_H
    } * s)
        .round();
    match &text.hint {
        Some(hint) => {
            let gap = (2.0 * s).round();
            let y = rect.1 + ((base - text.label.height - gap - hint.height) / 2.0).round();
            p.text(&text.label, x, y, c(TEXT, 1.0));
            let color = if row.warn { c(WARN, 1.0) } else { c(DIM, 1.0) };
            p.text(hint, x, y + text.label.height + gap, color);
        }
        None => p.text(
            &text.label,
            x,
            rect.1 + ((base - text.label.height) / 2.0).round(),
            c(TEXT, 1.0),
        ),
    }
    let cy = rect.1 + (base / 2.0).round();
    let right = rect.0 + rect.2 - (INSET * s).round();
    match &row.control {
        Control::Toggle(on) => toggle_track(p, s, right, cy, *on, alpha),
        Control::Order { on, up, down } => {
            toggle_track(p, s, right, cy, *on, alpha);
            for (part, enabled, icon) in [(Part::Up, *up, 0), (Part::Down, *down, 1)] {
                let Some(&(_, r)) = b.parts.iter().find(|(p, _)| *p == part) else {
                    continue;
                };
                if enabled && hover == Some(part) {
                    p.rounded_rect(r, 5.0 * s, c(CONTROL_HOVER, 1.0), none());
                }
                if let Some(icon) = text.control.get(icon) {
                    let color = if enabled { c(TEXT, 1.0) } else { c(FAINT, 0.5) };
                    centered(p, icon, r, color);
                }
            }
        }
        Control::Tile { up, down } => {
            for (i, (part, enabled)) in [(Part::Up, *up), (Part::Down, *down), (Part::Remove, true)]
                .into_iter()
                .enumerate()
            {
                let Some(&(_, r)) = b.parts.iter().find(|(p, _)| *p == part) else {
                    continue;
                };
                if enabled && hover == Some(part) {
                    p.rounded_rect(r, 5.0 * s, c(CONTROL_HOVER, 1.0), none());
                }
                if let Some(icon) = text.control.get(i) {
                    let color = if !enabled {
                        c(FAINT, 0.5)
                    } else if part == Part::Remove && hover == Some(part) {
                        c(WARN, 1.0)
                    } else {
                        c(TEXT, 1.0)
                    };
                    centered(p, icon, r, color);
                }
            }
        }
        Control::Picks(_) => {
            let plus = gfx.icon('\u{E710}', 10.0 * s).ok();
            for (i, (_, r)) in b.parts.iter().enumerate() {
                let hovered = hover == Some(Part::Option(i));
                if hovered {
                    p.rounded_rect(*r, 6.0 * s, c(HOVER, 1.0), none());
                }
                let ry = r.1 + r.3 / 2.0;
                let color = if hovered { c(MOON, 1.0) } else { c(DIM, 1.0) };
                if let Some(plus) = &plus {
                    centered(p, plus, (r.0 + 6.0 * s, r.1, 20.0 * s, r.3), color);
                }
                if let Some(label) = text.control.get(i) {
                    let color = if hovered { c(TEXT, 1.0) } else { c(DIM, 1.0) };
                    p.text(
                        label,
                        (r.0 + 34.0 * s).round(),
                        (ry - label.height / 2.0).round(),
                        color,
                    );
                }
            }
        }
        Control::Choice(_, selected) => {
            for (i, (_, r)) in b.parts.iter().enumerate() {
                let picked = *selected == Some(i);
                if picked {
                    p.rounded_rect(*r, 5.0 * s, c(MOON, 1.0), none());
                } else if hover == Some(Part::Segment(i)) {
                    p.rounded_rect(*r, 5.0 * s, c(CONTROL, 1.0), none());
                }
                if let Some(label) = text.control.get(i) {
                    let color = if picked { c(ON_MOON, 1.0) } else { c(DIM, 1.0) };
                    centered(p, label, *r, color);
                }
            }
        }
        Control::List(_, selected) => {
            for (i, (_, r)) in b.parts.iter().enumerate() {
                if hover == Some(Part::Option(i)) {
                    p.rounded_rect(*r, 6.0 * s, c(HOVER, 1.0), none());
                }
                let (rx, ry, radius) = (r.0 + 16.0 * s, r.1 + r.3 / 2.0, 6.5 * s);
                let picked = *selected == i;
                let ring = if picked { MOON } else { FIELD_LINE };
                p.rounded_rect(
                    (rx - radius, ry - radius, 2.0 * radius, 2.0 * radius),
                    radius,
                    gfx::rgb(0, 0.0),
                    (1.5 * s, c(ring, 1.0)),
                );
                if picked {
                    let dot = 3.0 * s;
                    p.rounded_rect(
                        (rx - dot, ry - dot, 2.0 * dot, 2.0 * dot),
                        dot,
                        c(MOON, 1.0),
                        none(),
                    );
                }
                if let Some(label) = text.control.get(i) {
                    let color = if picked { c(TEXT, 1.0) } else { c(DIM, 1.0) };
                    p.text(
                        label,
                        (r.0 + 34.0 * s).round(),
                        (ry - label.height / 2.0).round(),
                        color,
                    );
                }
            }
        }
        Control::Slider(slider) => {
            let (tx, tw) = b.track;
            let fx = tx + tw * slider.fraction();
            let h = (3.0 * s).round();
            let ty = (cy - h / 2.0).round();
            p.rounded_rect((tx, ty, tw, h), h / 2.0, c(CONTROL_HOVER, 1.0), none());
            p.rounded_rect((tx, ty, fx - tx, h), h / 2.0, c(MOON, 1.0), none());
            let r = if hover == Some(Part::Slider) {
                8.0
            } else {
                7.0
            } * s;
            p.rounded_rect((fx - r, cy - r, 2.0 * r, 2.0 * r), r, c(TEXT, 1.0), none());
            if let Some(value) = text.control.first() {
                p.text(
                    value,
                    (tx - 16.0 * s - value.width).round(),
                    (cy - value.height / 2.0).round(),
                    c(DIM, 1.0),
                );
            }
        }
        Control::Button(_) => {
            if let (Some(&(_, r)), Some(label)) = (b.parts.first(), text.control.first()) {
                let fill = if hover == Some(Part::Button) {
                    CONTROL_HOVER
                } else {
                    CONTROL
                };
                p.rounded_rect(r, 5.0 * s, c(fill, 1.0), none());
                centered(p, label, r, c(TEXT, 1.0));
            }
        }
        Control::Hotkey(_) => {
            let Some(&(_, r)) = b.parts.first() else {
                return;
            };
            let recording = focused && state.recording;
            let border = if recording {
                MOON
            } else if hover == Some(Part::Hotkey) {
                FIELD_HOVER
            } else {
                FIELD_LINE
            };
            p.rounded_rect(r, 5.0 * s, c(FIELD, 1.0), (1.0, c(border, 1.0)));
            if let Ok(icon) = gfx.icon('\u{E765}', 13.0 * s) {
                p.text(
                    &icon,
                    (r.0 + 10.0 * s).round(),
                    (r.1 + (r.3 - icon.height) / 2.0).round(),
                    c(FAINT, 1.0),
                );
            }
            let (label, color) = if recording {
                (text.control.get(1), c(MOON, 1.0))
            } else {
                (text.control.first(), c(TEXT, 1.0))
            };
            if let Some(label) = label {
                p.text(
                    label,
                    (r.0 + 32.0 * s).round(),
                    (r.1 + (r.3 - label.height) / 2.0).round(),
                    color,
                );
            }
        }
        Control::Field(value) => {
            let Some(&(_, r)) = b.parts.first() else {
                return;
            };
            let editing = state.field.filter(|_| focused);
            let border = if editing.is_some() {
                MOON
            } else if hover == Some(Part::Field) {
                FIELD_HOVER
            } else {
                FIELD_LINE
            };
            p.rounded_rect(r, 5.0 * s, c(FIELD, 1.0), (1.0, c(border, 1.0)));
            let (shown, scroll, all) = match editing {
                Some(field) => (
                    field.value(),
                    field_scroll(gfx, b, field, s).map_or(0.0, |(_, scroll)| scroll),
                    field.all,
                ),
                None => (value.clone(), 0.0, false),
            };
            let Ok(block) = gfx.text(&shown, 12.0 * s, false) else {
                return;
            };
            let tb = b.text_box;
            p.clip((tb.0 - 2.0, tb.1, tb.2 + 4.0, tb.3), || {
                let y = (tb.1 + (tb.3 - block.height) / 2.0).round();
                if all {
                    p.rounded_rect(
                        (tb.0 - scroll, y, block.width, block.height),
                        2.0 * s,
                        c(SELECTION, 1.0),
                        none(),
                    );
                }
                p.text(&block, (tb.0 - scroll).round(), y, c(TEXT, 1.0));
            });
        }
        Control::Info(_) => {
            if let Some(info) = text.control.first() {
                p.text(
                    info,
                    (right - info.width).round(),
                    (cy - info.height / 2.0).round(),
                    c(DIM, 1.0),
                );
            }
        }
        Control::Header | Control::Note { .. } => {}
    }
}

fn toggle_track(p: &Painter, s: f32, right: f32, cy: f32, on: bool, alpha: f32) {
    let (w, h) = ((36.0 * s).round(), (20.0 * s).round());
    let rect = (right - w, (cy - h / 2.0).round(), w, h);
    if on {
        p.rounded_rect(rect, h / 2.0, gfx::rgb(MOON, alpha), none());
    } else {
        p.rounded_rect(
            (rect.0 + 0.5, rect.1 + 0.5, w - 1.0, h - 1.0),
            h / 2.0,
            gfx::rgb(0, 0.0),
            (1.0, gfx::rgb(0x4A4F5A, alpha)),
        );
    }
}

fn blink(gfx: &Gfx) -> Result<IDCompositionAnimation> {
    unsafe {
        let animation = gfx.dcomp.CreateAnimation()?;
        let steps = (BLINK_SECS / BLINK_HALF) as usize;
        for step in 0..steps {
            let value = if step % 2 == 0 { 1.0 } else { 0.0 };
            animation.AddCubic(step as f64 * BLINK_HALF, value, 0.0, 0.0, 0.0)?;
        }
        animation.End(BLINK_SECS, 1.0)?;
        Ok(animation)
    }
}
