use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::SystemInformation::GetLocalTime;

use crate::config::SatelliteKind;
use crate::lang::{is_english, tr};

const CLOCK_COLOR: u32 = 0x4FD1C5;
#[cfg(feature = "music")]
const MUSIC_COLOR: u32 = 0xB794F4;
const MUSIC_IDLE: u32 = 0xF4F6FB;
const BATTERY_OK: u32 = 0x68D391;
const BATTERY_LOW: u32 = 0xF6AD55;
const BATTERY_CRITICAL: u32 = 0xFC8181;
const BATTERY_CHARGING: u32 = 0x63B3ED;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub color: u32,
    pub progress: f32,
}

impl Look {
    pub fn differs(&self, other: &Look) -> bool {
        self.color != other.color || (self.progress - other.progress).abs() > 0.002
    }
}

pub struct Card {
    pub title: &'static str,
    pub value: String,
    pub detail: String,
}

pub fn label(kind: SatelliteKind) -> &'static str {
    match kind {
        SatelliteKind::Clock => tr("Часы", "Clock"),
        SatelliteKind::Music => tr("Музыка", "Music"),
        SatelliteKind::Battery => tr("Батарея", "Battery"),
    }
}

#[cfg(feature = "music")]
pub fn music_look(track: Option<&super::media::Track>, t: f64) -> Look {
    match track {
        Some(track) => Look {
            color: if track.playing {
                MUSIC_COLOR
            } else {
                crate::gfx::mix(MUSIC_COLOR, MUSIC_IDLE, 0.5)
            },
            progress: track.progress(t),
        },
        None => idle_music(),
    }
}

fn idle_music() -> Look {
    Look {
        color: MUSIC_IDLE,
        progress: 0.0,
    }
}

pub fn look(kind: SatelliteKind) -> Look {
    match kind {
        SatelliteKind::Clock => {
            let t = unsafe { GetLocalTime() };
            let seconds = t.wMinute as f32 * 60.0 + t.wSecond as f32;
            Look {
                color: CLOCK_COLOR,
                progress: seconds / 3600.0,
            }
        }
        SatelliteKind::Music => idle_music(),
        SatelliteKind::Battery => {
            let battery = Battery::read();
            Look {
                color: battery.color(),
                progress: battery.percent as f32 / 100.0,
            }
        }
    }
}

pub fn card(kind: SatelliteKind) -> Card {
    match kind {
        SatelliteKind::Clock => {
            let t = unsafe { GetLocalTime() };
            let weekday = t.wDayOfWeek as usize % 7;
            let month = (t.wMonth as usize).clamp(1, 12) - 1;
            Card {
                title: tr("Часы", "Clock"),
                value: format!("{:02}:{:02}", t.wHour, t.wMinute),
                detail: if is_english() {
                    format!("{}, {} {}", WEEKDAYS_EN[weekday], MONTHS_EN[month], t.wDay)
                } else {
                    format!("{}, {} {}", WEEKDAYS[weekday], t.wDay, MONTHS[month])
                },
            }
        }
        SatelliteKind::Music => Card {
            title: tr("Музыка", "Music"),
            value: "—".to_string(),
            detail: String::new(),
        },
        SatelliteKind::Battery => {
            let battery = Battery::read();
            Card {
                title: tr("Батарея", "Battery"),
                value: format!("{}%", battery.percent),
                detail: battery.detail(),
            }
        }
    }
}

pub struct Notice {
    pub kind: Option<SatelliteKind>,
    pub text: String,
    pub look: Look,
    pub long: bool,
}

pub fn power_notice(on_battery: bool) -> Option<Notice> {
    if !battery_present() {
        return None;
    }
    let battery = Battery::read();
    let text = if on_battery {
        tr("От батареи", "On battery")
    } else {
        tr("Зарядка", "Charging")
    };
    Some(Notice {
        kind: Some(SatelliteKind::Battery),
        text: format!("{text} · {}%", battery.percent),
        look: look(SatelliteKind::Battery),
        long: false,
    })
}

pub fn low_battery_notice(warned: &mut Option<u8>) -> Option<Notice> {
    if !battery_present() {
        return None;
    }
    let battery = Battery::read();
    let (now_warned, crossed) = low_battery_step(battery.percent, battery.plugged, *warned);
    *warned = now_warned;
    crossed?;
    Some(Notice {
        kind: Some(SatelliteKind::Battery),
        text: format!(
            "{} · {}%",
            tr("Мало заряда", "Low battery"),
            battery.percent
        ),
        look: look(SatelliteKind::Battery),
        long: true,
    })
}

fn low_battery_step(percent: u8, plugged: bool, warned: Option<u8>) -> (Option<u8>, Option<u8>) {
    if plugged {
        return (None, None);
    }
    match [10, 20].into_iter().find(|&level| percent <= level) {
        Some(level) if warned.is_none_or(|w| level < w) => (Some(level), Some(level)),
        _ => (warned, None),
    }
}

#[cfg(feature = "music")]
pub fn track_notice(track: &super::media::Track) -> Notice {
    let text = if track.artist.is_empty() {
        track.title.clone()
    } else {
        format!("{} — {}", track.title, track.artist)
    };
    Notice {
        kind: Some(SatelliteKind::Music),
        text,
        look: music_look(Some(track), super::motion::now()),
        long: false,
    }
}

pub fn ms_to_next_minute() -> u32 {
    let t = unsafe { GetLocalTime() };
    let left = (60 - t.wSecond as u32) * 1000 - t.wMilliseconds as u32;
    left + 30
}

pub fn battery_present() -> bool {
    crate::power::has_battery()
}

fn status() -> Option<SYSTEM_POWER_STATUS> {
    let mut status = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut status) }.ok()?;
    Some(status)
}

struct Battery {
    percent: u8,
    charging: bool,
    plugged: bool,
    seconds_left: Option<u32>,
}

impl Battery {
    fn read() -> Self {
        let s = status().unwrap_or_default();
        Self {
            percent: if s.BatteryLifePercent <= 100 {
                s.BatteryLifePercent
            } else {
                0
            },
            charging: s.BatteryFlag & 8 != 0,
            plugged: s.ACLineStatus == 1,
            seconds_left: (s.BatteryLifeTime != u32::MAX).then_some(s.BatteryLifeTime),
        }
    }

    fn color(&self) -> u32 {
        match self.percent {
            _ if self.charging => BATTERY_CHARGING,
            0..15 => BATTERY_CRITICAL,
            15..30 => BATTERY_LOW,
            _ => BATTERY_OK,
        }
    }

    fn detail(&self) -> String {
        if self.charging {
            return tr("заряжается", "charging").to_string();
        }
        if self.plugged {
            return tr("от сети", "plugged in").to_string();
        }
        match self.seconds_left {
            Some(seconds) => {
                let minutes = seconds / 60;
                match minutes / 60 {
                    0 if is_english() => format!("on battery · ≈ {minutes} min"),
                    0 => format!("от батареи · ≈ {minutes} мин"),
                    hours if is_english() => {
                        format!("on battery · ≈ {hours} h {} min", minutes % 60)
                    }
                    hours => format!("от батареи · ≈ {hours} ч {} мин", minutes % 60),
                }
            }
            None => tr("от батареи", "on battery").to_string(),
        }
    }
}

const WEEKDAYS: [&str; 7] = [
    "воскресенье",
    "понедельник",
    "вторник",
    "среда",
    "четверг",
    "пятница",
    "суббота",
];

const WEEKDAYS_EN: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

const MONTHS_EN: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

const MONTHS: [&str; 12] = [
    "января",
    "февраля",
    "марта",
    "апреля",
    "мая",
    "июня",
    "июля",
    "августа",
    "сентября",
    "октября",
    "ноября",
    "декабря",
];

#[cfg(test)]
mod tests {
    use super::low_battery_step;

    #[test]
    fn warns_once_per_threshold() {
        assert_eq!(low_battery_step(35, false, None), (None, None));
        assert_eq!(low_battery_step(20, false, None), (Some(20), Some(20)));
        assert_eq!(low_battery_step(18, false, Some(20)), (Some(20), None));
        assert_eq!(low_battery_step(10, false, Some(20)), (Some(10), Some(10)));
        assert_eq!(low_battery_step(7, false, Some(10)), (Some(10), None));
        assert_eq!(low_battery_step(8, false, None), (Some(10), Some(10)));
    }

    #[test]
    fn charging_resets_warnings() {
        assert_eq!(low_battery_step(15, true, Some(20)), (None, None));
        assert_eq!(low_battery_step(15, false, None), (Some(20), Some(20)));
    }
}
