use windows::Win32::Graphics::DirectComposition::{
    DCOMPOSITION_BITMAP_INTERPOLATION_MODE_LINEAR, DCOMPOSITION_BORDER_MODE_SOFT,
    DCOMPOSITION_OPACITY_MODE_MULTIPLY, IDCompositionScaleTransform, IDCompositionSurface,
    IDCompositionTransform, IDCompositionTranslateTransform, IDCompositionVisual2,
    IDCompositionVisual3,
};
use windows::core::{Interface, Result};

use super::Geometry;
use super::info::{self, Look};
use super::motion::Channel;
use crate::config::SatelliteKind;
use crate::gfx::{self, Gfx};
#[cfg(feature = "music")]
use crate::lang::tr;

pub type Rect = (f64, f64, f64, f64);

const HALO_COLOR: u32 = 0x9FB2FF;
pub const PANEL_COLOR: u32 = 0x17191F;
pub const TEXT_COLOR: u32 = 0xE8EAF0;
pub const DIM_TEXT_COLOR: u32 = 0x9AA0AE;

pub struct Core {
    pub shifts: [IDCompositionTranslateTransform; 2],
    pub halo_scale: IDCompositionScaleTransform,
    pub halo: IDCompositionVisual3,
    _visuals: Vec<IDCompositionVisual2>,
    _surfaces: Vec<IDCompositionSurface>,
    pub x: Channel,
    pub y: Channel,
    pub glow: Channel,
    pub glow_size: Channel,
}

pub fn build_core(
    gfx: &Gfx,
    geo: &Geometry,
    halo_holder: IDCompositionVisual2,
    disk_holder: IDCompositionVisual2,
) -> Result<Core> {
    unsafe {
        let dcomp = &gfx.dcomp;
        let r = geo.core_radius as f32;
        let halo_r = geo.halo_radius as f32;

        let halo_surface = gfx.paint(geo.halo_size, |p| {
            p.fill_radial(
                halo_r,
                (0.0, 0.0),
                &[
                    (0.0, gfx::rgb(HALO_COLOR, 0.55)),
                    (0.45, gfx::rgb(HALO_COLOR, 0.38)),
                    (0.72, gfx::rgb(HALO_COLOR, 0.12)),
                    (1.0, gfx::rgb(HALO_COLOR, 0.0)),
                ],
            );
        })?;
        let disk_surface = gfx.paint(geo.core_size, |p| {
            p.fill_circle(r, gfx::rgb(0x16181E, 0.96));
            p.stroke_circle(r - 0.5, 1.0, gfx::rgb(0xFFFFFF, 0.14));
        })?;

        let halo = centered_visual(gfx, &halo_surface, geo.halo_size, geo.halo_size)?;
        let disk = centered_visual(gfx, &disk_surface, geo.core_size, geo.core_size)?;
        halo_holder.AddVisual(&halo, false, None)?;
        disk_holder.AddVisual(&disk, false, None)?;

        let halo_scale = dcomp.CreateScaleTransform()?;
        let halo_shift = dcomp.CreateTranslateTransform()?;
        let disk_shift = dcomp.CreateTranslateTransform()?;
        halo_holder.SetTransform(&group(gfx, &halo_scale, &halo_shift)?)?;
        disk_holder.SetTransform(&disk_shift)?;

        Ok(Core {
            shifts: [halo_shift, disk_shift],
            halo_scale,
            halo: halo.cast()?,
            _visuals: vec![halo_holder, disk_holder, halo, disk],
            _surfaces: vec![halo_surface, disk_surface],
            x: Channel::fixed(geo.center.x),
            y: Channel::fixed(geo.center.y),
            glow: Channel::fixed(0.0),
            glow_size: Channel::fixed(0.6),
        })
    }
}

pub struct Satellite {
    pub kind: SatelliteKind,
    pub look: Look,
    pub shift: IDCompositionTranslateTransform,
    pub scale: IDCompositionScaleTransform,
    pub body: IDCompositionVisual3,
    pub glow: IDCompositionVisual3,
    glow_surface: IDCompositionSurface,
    ring_surface: IDCompositionSurface,
    dot_surface: IDCompositionSurface,
    _visuals: Vec<IDCompositionVisual2>,
    pub phase: f64,
    pub x: Channel,
    pub y: Channel,
    pub size: Channel,
    pub alpha: Channel,
    pub shine: Channel,
}

pub fn build_satellite(
    gfx: &Gfx,
    geo: &Geometry,
    layer: &IDCompositionVisual2,
    kind: SatelliteKind,
    phase: f64,
) -> Result<Satellite> {
    unsafe {
        let dcomp = &gfx.dcomp;
        let glow_surface = gfx.surface(geo.glow_size, geo.glow_size)?;
        let ring_surface = gfx.surface(geo.ring_size, geo.ring_size)?;
        let dot_surface = gfx.surface(geo.satellite_size, geo.satellite_size)?;

        let glow = centered_visual(gfx, &glow_surface, geo.glow_size, geo.glow_size)?;
        let ring = centered_visual(gfx, &ring_surface, geo.ring_size, geo.ring_size)?;
        let dot = centered_visual(gfx, &dot_surface, geo.satellite_size, geo.satellite_size)?;
        let body = dcomp.CreateVisual()?;
        body.SetOpacityMode(DCOMPOSITION_OPACITY_MODE_MULTIPLY)?;
        body.AddVisual(&glow, false, None)?;
        body.AddVisual(&ring, true, &glow)?;
        body.AddVisual(&dot, true, &ring)?;

        let scale = dcomp.CreateScaleTransform()?;
        let shift = dcomp.CreateTranslateTransform()?;
        body.SetTransform(&group(gfx, &scale, &shift)?)?;
        layer.AddVisual(&body, false, None)?;

        let mut satellite = Satellite {
            kind,
            look: info::look(kind),
            shift,
            scale,
            body: body.cast()?,
            glow: glow.cast()?,
            glow_surface,
            ring_surface,
            dot_surface,
            _visuals: vec![body, glow, ring, dot],
            phase,
            x: Channel::fixed(geo.center.x),
            y: Channel::fixed(geo.center.y),
            size: Channel::fixed(0.0),
            alpha: Channel::fixed(0.0),
            shine: Channel::fixed(0.0),
        };
        satellite.paint(gfx, geo, satellite.look)?;
        Ok(satellite)
    }
}

impl Satellite {
    pub fn paint(&mut self, gfx: &Gfx, geo: &Geometry, look: Look) -> Result<()> {
        self.look = look;
        let color = look.color;
        let r = geo.satellite_radius as f32;
        let glow_r = geo.glow_radius as f32;
        let ring_r = geo.ring_radius as f32;
        let ring_w = geo.ring_width as f32;

        gfx.draw(&self.glow_surface, geo.glow_size, geo.glow_size, |p| {
            p.fill_radial(
                glow_r,
                (0.0, 0.0),
                &[
                    (0.0, gfx::rgb(color, 0.45)),
                    (0.3, gfx::rgb(color, 0.22)),
                    (1.0, gfx::rgb(color, 0.0)),
                ],
            );
        })?;
        gfx.draw(&self.ring_surface, geo.ring_size, geo.ring_size, |p| {
            p.stroke_circle(ring_r, ring_w, gfx::rgb(color, 0.22));
            p.stroke_arc(ring_r, ring_w, look.progress, gfx::rgb(color, 0.95));
        })?;
        gfx.draw(
            &self.dot_surface,
            geo.satellite_size,
            geo.satellite_size,
            |p| {
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
            },
        )
    }
}

pub struct Labels {
    pub layer: IDCompositionVisual3,
    pub shift: IDCompositionTranslateTransform,
    _visuals: Vec<IDCompositionVisual2>,
    _surfaces: Vec<IDCompositionSurface>,
    pub alpha: Channel,
}

pub fn build_labels(
    gfx: &Gfx,
    geo: &Geometry,
    parent: &IDCompositionVisual2,
    items: &[(&str, (f64, f64))],
) -> Result<Labels> {
    unsafe {
        let layer = gfx.dcomp.CreateVisual()?;
        let shift = gfx.dcomp.CreateTranslateTransform()?;
        layer.SetTransform(&shift)?;
        parent.AddVisual(&layer, true, None)?;

        let size = (11.0 * geo.scale) as f32;
        let height = geo.label_height as f32;
        let mut visuals = vec![];
        let mut surfaces = vec![];
        for &(text, (dx, dy)) in items {
            let block = gfx.text(text, size, false)?;
            let width = (block.width + 14.0 * geo.scale as f32).ceil();
            let surface = gfx.surface(width as u32, height as u32)?;
            gfx.draw(&surface, width as u32, height as u32, |p| {
                p.rounded_rect(
                    (0.0, 0.0, width, height),
                    height / 2.0,
                    gfx::rgb(PANEL_COLOR, 0.85),
                    (0.0, gfx::rgb(0, 0.0)),
                );
                let x = (width - block.width) / 2.0;
                let y = (height - block.height) / 2.0;
                p.text(&block, x, y, gfx::rgb(TEXT_COLOR, 1.0));
            })?;
            let visual = gfx.dcomp.CreateVisual()?;
            visual.SetContent(&surface)?;
            visual.SetOffsetX2((dx - width as f64 / 2.0).round() as f32)?;
            visual.SetOffsetY2(dy.round() as f32)?;
            layer.AddVisual(&visual, true, None)?;
            visuals.push(visual);
            surfaces.push(surface);
        }
        visuals.push(layer.clone());
        Ok(Labels {
            layer: layer.cast()?,
            shift,
            _visuals: visuals,
            _surfaces: surfaces,
            alpha: Channel::fixed(0.0),
        })
    }
}

pub struct CardView {
    pub visual: IDCompositionVisual3,
    pub shift: IDCompositionTranslateTransform,
    surface: IDCompositionSurface,
    _holder: IDCompositionVisual2,
    pub x: Channel,
    pub y: Channel,
    pub alpha: Channel,
    pub owner: Option<usize>,
    pub rect: (f64, f64, f64, f64),
}

pub fn card_size(geo: &Geometry, kind: SatelliteKind) -> (f64, f64) {
    let s = geo.scale;
    match kind {
        SatelliteKind::Music => ((232.0 * s).round(), (124.0 * s).round()),
        _ => ((184.0 * s).round(), (76.0 * s).round()),
    }
}

#[cfg(feature = "music")]
pub fn music_buttons(geo: &Geometry) -> [(f64, f64, f64, f64); 3] {
    let s = geo.scale;
    let (w, h, top, left, step) = (32.0 * s, 26.0 * s, 56.0 * s, 92.0 * s, 38.0 * s);
    [0.0, 1.0, 2.0].map(|i| (left + step * i, top, w, h))
}

#[cfg(feature = "music")]
pub fn volume_layout(geo: &Geometry) -> (Rect, Rect) {
    let s = geo.scale;
    let (width, _) = card_size(geo, SatelliteKind::Music);
    let row_top = 90.0 * s;
    let mute = (10.0 * s, row_top, 28.0 * s, 24.0 * s);
    let bar = (46.0 * s, row_top, width - 46.0 * s - 46.0 * s, 24.0 * s);
    (mute, bar)
}

#[cfg(feature = "music")]
mod glyph {
    pub const PREVIOUS: char = '\u{E892}';
    pub const NEXT: char = '\u{E893}';
    pub const PLAY: char = '\u{E768}';
    pub const PAUSE: char = '\u{E769}';
    pub const VOLUME: char = '\u{E767}';
    pub const MUTE: char = '\u{E74F}';
}

pub fn build_card(gfx: &Gfx, geo: &Geometry, parent: &IDCompositionVisual2) -> Result<CardView> {
    unsafe {
        let holder = gfx.dcomp.CreateVisual()?;
        let shift = gfx.dcomp.CreateTranslateTransform()?;
        holder.SetTransform(&shift)?;
        let surface = gfx.surface(geo.card_width as u32, geo.card_height as u32)?;
        holder.SetContent(&surface)?;
        parent.AddVisual(&holder, true, None)?;
        Ok(CardView {
            visual: holder.cast()?,
            shift,
            surface,
            _holder: holder,
            x: Channel::fixed(0.0),
            y: Channel::fixed(0.0),
            alpha: Channel::fixed(0.0),
            owner: None,
            rect: (0.0, 0.0, 0.0, 0.0),
        })
    }
}

impl CardView {
    pub fn paint(&self, gfx: &Gfx, geo: &Geometry, kind: SatelliteKind, color: u32) -> Result<()> {
        let card = info::card(kind);
        let s = geo.scale as f32;
        let (width, height) = card_size(geo, kind);
        let (width, height) = (width as f32, height as f32);
        let title = gfx.text(card.title, 11.0 * s, false)?;
        let value = gfx.text(&card.value, 20.0 * s, true)?;
        let detail = gfx.text(&card.detail, 11.0 * s, false)?;
        gfx.draw(
            &self.surface,
            geo.card_width as u32,
            geo.card_height as u32,
            |p| {
                panel(p, width, height, s);
                p.rounded_rect(
                    (7.0 * s, 12.0 * s, 3.0 * s, height - 24.0 * s),
                    1.5 * s,
                    gfx::rgb(color, 1.0),
                    (0.0, gfx::rgb(0, 0.0)),
                );
                let left = 18.0 * s;
                let mut y = 9.0 * s;
                p.text(&title, left, y, gfx::rgb(DIM_TEXT_COLOR, 1.0));
                y += title.height;
                p.text(&value, left, y, gfx::rgb(TEXT_COLOR, 1.0));
                y += value.height;
                p.text(&detail, left, y, gfx::rgb(DIM_TEXT_COLOR, 1.0));
            },
        )
    }

    #[cfg(feature = "music")]
    pub fn paint_music(
        &self,
        gfx: &Gfx,
        geo: &Geometry,
        track: Option<&super::media::Track>,
        cover: Option<&windows::Win32::Graphics::Imaging::IWICBitmap>,
        volume: Option<super::volume::Level>,
        color: u32,
    ) -> Result<()> {
        let s = geo.scale as f32;
        let (width, height) = card_size(geo, SatelliteKind::Music);
        let (width, height) = (width as f32, height as f32);
        let cover_size = cover_size(geo) as f32;
        let left = 92.0 * s;
        let text_width = width - left - 12.0 * s;
        let (title, artist) = match track {
            Some(track) if !track.title.is_empty() => (track.title.as_str(), track.artist.as_str()),
            Some(_) => (tr("Без названия", "Untitled"), ""),
            None => (
                tr("Ничего не играет", "Nothing playing"),
                tr("Включите музыку или видео", "Play some music or a video"),
            ),
        };
        let title = gfx.text_fit(title, 13.0 * s, true, Some(text_width))?;
        let artist = gfx.text_fit(artist, 11.0 * s, false, Some(text_width))?;
        let playing = track.is_some_and(|t| t.playing);
        let enabled = track.is_some();
        let play = if playing { glyph::PAUSE } else { glyph::PLAY };
        let icons = [glyph::PREVIOUS, play, glyph::NEXT].map(|g| gfx.icon(g, 14.0 * s));
        let (level, muted, system) =
            volume.map_or((0.0, true, true), |v| (v.level, v.muted, v.system));
        let speaker = gfx.icon(if muted { glyph::MUTE } else { glyph::VOLUME }, 14.0 * s)?;
        let percent = gfx.text(&format!("{}%", (level * 100.0).round()), 11.0 * s, false)?;
        let (mute_rect, bar_rect) = volume_layout(geo);

        gfx.draw(
            &self.surface,
            geo.card_width as u32,
            geo.card_height as u32,
            |p| {
                panel(p, width, height, s);
                let (cx, cy, r) = (10.0 * s, 10.0 * s, 8.0 * s);
                match cover {
                    Some(cover) => p.image(cover, cx, cy, cover_size, r),
                    None => p.rounded_rect(
                        (cx, cy, cover_size, cover_size),
                        r,
                        gfx::rgb(color, 0.22),
                        (0.0, gfx::rgb(0, 0.0)),
                    ),
                }
                p.text(&title, left, 12.0 * s, gfx::rgb(TEXT_COLOR, 1.0));
                p.text(
                    &artist,
                    left,
                    12.0 * s + title.height,
                    gfx::rgb(DIM_TEXT_COLOR, 1.0),
                );

                let ink = if enabled {
                    gfx::rgb(TEXT_COLOR, 1.0)
                } else {
                    gfx::rgb(DIM_TEXT_COLOR, 0.5)
                };
                for (index, ((x, y, w, h), icon)) in
                    music_buttons(geo).into_iter().zip(&icons).enumerate()
                {
                    let (x, y, w, h) = (x as f32, y as f32, w as f32, h as f32);
                    let fill = if index == 1 {
                        gfx::rgb(color, 0.28)
                    } else {
                        gfx::rgb(0xFFFFFF, 0.06)
                    };
                    p.rounded_rect((x, y, w, h), 6.0 * s, fill, (0.0, gfx::rgb(0, 0.0)));
                    if let Ok(icon) = icon {
                        p.text(
                            icon,
                            x + (w - icon.width) / 2.0,
                            y + (h - icon.height) / 2.0,
                            ink,
                        );
                    }
                }

                let (mx, my, mw, mh) = (
                    mute_rect.0 as f32,
                    mute_rect.1 as f32,
                    mute_rect.2 as f32,
                    mute_rect.3 as f32,
                );
                p.rounded_rect(
                    (mx, my, mw, mh),
                    6.0 * s,
                    gfx::rgb(0xFFFFFF, 0.06),
                    (0.0, gfx::rgb(0, 0.0)),
                );
                p.text(
                    &speaker,
                    mx + (mw - speaker.width) / 2.0,
                    my + (mh - speaker.height) / 2.0,
                    gfx::rgb(TEXT_COLOR, 1.0),
                );
                let (bx, by, bw, bh) = (
                    bar_rect.0 as f32,
                    bar_rect.1 as f32,
                    bar_rect.2 as f32,
                    bar_rect.3 as f32,
                );
                let track_h = 4.0 * s;
                let ty = by + (bh - track_h) / 2.0;
                p.rounded_rect(
                    (bx, ty, bw, track_h),
                    track_h / 2.0,
                    gfx::rgb(0xFFFFFF, 0.14),
                    (0.0, gfx::rgb(0, 0.0)),
                );
                let filled = if muted {
                    gfx::rgb(DIM_TEXT_COLOR, 0.6)
                } else if system {
                    gfx::rgb(TEXT_COLOR, 0.85)
                } else {
                    gfx::rgb(color, 1.0)
                };
                if level > 0.0 {
                    p.rounded_rect(
                        (bx, ty, (bw * level).max(track_h), track_h),
                        track_h / 2.0,
                        filled,
                        (0.0, filled),
                    );
                }
                p.text(
                    &percent,
                    bx + bw + 8.0 * s,
                    by + (bh - percent.height) / 2.0,
                    gfx::rgb(DIM_TEXT_COLOR, 1.0),
                );
            },
        )
    }
}

pub enum TileIcon {
    Glyph(char),
    Letter(char),
}

pub struct HubTile {
    pub icon: TileIcon,
    pub label: String,
    pub dim: bool,
}

pub fn hub_layout(geo: &Geometry, count: usize) -> ((f64, f64), Vec<Rect>) {
    let s = geo.scale;
    let (tile_w, tile_h, gap, pad) = (72.0 * s, 64.0 * s, 4.0 * s, 8.0 * s);
    let cols = count.clamp(1, 4);
    let rows = count.div_ceil(cols).max(1);
    let width = pad * 2.0 + cols as f64 * tile_w + (cols - 1) as f64 * gap;
    let height = pad * 2.0 + rows as f64 * tile_h + (rows - 1) as f64 * gap;
    let tiles = (0..count)
        .map(|i| {
            let (col, row) = ((i % cols) as f64, (i / cols) as f64);
            (
                pad + col * (tile_w + gap),
                pad + row * (tile_h + gap),
                tile_w,
                tile_h,
            )
        })
        .collect();
    ((width.round(), height.round()), tiles)
}

pub struct HubView {
    pub visual: IDCompositionVisual3,
    pub shift: IDCompositionTranslateTransform,
    pub highlight: IDCompositionVisual3,
    pub highlight_shift: IDCompositionTranslateTransform,
    surface: IDCompositionSurface,
    _visuals: Vec<IDCompositionVisual2>,
    _highlight_surface: IDCompositionSurface,
    pub x: Channel,
    pub y: Channel,
    pub alpha: Channel,
    pub hl_x: Channel,
    pub hl_y: Channel,
    pub hl_alpha: Channel,
    pub open: bool,
    pub rect: (f64, f64, f64, f64),
    pub hovered: Option<usize>,
    pub count: usize,
}

pub fn build_hub(
    gfx: &Gfx,
    geo: &Geometry,
    parent: &IDCompositionVisual2,
    count: usize,
) -> Result<HubView> {
    unsafe {
        let ((width, height), tiles) = hub_layout(geo, count);
        let (tile_w, tile_h) = (tiles[0].2, tiles[0].3);
        let holder = gfx.dcomp.CreateVisual()?;
        let shift = gfx.dcomp.CreateTranslateTransform()?;
        holder.SetTransform(&shift)?;
        let surface = gfx.surface(width as u32, height as u32)?;
        holder.SetContent(&surface)?;

        let s = geo.scale as f32;
        let highlight_surface = gfx.surface(tile_w as u32, tile_h as u32)?;
        gfx.draw(&highlight_surface, tile_w as u32, tile_h as u32, |p| {
            p.rounded_rect(
                (0.0, 0.0, tile_w as f32, tile_h as f32),
                10.0 * s,
                gfx::rgb(0xFFFFFF, 0.10),
                (0.0, gfx::rgb(0, 0.0)),
            );
        })?;
        let highlight = gfx.dcomp.CreateVisual()?;
        let highlight_shift = gfx.dcomp.CreateTranslateTransform()?;
        highlight.SetContent(&highlight_surface)?;
        highlight.SetTransform(&highlight_shift)?;
        holder.AddVisual(&highlight, true, None)?;
        parent.AddVisual(&holder, true, None)?;

        Ok(HubView {
            visual: holder.cast()?,
            shift,
            highlight: highlight.cast()?,
            highlight_shift,
            surface,
            _visuals: vec![holder, highlight],
            _highlight_surface: highlight_surface,
            x: Channel::fixed(0.0),
            y: Channel::fixed(0.0),
            alpha: Channel::fixed(0.0),
            hl_x: Channel::fixed(0.0),
            hl_y: Channel::fixed(0.0),
            hl_alpha: Channel::fixed(0.0),
            open: false,
            rect: (0.0, 0.0, width, height),
            hovered: None,
            count,
        })
    }
}

impl HubView {
    pub fn paint(&self, gfx: &Gfx, geo: &Geometry, items: &[HubTile]) -> Result<()> {
        let s = geo.scale as f32;
        let ((width, height), tiles) = hub_layout(geo, items.len());
        let label_width = tiles.first().map_or(0.0, |t| t.2 as f32) - 8.0 * s;
        let blocks: Vec<_> = items
            .iter()
            .map(|item| {
                let icon = match item.icon {
                    TileIcon::Glyph(glyph) => gfx.icon(glyph, 20.0 * s).map(|b| (b, false)),
                    TileIcon::Letter(letter) => gfx
                        .text(letter.encode_utf8(&mut [0; 4]), 12.0 * s, false)
                        .map(|b| (b, true)),
                };
                let label = gfx.text_fit(&item.label, 11.0 * s, false, Some(label_width));
                (icon, label, item.dim)
            })
            .collect();
        gfx.draw(&self.surface, width as u32, height as u32, |p| {
            p.rounded_rect(
                (0.5, 0.5, width as f32 - 1.0, height as f32 - 1.0),
                14.0 * s,
                gfx::rgb(PANEL_COLOR, 0.94),
                (1.0, gfx::rgb(0xFFFFFF, 0.10)),
            );
            for ((x, y, w, _), (icon, label, dim)) in tiles.iter().zip(&blocks) {
                let (x, y, w) = (*x as f32, *y as f32, *w as f32);
                p.rounded_rect(
                    (x, y, w, 64.0 * s),
                    10.0 * s,
                    gfx::rgb(0xFFFFFF, 0.04),
                    (0.0, gfx::rgb(0, 0.0)),
                );
                match icon {
                    Ok((icon, false)) => p.text(
                        icon,
                        x + (w - icon.width) / 2.0,
                        y + 12.0 * s,
                        if *dim {
                            gfx::rgb(DIM_TEXT_COLOR, 0.55)
                        } else {
                            gfx::rgb(TEXT_COLOR, 1.0)
                        },
                    ),
                    Ok((letter, true)) => {
                        let d = 21.0 * s;
                        let (cx, cy) = (x + (w - d) / 2.0, y + 11.5 * s);
                        p.rounded_rect(
                            (cx, cy, d, d),
                            d / 2.0,
                            gfx::rgb(0, 0.0),
                            (1.3 * s, gfx::rgb(TEXT_COLOR, 1.0)),
                        );
                        p.text(
                            letter,
                            (cx + (d - letter.width) / 2.0).round(),
                            (cy + (d - letter.height) / 2.0).round(),
                            gfx::rgb(TEXT_COLOR, 1.0),
                        );
                    }
                    Err(_) => {}
                }
                if let Ok(label) = label {
                    p.text(
                        label,
                        x + (w - label.width) / 2.0,
                        y + 38.0 * s,
                        gfx::rgb(DIM_TEXT_COLOR, 1.0),
                    );
                }
            }
        })
    }
}

pub struct NoticeView {
    pub body: IDCompositionVisual3,
    pub shift: IDCompositionTranslateTransform,
    pub scale: IDCompositionScaleTransform,
    pub glow: IDCompositionVisual3,
    _visuals: Vec<IDCompositionVisual2>,
    _surfaces: Vec<IDCompositionSurface>,
    pub width: f64,
    pub height: f64,
    pub reach: f64,
    pub x: Channel,
    pub y: Channel,
    pub size: Channel,
    pub alpha: Channel,
    pub shine: Channel,
    pub rect: Rect,
    pub hiding: bool,
    pub owner: Option<SatelliteKind>,
}

pub fn build_notice(
    gfx: &Gfx,
    geo: &Geometry,
    parent: &IDCompositionVisual2,
    text: &str,
    look: Look,
) -> Result<NoticeView> {
    unsafe {
        let s = geo.scale;
        let color = look.color;
        let dot_r = geo.satellite_radius * 0.75;
        let ring_r = dot_r + 2.6 * s;
        let ring_w = 1.4 * s;
        let glow_r = dot_r * 3.2;
        let even = |radius: f64| 2 * (radius + 2.0).ceil() as u32;
        let (dot_size, ring_size, glow_size) = (even(dot_r), even(ring_r + ring_w), even(glow_r));
        let r = dot_r as f32;

        let glow_surface = gfx.paint(glow_size, |p| {
            p.fill_radial(
                glow_r as f32,
                (0.0, 0.0),
                &[
                    (0.0, gfx::rgb(color, 0.75)),
                    (0.35, gfx::rgb(color, 0.35)),
                    (1.0, gfx::rgb(color, 0.0)),
                ],
            );
        })?;
        let ring_surface = gfx.paint(ring_size, |p| {
            p.stroke_circle(ring_r as f32, ring_w as f32, gfx::rgb(color, 0.22));
            p.stroke_arc(
                ring_r as f32,
                ring_w as f32,
                look.progress,
                gfx::rgb(color, 0.95),
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

        let block = gfx.text_fit(text, (11.5 * s) as f32, false, Some((240.0 * s) as f32))?;
        let pill_h = 2.0 * (11.0 * s).round();
        let pill_w = (block.width as f64 + 20.0 * s).ceil();
        let pill_surface = gfx.surface(pill_w as u32, pill_h as u32)?;
        gfx.draw(&pill_surface, pill_w as u32, pill_h as u32, |p| {
            let (w, h) = (pill_w as f32, pill_h as f32);
            p.rounded_rect(
                (0.5, 0.5, w - 1.0, h - 1.0),
                h / 2.0,
                gfx::rgb(PANEL_COLOR, 0.92),
                (1.0, gfx::rgb(color, 0.35)),
            );
            p.text(
                &block,
                (w - block.width) / 2.0,
                (h - block.height) / 2.0,
                gfx::rgb(TEXT_COLOR, 1.0),
            );
        })?;

        let glow = centered_visual(gfx, &glow_surface, glow_size, glow_size)?;
        let ring = centered_visual(gfx, &ring_surface, ring_size, ring_size)?;
        let dot = centered_visual(gfx, &dot_surface, dot_size, dot_size)?;
        let reach = ring_r + ring_w / 2.0;
        let gap = (6.0 * s).round();
        let pill_x = (reach + gap).round();
        let pill = gfx.dcomp.CreateVisual()?;
        pill.SetContent(&pill_surface)?;
        pill.SetOffsetX2(pill_x as f32)?;
        pill.SetOffsetY2(-(pill_h as f32) / 2.0)?;

        let body = gfx.dcomp.CreateVisual()?;
        body.SetOpacityMode(DCOMPOSITION_OPACITY_MODE_MULTIPLY)?;
        body.AddVisual(&glow, false, None)?;
        body.AddVisual(&ring, true, &glow)?;
        body.AddVisual(&dot, true, &ring)?;
        body.AddVisual(&pill, true, &dot)?;
        let scale = gfx.dcomp.CreateScaleTransform()?;
        let shift = gfx.dcomp.CreateTranslateTransform()?;
        body.SetTransform(&group(gfx, &scale, &shift)?)?;
        parent.AddVisual(&body, true, None)?;

        Ok(NoticeView {
            body: body.cast()?,
            shift,
            scale,
            glow: glow.cast()?,
            _visuals: vec![body, glow, ring, dot, pill],
            _surfaces: vec![glow_surface, ring_surface, dot_surface, pill_surface],
            width: pill_x + pill_w + reach,
            height: pill_h.max(2.0 * reach),
            reach,
            x: Channel::fixed(geo.center.x),
            y: Channel::fixed(geo.center.y),
            size: Channel::fixed(0.0),
            alpha: Channel::fixed(0.0),
            shine: Channel::fixed(1.0),
            rect: (0.0, 0.0, 0.0, 0.0),
            hiding: false,
            owner: None,
        })
    }
}

#[cfg(feature = "music")]
pub fn cover_size(geo: &Geometry) -> u32 {
    (72.0 * geo.scale).round() as u32
}

fn panel(p: &gfx::Painter, width: f32, height: f32, s: f32) {
    p.rounded_rect(
        (0.5, 0.5, width - 1.0, height - 1.0),
        10.0 * s,
        gfx::rgb(PANEL_COLOR, 0.94),
        (1.0, gfx::rgb(0xFFFFFF, 0.10)),
    );
}

pub fn centered_visual(
    gfx: &Gfx,
    surface: &IDCompositionSurface,
    width: u32,
    height: u32,
) -> Result<IDCompositionVisual2> {
    unsafe {
        let visual = gfx.dcomp.CreateVisual()?;
        visual.SetContent(surface)?;
        visual.SetOffsetX2(-(width as f32) / 2.0)?;
        visual.SetOffsetY2(-(height as f32) / 2.0)?;
        // Linear interpolation and soft borders keep slow sub-pixel motion smooth.
        visual.SetBitmapInterpolationMode(DCOMPOSITION_BITMAP_INTERPOLATION_MODE_LINEAR)?;
        visual.SetBorderMode(DCOMPOSITION_BORDER_MODE_SOFT)?;
        Ok(visual)
    }
}

pub fn group(
    gfx: &Gfx,
    scale: &IDCompositionScaleTransform,
    shift: &IDCompositionTranslateTransform,
) -> Result<IDCompositionTransform> {
    unsafe {
        gfx.dcomp
            .CreateTransformGroup(&[Some(scale.cast()?), Some(shift.cast()?)])
    }
}
