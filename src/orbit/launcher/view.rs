use windows::Win32::Graphics::DirectComposition::{
    DCOMPOSITION_OPACITY_MODE_MULTIPLY, IDCompositionAnimation, IDCompositionScaleTransform,
    IDCompositionSurface, IDCompositionVisual2, IDCompositionVisual3,
};
use windows::core::{Interface, Result};

use super::super::Geometry;
use super::super::motion::Channel;
use super::super::scene::{self, DIM_TEXT_COLOR, PANEL_COLOR, TEXT_COLOR};
use super::search::{Item, MAX_ITEMS};
use crate::gfx::{self, Gfx};
use crate::lang::tr;

const WIDTH: f64 = 520.0;
const FIELD: f64 = 48.0;
const ROW: f64 = 36.0;
const PAD: f64 = 6.0;
pub const GAP: f64 = 6.0;
const BLINK_SECS: f64 = 5.0;
const BLINK_HALF: f64 = 0.5;

pub fn extent(scale: f64) -> (f64, f64) {
    (
        (WIDTH * scale).round(),
        Layout::with_scale(scale, 0.0).height(MAX_ITEMS),
    )
}

#[derive(Clone, Copy)]
pub struct Layout {
    pub width: f64,
    field: f64,
    row: f64,
    pad: f64,
    scale: f64,
    pub top: f64,
}

impl Layout {
    pub fn new(geo: &Geometry) -> Self {
        Self::with_scale(geo.scale, geo.hover_radius + GAP * geo.scale)
    }

    fn with_scale(scale: f64, top: f64) -> Self {
        Self {
            width: (WIDTH * scale).round(),
            field: (FIELD * scale).round(),
            row: (ROW * scale).round(),
            pad: (PAD * scale).round(),
            scale,
            top: top.round(),
        }
    }

    pub fn height(&self, rows: usize) -> f64 {
        if rows == 0 {
            self.field
        } else {
            self.field + 1.0 + 2.0 * self.pad + rows as f64 * self.row
        }
    }

    pub fn row_y(&self, index: usize) -> f64 {
        self.field + 1.0 + self.pad + index as f64 * self.row
    }

    pub fn row_at(&self, y: f64, rows: usize) -> Option<usize> {
        let first = self.row_y(0);
        if y < first {
            return None;
        }
        let index = ((y - first) / self.row) as usize;
        (index < rows).then_some(index)
    }

    pub fn highlight_y(&self, index: usize) -> f64 {
        self.top + self.row_y(index)
    }
}

pub struct View {
    holder: IDCompositionVisual2,
    pub holder3: IDCompositionVisual3,
    pub scale: IDCompositionScaleTransform,
    pub highlight: IDCompositionVisual3,
    caret: IDCompositionVisual3,
    panel: IDCompositionSurface,
    _visuals: Vec<IDCompositionVisual2>,
    _surfaces: Vec<IDCompositionSurface>,
    pub layout: Layout,
    pub size: Channel,
    pub alpha: Channel,
    pub hl_y: Channel,
    pub hl_alpha: Channel,
    pub hiding: bool,
}

pub fn build(gfx: &Gfx, geo: &Geometry, parent: &IDCompositionVisual2) -> Result<View> {
    let layout = Layout::new(geo);
    let s = geo.scale as f32;
    let (width, max_height) = (layout.width, layout.height(MAX_ITEMS));
    unsafe {
        let dcomp = &gfx.dcomp;
        let holder = dcomp.CreateVisual()?;
        let scale = dcomp.CreateScaleTransform()?;
        let shift = dcomp.CreateTranslateTransform()?;
        shift.SetOffsetX2(geo.center.x as f32)?;
        shift.SetOffsetY2(geo.center.y as f32)?;
        holder.SetTransform(&scene::group(gfx, &scale, &shift)?)?;
        holder.SetOpacityMode(DCOMPOSITION_OPACITY_MODE_MULTIPLY)?;
        let left = -(width / 2.0) as f32;

        let panel = gfx.surface(width as u32, max_height as u32)?;
        let panel_visual = dcomp.CreateVisual()?;
        panel_visual.SetContent(&panel)?;
        panel_visual.SetOffsetX2(left)?;
        panel_visual.SetOffsetY2(layout.top as f32)?;

        let (hl_w, hl_h) = (width - 2.0 * layout.pad, layout.row);
        let highlight_surface = gfx.surface(hl_w as u32, hl_h as u32)?;
        gfx.draw(&highlight_surface, hl_w as u32, hl_h as u32, |p| {
            p.rounded_rect(
                (0.0, 0.0, hl_w as f32, hl_h as f32),
                8.0 * s,
                gfx::rgb(0xFFFFFF, 0.09),
                (0.0, gfx::rgb(0, 0.0)),
            );
        })?;
        let highlight = dcomp.CreateVisual()?;
        highlight.SetContent(&highlight_surface)?;
        highlight.SetOffsetX2(left + layout.pad as f32)?;

        let (caret_w, caret_h) = ((1.5 * s).ceil().max(1.0) as u32, (22.0 * s).round() as u32);
        let caret_surface = gfx.surface(caret_w, caret_h)?;
        gfx.draw(&caret_surface, caret_w, caret_h, |p| {
            p.rounded_rect(
                (0.0, 0.0, caret_w as f32, caret_h as f32),
                0.0,
                gfx::rgb(TEXT_COLOR, 0.95),
                (0.0, gfx::rgb(0, 0.0)),
            );
        })?;
        let caret = dcomp.CreateVisual()?;
        caret.SetContent(&caret_surface)?;

        holder.AddVisual(&panel_visual, false, None)?;
        holder.AddVisual(&highlight, true, &panel_visual)?;
        holder.AddVisual(&caret, true, &highlight)?;
        parent.AddVisual(&holder, true, None)?;

        Ok(View {
            holder3: holder.cast()?,
            holder: holder.clone(),
            scale,
            highlight: highlight.cast()?,
            caret: caret.cast()?,
            panel,
            _visuals: vec![holder, panel_visual, highlight, caret],
            _surfaces: vec![highlight_surface, caret_surface],
            layout,
            size: Channel::fixed(1.0),
            alpha: Channel::fixed(0.0),
            hl_y: Channel::fixed(layout.highlight_y(0)),
            hl_alpha: Channel::fixed(0.0),
            hiding: false,
        })
    }
}

impl View {
    pub fn paint(&self, gfx: &Gfx, text: &[char], caret: usize, items: &[Item]) -> Result<()> {
        let l = self.layout;
        let s = l.scale as f32;
        let (width, height) = (l.width as f32, l.height(items.len()) as f32);
        let text_x = (44.0 * s).round();
        let available = width - text_x - 16.0 * s;

        let string: String = text.iter().collect();
        let field = gfx.text(&string, 17.0 * s, false)?;
        let placeholder = gfx.text(
            tr(
                "Программа, файл, команда, пример или поиск",
                "App, file, command, math or web search",
            ),
            17.0 * s,
            false,
        )?;
        let search_icon = gfx.icon('\u{E721}', 16.0 * s)?;
        let before: usize = text[..caret].iter().map(|c| c.len_utf16()).sum();
        let caret_x = field.caret_x(before as u32);
        let scroll = (caret_x - available).max(0.0);

        let rows: Vec<_> = items
            .iter()
            .map(|item| {
                let hint = gfx.text(&item.hint, 11.0 * s, false);
                let hint_width = hint.as_ref().map_or(0.0, |h| h.width);
                let title_max = width - 40.0 * s - l.pad as f32 - hint_width - 28.0 * s;
                (
                    gfx.icon(item.glyph, 15.0 * s),
                    gfx.text_fit(&item.title, 13.5 * s, item.strong, Some(title_max)),
                    hint,
                )
            })
            .collect();

        let field_h = l.field as f32;
        gfx.draw(
            &self.panel,
            l.width as u32,
            l.height(MAX_ITEMS) as u32,
            |p| {
                p.rounded_rect(
                    (0.5, 0.5, width - 1.0, height - 1.0),
                    14.0 * s,
                    gfx::rgb(PANEL_COLOR, 0.98),
                    (1.0, gfx::rgb(0xFFFFFF, 0.10)),
                );
                p.text(
                    &search_icon,
                    16.0 * s,
                    (field_h - search_icon.height) / 2.0,
                    gfx::rgb(DIM_TEXT_COLOR, 1.0),
                );
                if text.is_empty() {
                    p.text(
                        &placeholder,
                        text_x,
                        (field_h - placeholder.height) / 2.0,
                        gfx::rgb(DIM_TEXT_COLOR, 0.8),
                    );
                } else {
                    p.clip((text_x - 2.0, 0.0, available + 4.0, field_h), || {
                        p.text(
                            &field,
                            text_x - scroll,
                            (field_h - field.height) / 2.0,
                            gfx::rgb(TEXT_COLOR, 1.0),
                        );
                    });
                }
                if items.is_empty() {
                    return;
                }
                let pad = l.pad as f32;
                p.rounded_rect(
                    (pad, field_h, width - 2.0 * pad, 1.0),
                    0.0,
                    gfx::rgb(0xFFFFFF, 0.08),
                    (0.0, gfx::rgb(0, 0.0)),
                );
                let row_h = l.row as f32;
                for (index, (icon, title, hint)) in rows.iter().enumerate() {
                    let y = l.row_y(index) as f32;
                    if let Ok(icon) = icon {
                        p.text(
                            icon,
                            pad + 12.0 * s,
                            y + (row_h - icon.height) / 2.0,
                            gfx::rgb(DIM_TEXT_COLOR, 1.0),
                        );
                    }
                    if let Ok(title) = title {
                        p.text(
                            title,
                            pad + 40.0 * s,
                            y + (row_h - title.height) / 2.0,
                            gfx::rgb(TEXT_COLOR, 1.0),
                        );
                    }
                    if let Ok(hint) = hint {
                        p.text(
                            hint,
                            width - pad - 12.0 * s - hint.width,
                            y + (row_h - hint.height) / 2.0,
                            gfx::rgb(DIM_TEXT_COLOR, 0.9),
                        );
                    }
                }
            },
        )?;

        unsafe {
            let caret_h = (22.0 * s).round();
            let x = -(width / 2.0) + text_x + caret_x - scroll;
            self.caret.SetOffsetX2(x.round())?;
            self.caret
                .SetOffsetY2((l.top as f32 + (field_h - caret_h) / 2.0).round())?;
            self.caret.SetOpacity(&blink(gfx)?)?;
        }
        Ok(())
    }

    pub fn detach(&self, parent: &IDCompositionVisual2) {
        unsafe {
            let _ = parent.RemoveVisual(&self.holder);
        }
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
