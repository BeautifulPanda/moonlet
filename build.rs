// Embeds the moon icon and version info as a hand-written .res file.

use std::path::PathBuf;

#[path = "src/icon/shape.rs"]
mod shape;

const SIZES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];

const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
const RT_VERSION: u16 = 16;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/icon/shape.rs");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if !windows || !msvc {
        return;
    }

    let mut res = Vec::new();
    resource(&mut res, 0, 0, 0, &[]);

    let mut group = Vec::new();
    group.extend_from_slice(&0u16.to_le_bytes());
    group.extend_from_slice(&1u16.to_le_bytes());
    group.extend_from_slice(&(SIZES.len() as u16).to_le_bytes());
    for (index, &size) in SIZES.iter().enumerate() {
        let id = index as u16 + 1;
        let png = png(size);
        resource(&mut res, RT_ICON, id, 0x1010, &png);
        let side = if size >= 256 { 0 } else { size as u8 };
        group.extend_from_slice(&[side, side, 0, 0]);
        group.extend_from_slice(&1u16.to_le_bytes());
        group.extend_from_slice(&32u16.to_le_bytes());
        group.extend_from_slice(&(png.len() as u32).to_le_bytes());
        group.extend_from_slice(&id.to_le_bytes());
    }
    resource(&mut res, RT_GROUP_ICON, 1, 0x1030, &group);
    resource(&mut res, RT_VERSION, 1, 0x0030, &version_info());

    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("moonlet.res");
    std::fs::write(&out, res).unwrap();
    println!("cargo:rustc-link-arg-bins={}", out.display());
}

fn resource(res: &mut Vec<u8>, kind: u16, name: u16, flags: u16, data: &[u8]) {
    res.extend_from_slice(&(data.len() as u32).to_le_bytes());
    res.extend_from_slice(&32u32.to_le_bytes());
    for value in [0xFFFF, kind, 0xFFFF, name] {
        res.extend_from_slice(&value.to_le_bytes());
    }
    res.extend_from_slice(&0u32.to_le_bytes());
    res.extend_from_slice(&flags.to_le_bytes());
    res.extend_from_slice(&0u16.to_le_bytes());
    res.extend_from_slice(&0u32.to_le_bytes());
    res.extend_from_slice(&0u32.to_le_bytes());
    res.extend_from_slice(data);
    pad(res);
}

fn pad(bytes: &mut Vec<u8>) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
}

fn node(key: &str, value: &[u8], value_len: u16, text: bool, children: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![0, 0];
    out.extend_from_slice(&value_len.to_le_bytes());
    out.extend_from_slice(&(text as u16).to_le_bytes());
    out.extend(utf16z(key));
    pad(&mut out);
    out.extend_from_slice(value);
    for child in children {
        pad(&mut out);
        out.extend_from_slice(child);
    }
    let length = out.len() as u16;
    out[..2].copy_from_slice(&length.to_le_bytes());
    out
}

fn utf16z(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain([0])
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
}

fn string(key: &str, value: &str) -> Vec<u8> {
    let value_units = value.encode_utf16().count() as u16 + 1;
    node(key, &utf16z(value), value_units, true, &[])
}

fn version_info() -> Vec<u8> {
    let part = |name: &str| -> u32 {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    };
    let (major, minor, patch) = (
        part("CARGO_PKG_VERSION_MAJOR"),
        part("CARGO_PKG_VERSION_MINOR"),
        part("CARGO_PKG_VERSION_PATCH"),
    );
    let version = std::env::var("CARGO_PKG_VERSION").unwrap();

    let mut fixed = Vec::new();
    for value in [
        0xFEEF_04BD,
        0x0001_0000,
        (major << 16) | minor,
        patch << 16,
        (major << 16) | minor,
        patch << 16,
        0x3F,
        0,
        0x0004_0004,
        1,
        0,
        0,
        0,
    ] {
        fixed.extend_from_slice(&u32::to_le_bytes(value));
    }

    let strings = node(
        "040904B0",
        &[],
        0,
        true,
        &[
            string("FileDescription", "Moonlet"),
            string("FileVersion", &version),
            string("InternalName", "moonlet"),
            string("LegalCopyright", "MIT License"),
            string("OriginalFilename", "moonlet.exe"),
            string("ProductName", "Moonlet"),
            string("ProductVersion", &version),
        ],
    );
    let string_info = node("StringFileInfo", &[], 0, true, &[strings]);
    let mut translation = Vec::new();
    translation.extend_from_slice(&0x0409u16.to_le_bytes());
    translation.extend_from_slice(&0x04B0u16.to_le_bytes());
    let var = node("Translation", &translation, 4, false, &[]);
    let var_info = node("VarFileInfo", &[], 0, true, &[var]);
    node(
        "VS_VERSION_INFO",
        &fixed,
        fixed.len() as u16,
        false,
        &[string_info, var_info],
    )
}

// ---- PNG ----

fn png(size: u32) -> Vec<u8> {
    let (r, g, b) = (
        (shape::MOON_COLOR >> 16) as u8,
        (shape::MOON_COLOR >> 8) as u8,
        shape::MOON_COLOR as u8,
    );
    let row_len = size as usize * 4;
    let mut rows: Vec<Vec<u8>> = Vec::new();
    for y in 0..size as i32 {
        let mut row = Vec::with_capacity(row_len);
        for x in 0..size as i32 {
            let alpha = (shape::coverage(x, y, size as i32) * 255.0).round() as u8;
            row.extend_from_slice(&[r, g, b, alpha]);
        }
        rows.push(row);
    }

    let mut raw = Vec::with_capacity((row_len + 1) * size as usize);
    for (y, row) in rows.iter().enumerate() {
        let sub: Vec<u8> = (0..row_len)
            .map(|i| row[i].wrapping_sub(if i >= 4 { row[i - 4] } else { 0 }))
            .collect();
        let up: Vec<u8> = (0..row_len)
            .map(|i| row[i].wrapping_sub(if y > 0 { rows[y - 1][i] } else { 0 }))
            .collect();
        let cost = |line: &[u8]| {
            line.iter()
                .map(|&v| (v as i8).unsigned_abs() as u32)
                .sum::<u32>()
        };
        if cost(&up) < cost(&sub) {
            raw.push(2);
            raw.extend(up);
        } else {
            raw.push(1);
            raw.extend(sub);
        }
    }

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut header = Vec::new();
    header.extend_from_slice(&size.to_be_bytes());
    header.extend_from_slice(&size.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &zlib(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

struct Bits {
    out: Vec<u8>,
    acc: u32,
    count: u32,
}

impl Bits {
    fn put(&mut self, value: u32, bits: u32) {
        self.acc |= value << self.count;
        self.count += bits;
        while self.count >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.count -= 8;
        }
    }

    fn code(&mut self, code: u32, bits: u32) {
        let mut reversed = 0;
        for i in 0..bits {
            reversed |= ((code >> i) & 1) << (bits - 1 - i);
        }
        self.put(reversed, bits);
    }

    fn literal(&mut self, value: u32) {
        match value {
            0..=143 => self.code(0x30 + value, 8),
            144..=255 => self.code(0x190 + value - 144, 9),
            256..=279 => self.code(value - 256, 7),
            _ => self.code(0xC0 + value - 280, 8),
        }
    }
}

const LENGTH_BASE: [u32; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u32; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u32; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u32; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

fn zlib(data: &[u8]) -> Vec<u8> {
    const WINDOW: usize = 32768;
    const HASH: usize = 1 << 14;
    let mut bits = Bits {
        out: vec![0x78, 0x01],
        acc: 0,
        count: 0,
    };
    bits.put(1, 1);
    bits.put(1, 2);
    let mut last = vec![usize::MAX; HASH];
    let hash = |i: usize| {
        ((data[i] as usize) << 8 ^ (data[i + 1] as usize) << 4 ^ data[i + 2] as usize) % HASH
    };
    let matching = |a: usize, b: usize| {
        let mut n = 0;
        while b + n < data.len() && n < 258 && data[a + n] == data[b + n] {
            n += 1;
        }
        n
    };
    let mut i = 0;
    while i < data.len() {
        let mut best = (0, 0);
        if i + 3 <= data.len() {
            let h = hash(i);
            for from in [last[h], i.wrapping_sub(4), i.wrapping_sub(1)] {
                if from < i && i - from <= WINDOW {
                    let len = matching(from, i);
                    if len > best.0 {
                        best = (len, i - from);
                    }
                }
            }
            last[h] = i;
        }
        if best.0 >= 3 {
            let (len, dist) = best;
            let code = LENGTH_BASE
                .iter()
                .rposition(|&base| base <= len as u32)
                .unwrap();
            bits.literal(257 + code as u32);
            bits.put(len as u32 - LENGTH_BASE[code], LENGTH_EXTRA[code]);
            let dcode = DIST_BASE
                .iter()
                .rposition(|&base| base <= dist as u32)
                .unwrap();
            bits.code(dcode as u32, 5);
            bits.put(dist as u32 - DIST_BASE[dcode], DIST_EXTRA[dcode]);
            for j in i + 1..(i + len).min(data.len().saturating_sub(2)) {
                last[hash(j)] = j;
            }
            i += len;
        } else {
            bits.literal(data[i] as u32);
            i += 1;
        }
    }
    bits.literal(256);
    if bits.count > 0 {
        bits.out.push(bits.acc as u8);
    }
    let mut out = bits.out;
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    out.extend_from_slice(&((b << 16) | a).to_be_bytes());
    out
}
