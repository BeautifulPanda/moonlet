pub const MOON_COLOR: u32 = 0xF2D27A;

pub fn coverage(x: i32, y: i32, size: i32) -> f32 {
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
