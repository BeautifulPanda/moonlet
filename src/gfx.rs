use std::cell::OnceCell;
use std::f32::consts::TAU;

use windows::Win32::Foundation::{CloseHandle, E_FAIL, HANDLE, HMODULE, POINT};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_F, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_OPEN,
    D2D1_GRADIENT_STOP,
};
#[cfg(feature = "music")]
use windows::Win32::Graphics::Direct2D::D2D1_BRUSH_PROPERTIES;
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_ALIASED, D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_LARGE, D2D1_ARC_SIZE_SMALL,
    D2D1_CAP_STYLE_ROUND, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ELLIPSE, D2D1_EXTEND_MODE_CLAMP,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_GAMMA_2_2, D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES,
    D2D1_ROUNDED_RECT, D2D1_STROKE_STYLE_PROPERTIES, D2D1_SWEEP_DIRECTION_CLOCKWISE,
    D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, D2D1CreateFactory, ID2D1Device, ID2D1DeviceContext,
    ID2D1Factory, ID2D1Factory1, ID2D1RenderTarget,
};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_UNKNOWN, D3D_DRIVER_TYPE_WARP,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
    ID3D11Device4,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice2, IDCompositionDesktopDevice, IDCompositionSurface,
};
#[cfg(feature = "launcher")]
use windows::Win32::Graphics::DirectWrite::DWRITE_HIT_TEST_METRICS;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_METRICS,
    DWRITE_WORD_WRAPPING_NO_WRAP, DWriteCreateFactory, IDWriteFactory, IDWriteTextLayout,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory2, DXGI_CREATE_FACTORY_FLAGS, DXGI_GPU_PREFERENCE_MINIMUM_POWER, IDXGIAdapter,
    IDXGIDevice, IDXGIDevice3, IDXGIFactory6,
};
#[cfg(feature = "music")]
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICBitmap, IWICImagingFactory,
    WICBitmapCacheOnLoad, WICBitmapDitherTypeNone, WICBitmapInterpolationModeFant,
    WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
};
#[cfg(feature = "music")]
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::Threading::CreateEventW;
#[cfg(feature = "dev")]
use windows::Win32::System::Threading::SetEvent;
use windows::core::{Interface, Result, w};
#[cfg(feature = "music")]
use windows_numerics::Matrix3x2;
use windows_numerics::Vector2;

pub struct Gfx {
    pub dcomp: IDCompositionDesktopDevice,
    factory: ID2D1Factory1,
    dwrite: OnceCell<IDWriteFactory>,
    #[cfg(feature = "music")]
    wic: OnceCell<IWICImagingFactory>,
    d2d: ID2D1Device,
    d3d: ID3D11Device,
    lost: Option<(HANDLE, u32)>,
}

impl Drop for Gfx {
    fn drop(&mut self) {
        // DirectComposition frees released visuals only on the next Commit, so commit
        // once more here; otherwise the device and its driver threads stay alive.
        unsafe {
            let _ = self.dcomp.Commit();
            if self.d3d.GetDeviceRemovedReason().is_ok() {
                let _ = self.dcomp.WaitForCommitCompletion();
            }
            if let Some((event, cookie)) = self.lost {
                if let Ok(device) = self.d3d.cast::<ID3D11Device4>() {
                    device.UnregisterDeviceRemoved(cookie);
                }
                let _ = CloseHandle(event);
            }
        }
    }
}

impl Gfx {
    pub fn new() -> Result<Self> {
        unsafe {
            let d3d = create_d3d_device()?;
            let dxgi: IDXGIDevice = d3d.cast()?;
            let factory: ID2D1Factory1 =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let d2d = factory.CreateDevice(&dxgi)?;
            let dcomp: IDCompositionDesktopDevice = DCompositionCreateDevice2(&d2d)?;
            let lost = watch_device_lost(&d3d);
            Ok(Self {
                dcomp,
                factory,
                dwrite: OnceCell::new(),
                #[cfg(feature = "music")]
                wic: OnceCell::new(),
                d2d,
                d3d,
                lost,
            })
        }
    }

    pub fn lost_event(&self) -> Option<HANDLE> {
        self.lost.map(|(event, _)| event)
    }

    #[cfg(feature = "dev")]
    pub fn simulate_device_lost(&self) {
        if let Some(event) = self.lost_event() {
            unsafe {
                let _ = SetEvent(event);
            }
        }
    }

    pub fn trim(&self) {
        unsafe {
            self.d2d.ClearResources(0);
            if let Ok(dxgi) = self.d3d.cast::<IDXGIDevice3>() {
                dxgi.Trim();
            }
        }
    }

    pub fn surface(&self, width: u32, height: u32) -> Result<IDCompositionSurface> {
        unsafe {
            self.dcomp.CreateSurface(
                width,
                height,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_ALPHA_MODE_PREMULTIPLIED,
            )
        }
    }

    pub fn draw(
        &self,
        surface: &IDCompositionSurface,
        width: u32,
        height: u32,
        draw: impl FnOnce(&Painter),
    ) -> Result<()> {
        unsafe {
            let mut offset = POINT::default();
            let dc: ID2D1DeviceContext = surface.BeginDraw(None, &mut offset)?;
            dc.SetDpi(96.0, 96.0);
            // ClearType leaves colored fringes on a transparent surface.
            dc.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            let origin = (offset.x as f32, offset.y as f32);
            let size = (width as f32, height as f32);
            // Surfaces share an atlas: clip to our rectangle before clearing.
            let bounds = D2D_RECT_F {
                left: origin.0,
                top: origin.1,
                right: origin.0 + size.0,
                bottom: origin.1 + size.1,
            };
            dc.PushAxisAlignedClip(&bounds, D2D1_ANTIALIAS_MODE_ALIASED);
            dc.Clear(Some(&D2D1_COLOR_F::default()));
            draw(&Painter {
                dc: &dc,
                factory: &self.factory,
                origin,
                size,
            });
            dc.PopAxisAlignedClip();
            surface.EndDraw()
        }
    }

    pub fn paint(&self, size: u32, draw: impl FnOnce(&Painter)) -> Result<IDCompositionSurface> {
        let surface = self.surface(size, size)?;
        self.draw(&surface, size, size, draw)?;
        Ok(surface)
    }

    pub fn text(&self, text: &str, size: f32, bold: bool) -> Result<TextBlock> {
        self.text_fit(text, size, bold, None)
    }

    pub fn text_fit(
        &self,
        text: &str,
        size: f32,
        bold: bool,
        max_width: Option<f32>,
    ) -> Result<TextBlock> {
        self.layout(text, w!("Segoe UI"), size, bold, max_width)
    }

    pub fn icon(&self, glyph: char, size: f32) -> Result<TextBlock> {
        self.layout(
            glyph.encode_utf8(&mut [0; 4]),
            w!("Segoe MDL2 Assets"),
            size,
            false,
            None,
        )
    }

    fn layout(
        &self,
        text: &str,
        family: windows::core::PCWSTR,
        size: f32,
        bold: bool,
        max_width: Option<f32>,
    ) -> Result<TextBlock> {
        unsafe {
            let factory = match self.dwrite.get() {
                Some(factory) => factory,
                None => {
                    let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
                    self.dwrite.get_or_init(|| factory)
                }
            };
            let weight = if bold {
                DWRITE_FONT_WEIGHT_SEMI_BOLD
            } else {
                DWRITE_FONT_WEIGHT_NORMAL
            };
            let format = factory.CreateTextFormat(
                family,
                None,
                weight,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                w!("ru-ru"),
            )?;
            format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            if max_width.is_some() {
                let trimming = DWRITE_TRIMMING {
                    granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                    delimiter: 0,
                    delimiterCount: 0,
                };
                let ellipsis = factory.CreateEllipsisTrimmingSign(&format)?;
                format.SetTrimming(&trimming, &ellipsis)?;
            }
            let wide: Vec<u16> = text.encode_utf16().collect();
            let layout =
                factory.CreateTextLayout(&wide, &format, max_width.unwrap_or(4096.0), 1024.0)?;
            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut metrics)?;
            Ok(TextBlock {
                layout,
                width: metrics.width.min(max_width.unwrap_or(f32::MAX)),
                height: metrics.height,
            })
        }
    }

    #[cfg(feature = "music")]
    pub fn decode_image(&self, bytes: &[u8], size: u32) -> Result<IWICBitmap> {
        unsafe {
            let wic = match self.wic.get() {
                Some(wic) => wic,
                None => {
                    let wic: IWICImagingFactory =
                        CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
                    self.wic.get_or_init(|| wic)
                }
            };
            let stream = wic.CreateStream()?;
            stream.InitializeFromMemory(bytes)?;
            let decoder = wic.CreateDecoderFromStream(
                &stream,
                std::ptr::null(),
                WICDecodeMetadataCacheOnDemand,
            )?;
            let frame = decoder.GetFrame(0)?;
            let scaler = wic.CreateBitmapScaler()?;
            scaler.Initialize(&frame, size, size, WICBitmapInterpolationModeFant)?;
            let converter = wic.CreateFormatConverter()?;
            converter.Initialize(
                &scaler,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeCustom,
            )?;
            wic.CreateBitmapFromSource(&converter, WICBitmapCacheOnLoad)
        }
    }
}

pub struct TextBlock {
    layout: IDWriteTextLayout,
    pub width: f32,
    pub height: f32,
}

impl TextBlock {
    #[cfg(feature = "launcher")]
    pub fn caret_x(&self, position: u32) -> f32 {
        if position == 0 {
            return 0.0;
        }
        let (mut x, mut y) = (0.0, 0.0);
        let mut metrics = DWRITE_HIT_TEST_METRICS::default();
        unsafe {
            let _ =
                self.layout
                    .HitTestTextPosition(position - 1, true, &mut x, &mut y, &mut metrics);
        }
        x
    }
}

pub struct Painter<'a> {
    dc: &'a ID2D1DeviceContext,
    factory: &'a ID2D1Factory1,
    origin: (f32, f32),
    size: (f32, f32),
}

impl Painter<'_> {
    fn target(&self) -> &ID2D1RenderTarget {
        self.dc
    }

    fn center(&self) -> (f32, f32) {
        (
            self.origin.0 + self.size.0 / 2.0,
            self.origin.1 + self.size.1 / 2.0,
        )
    }

    pub fn fill_circle(&self, radius: f32, color: D2D1_COLOR_F) {
        unsafe {
            if let Ok(brush) = self.dc.CreateSolidColorBrush(&color, None) {
                self.dc.FillEllipse(&self.ellipse(radius), &brush);
            }
        }
    }

    pub fn stroke_circle(&self, radius: f32, width: f32, color: D2D1_COLOR_F) {
        unsafe {
            if let Ok(brush) = self.dc.CreateSolidColorBrush(&color, None) {
                self.dc
                    .DrawEllipse(&self.ellipse(radius), &brush, width, None);
            }
        }
    }

    pub fn stroke_arc(&self, radius: f32, width: f32, fraction: f32, color: D2D1_COLOR_F) {
        if fraction <= 0.001 {
            return;
        }
        if fraction >= 0.999 {
            self.stroke_circle(radius, width, color);
            return;
        }
        let (cx, cy) = self.center();
        let angle = fraction * TAU;
        let start = Vector2 {
            X: cx,
            Y: cy - radius,
        };
        let end = Vector2 {
            X: cx + radius * angle.sin(),
            Y: cy - radius * angle.cos(),
        };
        let factory: &ID2D1Factory = self.factory;
        unsafe {
            let Ok(path) = factory.CreatePathGeometry() else {
                return;
            };
            let Ok(sink) = path.Open() else {
                return;
            };
            sink.BeginFigure(start, D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: end,
                size: D2D_SIZE_F {
                    width: radius,
                    height: radius,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: if fraction > 0.5 {
                    D2D1_ARC_SIZE_LARGE
                } else {
                    D2D1_ARC_SIZE_SMALL
                },
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            if sink.Close().is_err() {
                return;
            }
            let caps = D2D1_STROKE_STYLE_PROPERTIES {
                startCap: D2D1_CAP_STYLE_ROUND,
                endCap: D2D1_CAP_STYLE_ROUND,
                ..Default::default()
            };
            let Ok(style) = factory.CreateStrokeStyle(&caps, None) else {
                return;
            };
            if let Ok(brush) = self.dc.CreateSolidColorBrush(&color, None) {
                self.target().DrawGeometry(&path, &brush, width, &style);
            }
        }
    }

    pub fn fill_radial(&self, radius: f32, highlight: (f32, f32), stops: &[(f32, D2D1_COLOR_F)]) {
        let stops: Vec<D2D1_GRADIENT_STOP> = stops
            .iter()
            .map(|&(position, color)| D2D1_GRADIENT_STOP { position, color })
            .collect();
        let (cx, cy) = self.center();
        let props = D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES {
            center: Vector2 { X: cx, Y: cy },
            gradientOriginOffset: Vector2 {
                X: highlight.0,
                Y: highlight.1,
            },
            radiusX: radius,
            radiusY: radius,
        };
        let target = self.target();
        unsafe {
            let Ok(collection) =
                target.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)
            else {
                return;
            };
            if let Ok(brush) = target.CreateRadialGradientBrush(&props, None, &collection) {
                target.FillEllipse(&self.ellipse(radius), &brush);
            }
        }
    }

    pub fn rounded_rect(
        &self,
        rect: (f32, f32, f32, f32),
        radius: f32,
        fill: D2D1_COLOR_F,
        border: (f32, D2D1_COLOR_F),
    ) {
        let (x, y, w, h) = rect;
        let shape = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: self.origin.0 + x,
                top: self.origin.1 + y,
                right: self.origin.0 + x + w,
                bottom: self.origin.1 + y + h,
            },
            radiusX: radius,
            radiusY: radius,
        };
        let target = self.target();
        unsafe {
            if let Ok(brush) = self.dc.CreateSolidColorBrush(&fill, None) {
                target.FillRoundedRectangle(&shape, &brush);
            }
            if border.0 > 0.0
                && let Ok(brush) = self.dc.CreateSolidColorBrush(&border.1, None)
            {
                target.DrawRoundedRectangle(&shape, &brush, border.0, None);
            }
        }
    }

    #[cfg(feature = "launcher")]
    pub fn clip(&self, rect: (f32, f32, f32, f32), draw: impl FnOnce()) {
        let (x, y, w, h) = rect;
        let bounds = D2D_RECT_F {
            left: self.origin.0 + x,
            top: self.origin.1 + y,
            right: self.origin.0 + x + w,
            bottom: self.origin.1 + y + h,
        };
        unsafe {
            self.dc
                .PushAxisAlignedClip(&bounds, D2D1_ANTIALIAS_MODE_ALIASED);
        }
        draw();
        unsafe {
            self.dc.PopAxisAlignedClip();
        }
    }

    pub fn text(&self, block: &TextBlock, x: f32, y: f32, color: D2D1_COLOR_F) {
        let origin = Vector2 {
            X: self.origin.0 + x,
            Y: self.origin.1 + y,
        };
        unsafe {
            if let Ok(brush) = self.dc.CreateSolidColorBrush(&color, None) {
                self.target().DrawTextLayout(
                    origin,
                    &block.layout,
                    &brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                );
            }
        }
    }

    #[cfg(feature = "music")]
    pub fn image(&self, image: &IWICBitmap, x: f32, y: f32, size: f32, radius: f32) {
        let target = self.target();
        let (left, top) = (self.origin.0 + x, self.origin.1 + y);
        unsafe {
            let Ok(bitmap) = target.CreateBitmapFromWicBitmap(image, None) else {
                return;
            };
            let props = D2D1_BRUSH_PROPERTIES {
                opacity: 1.0,
                transform: Matrix3x2::translation(left, top),
            };
            let Ok(brush) = target.CreateBitmapBrush(&bitmap, None, Some(&props)) else {
                return;
            };
            let shape = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left,
                    top,
                    right: left + size,
                    bottom: top + size,
                },
                radiusX: radius,
                radiusY: radius,
            };
            target.FillRoundedRectangle(&shape, &brush);
        }
    }

    fn ellipse(&self, radius: f32) -> D2D1_ELLIPSE {
        let (cx, cy) = self.center();
        D2D1_ELLIPSE {
            point: Vector2 { X: cx, Y: cy },
            radiusX: radius,
            radiusY: radius,
        }
    }
}

pub fn rgb(hex: u32, alpha: f32) -> D2D1_COLOR_F {
    let channel = |shift: u32| ((hex >> shift) & 0xFF) as f32 / 255.0;
    D2D1_COLOR_F {
        r: channel(16),
        g: channel(8),
        b: channel(0),
        a: alpha,
    }
}

pub fn mix(a: u32, b: u32, k: f32) -> u32 {
    let channel = |shift: u32| {
        let ca = ((a >> shift) & 0xFF) as f32;
        let cb = ((b >> shift) & 0xFF) as f32;
        ((ca + (cb - ca) * k).round() as u32) << shift
    };
    channel(16) | channel(8) | channel(0)
}

// Prefer the low-power adapter so the discrete GPU of a laptop can stay asleep.
unsafe fn create_d3d_device() -> Result<ID3D11Device> {
    unsafe {
        let adapter: Option<IDXGIAdapter> =
            CreateDXGIFactory2::<IDXGIFactory6>(DXGI_CREATE_FACTORY_FLAGS(0))
                .and_then(|factory| {
                    factory.EnumAdapterByGpuPreference(0, DXGI_GPU_PREFERENCE_MINIMUM_POWER)
                })
                .ok();

        let driver = if adapter.is_some() {
            D3D_DRIVER_TYPE_UNKNOWN
        } else {
            D3D_DRIVER_TYPE_HARDWARE
        };
        if let Ok(device) = try_create(adapter.as_ref(), driver) {
            return Ok(device);
        }
        try_create(None, D3D_DRIVER_TYPE_WARP)
    }
}

// After a TDR or a driver update everything drawn is gone. The main loop
// waits on this event and rebuilds the orbit.
fn watch_device_lost(d3d: &ID3D11Device) -> Option<(HANDLE, u32)> {
    unsafe {
        let device: ID3D11Device4 = d3d.cast().ok()?;
        let event = CreateEventW(None, false, false, None).ok()?;
        match device.RegisterDeviceRemovedEvent(event) {
            Ok(cookie) => Some((event, cookie)),
            Err(_) => {
                let _ = CloseHandle(event);
                None
            }
        }
    }
}

unsafe fn try_create(
    adapter: Option<&IDXGIAdapter>,
    driver: D3D_DRIVER_TYPE,
) -> Result<ID3D11Device> {
    let mut device = None;
    unsafe {
        D3D11CreateDevice(
            adapter,
            driver,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )?;
    }
    device.ok_or_else(|| E_FAIL.into())
}
