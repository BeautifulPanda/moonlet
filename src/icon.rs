use std::ffi::c_void;

use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
    DeleteObject,
};
use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, HICON, ICONINFO};
use windows::core::Result;

const MOON_COLOR: u32 = 0xF2D27A;

pub fn moon(size: i32) -> Result<HICON> {
    unsafe {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size,
                biHeight: -size,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let color = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0)?;
        let pixels = std::slice::from_raw_parts_mut(bits.cast::<u32>(), (size * size) as usize);
        for y in 0..size {
            for x in 0..size {
                let alpha = (coverage(x, y, size) * 255.0).round() as u32;
                pixels[(y * size + x) as usize] = (alpha << 24) | MOON_COLOR;
            }
        }

        let stride = ((size + 15) / 16 * 2) as usize;
        let mask_bits = vec![0u8; stride * size as usize];
        let mask = CreateBitmap(size, size, 1, 1, Some(mask_bits.as_ptr().cast()));

        let icon = CreateIconIndirect(&ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        });
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
        icon
    }
}

fn coverage(x: i32, y: i32, size: i32) -> f32 {
    const GRID: i32 = 4;
    let size = size as f32;
    let mut hits = 0;
    for sy in 0..GRID {
        for sx in 0..GRID {
            let px = (x as f32 + (sx as f32 + 0.5) / GRID as f32) / size;
            let py = (y as f32 + (sy as f32 + 0.5) / GRID as f32) / size;
            let in_moon = distance_sq(px, py, 0.5, 0.5) <= 0.42 * 0.42;
            let in_shadow = distance_sq(px, py, 0.70, 0.36) <= 0.36 * 0.36;
            if in_moon && !in_shadow {
                hits += 1;
            }
        }
    }
    hits as f32 / (GRID * GRID) as f32
}

fn distance_sq(x: f32, y: f32, cx: f32, cy: f32) -> f32 {
    (x - cx) * (x - cx) + (y - cy) * (y - cy)
}
