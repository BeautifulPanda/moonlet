use std::sync::OnceLock;

use windows::Win32::Media::Audio::{
    PlaySoundW, SND_ASYNC, SND_FILENAME, SND_MEMORY, SND_NODEFAULT,
};
use windows::core::{HSTRING, PCWSTR};

pub const DROP: &str = "drop";
pub const OFF: &str = "off";

const RATE: u32 = 22050;

pub fn play(setting: &str) {
    match setting {
        OFF => {}
        "" | DROP => play_drop(),
        path => {
            let path = HSTRING::from(path);
            let ok = unsafe {
                PlaySoundW(&path, None, SND_FILENAME | SND_ASYNC | SND_NODEFAULT).as_bool()
            };
            if !ok {
                play_drop();
            }
        }
    }
}

fn play_drop() {
    static WAVE: OnceLock<Vec<u8>> = OnceLock::new();
    let wave = WAVE.get_or_init(drop_wave);
    unsafe {
        let _ = PlaySoundW(
            PCWSTR(wave.as_ptr() as *const u16),
            None,
            SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
        );
    }
}

struct Note {
    freq: f64,
    start: f64,
    len: f64,
    gain: f64,
    fall: f64,
}

const NOTES: [Note; 4] = [
    Note {
        freq: 659.25,
        start: 0.0,
        len: 0.9,
        gain: 0.22,
        fall: 7.696,
    },
    Note {
        freq: 1318.5,
        start: 0.0,
        len: 0.4,
        gain: 0.04,
        fall: 5.991,
    },
    Note {
        freq: 987.77,
        start: 0.16,
        len: 1.1,
        gain: 0.20,
        fall: 7.601,
    },
    Note {
        freq: 1975.5,
        start: 0.16,
        len: 0.4,
        gain: 0.03,
        fall: 5.704,
    },
];
const ATTACK: f64 = 0.012;

fn drop_wave() -> Vec<u8> {
    let secs = NOTES.iter().map(|n| n.start + n.len).fold(0.0, f64::max) + 0.05;
    let count = (secs * RATE as f64) as usize;
    let mut samples = vec![0.0f64; count];
    for note in &NOTES {
        let first = (note.start * RATE as f64) as usize;
        let total = (note.len * RATE as f64) as usize;
        let attack = (ATTACK * RATE as f64) as usize;
        let a = note.fall / (total - attack) as f64;
        let decay = 1.0 - a + a * a / 2.0;
        let step = std::f64::consts::TAU * note.freq / RATE as f64;
        let mut level = note.gain;
        for k in 0..total.min(count - first) {
            let envelope = if k < attack {
                note.gain * k as f64 / attack as f64
            } else {
                level *= decay;
                level
            };
            samples[first + k] += envelope * (step * k as f64).sin();
        }
    }

    let data_len = (count * 2) as u32;
    let mut wave = Vec::with_capacity(44 + count * 2);
    let mut put = |bytes: &[u8]| wave.extend_from_slice(bytes);
    put(b"RIFF");
    put(&(36 + data_len).to_le_bytes());
    put(b"WAVEfmt ");
    put(&16u32.to_le_bytes());
    put(&1u16.to_le_bytes());
    put(&1u16.to_le_bytes());
    put(&RATE.to_le_bytes());
    put(&(RATE * 2).to_le_bytes());
    put(&2u16.to_le_bytes());
    put(&16u16.to_le_bytes());
    put(b"data");
    put(&data_len.to_le_bytes());
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
        wave.extend_from_slice(&value.to_le_bytes());
    }
    wave
}

#[cfg(test)]
mod tests {
    #[test]
    fn drop_is_a_valid_wave() {
        let wave = super::drop_wave();
        assert_eq!(&wave[0..4], b"RIFF");
        assert_eq!(&wave[8..16], b"WAVEfmt ");
        let peak = wave[44..]
            .chunks(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]).unsigned_abs())
            .max()
            .unwrap();
        assert!(peak > 8000 && peak < 32767, "{peak}");
    }
}
