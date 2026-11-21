use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, SYSTEMTIME};
use windows::Win32::System::Threading::{CreateWaitableTimerW, SetWaitableTimer};
use windows::Win32::System::Time::{
    FileTimeToSystemTime, SystemTimeToFileTime, SystemTimeToTzSpecificLocalTime,
    TzSpecificLocalTimeToSystemTime,
};
use windows::core::PCWSTR;

use crate::lang::{is_english, tr};

pub const MAX_REMINDERS: usize = 20;
const MINUTE: i64 = 60;
const HOUR: i64 = 3600;
const DAY: i64 = 86400;
const MAX_SECS: i64 = 31 * DAY;

#[derive(Clone, Debug, PartialEq)]
pub struct Reminder {
    pub id: u64,
    pub created: i64,
    pub due: i64,
    pub timer: bool,
    pub text: String,
}

impl Reminder {
    pub fn left_fraction(&self, now: i64) -> f32 {
        let total = (self.due - self.created).max(1) as f32;
        ((self.due - now) as f32 / total).clamp(0.0, 1.0)
    }

    pub fn fire_text(&self) -> String {
        if !self.text.is_empty() {
            return self.text.clone();
        }
        if self.timer {
            let secs = self.due - self.created;
            let what = if secs % MINUTE == 0 {
                duration_text(secs)
            } else {
                due_text(self.due, self.due)
            };
            format!("{} · {what}", tr("Время вышло", "Time's up"))
        } else {
            tr("Напоминание", "Reminder").to_string()
        }
    }

    #[cfg_attr(not(feature = "settings"), allow(dead_code))]
    pub fn title(&self) -> String {
        if self.timer {
            let base = format!(
                "{} {}",
                tr("Таймер на", "Timer for"),
                duration_text(self.due - self.created)
            );
            if self.text.is_empty() {
                base
            } else {
                format!("{base} · {}", self.text)
            }
        } else if self.text.is_empty() {
            tr("Напоминание", "Reminder").to_string()
        } else {
            self.text.clone()
        }
    }
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum When {
    In(i64),
    At {
        days: Option<u8>,
        hour: u8,
        minute: u8,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub timer: bool,
    pub when: When,
    pub text: String,
}

impl Request {
    pub fn title(&self) -> String {
        let when = match self.when {
            When::In(secs) if self.timer => {
                format!("{} {}", tr("Таймер на", "Timer for"), duration_text(secs))
            }
            When::In(secs) => format!("{} {}", tr("через", "in"), duration_text(secs)),
            When::At { days, hour, minute } => {
                let day = match days {
                    Some(1) => tr("завтра ", "tomorrow "),
                    Some(2) => tr("послезавтра ", "the day after tomorrow "),
                    Some(0) => tr("сегодня ", "today "),
                    _ => "",
                };
                if self.timer {
                    format!("{} {day}{hour}:{minute:02}", tr("Таймер до", "Timer until"))
                } else {
                    format!("{day}{} {hour}:{minute:02}", tr("в", "at"))
                }
            }
        };
        match (self.timer, self.text.is_empty()) {
            (true, true) => when,
            (true, false) => format!("{when} · {}", self.text),
            (false, true) => format!("{} {when}", tr("Напомнить", "Remind")),
            (false, false) => format!("{} {when}: {}", tr("Напомнить", "Remind"), self.text),
        }
    }

    pub fn resolve(&self, now: i64) -> Option<Reminder> {
        let due = match self.when {
            When::In(secs) => now + secs,
            When::At { days, hour, minute } => {
                let today = at_local(now, 0, hour, minute)?;
                match days {
                    None if today > now => today,
                    None => at_local(now, 1, hour, minute)?,
                    Some(days) => at_local(now, days as i64, hour, minute)?,
                }
            }
        };
        if due <= now {
            return None;
        }
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        Some(Reminder {
            id,
            created: now,
            due,
            timer: self.timer,
            text: self.text.clone(),
        })
    }
}

pub fn duration_text(secs: i64) -> String {
    let secs = secs.max(0);
    let (d, h, m, s) = (
        secs / DAY,
        secs % DAY / HOUR,
        secs % HOUR / MINUTE,
        secs % MINUTE,
    );
    let mut parts = Vec::new();
    if d > 0 {
        parts.push(format!("{d} {}", tr("дн", "d")));
    }
    if h > 0 {
        parts.push(format!("{h} {}", tr("ч", "h")));
    }
    if m > 0 {
        parts.push(format!("{m} {}", tr("мин", "min")));
    }
    if s > 0 && d == 0 && h == 0 {
        parts.push(format!("{s} {}", tr("сек", "s")));
    }
    if parts.is_empty() {
        parts.push(format!("0 {}", tr("сек", "s")));
    }
    parts.join(" ")
}

pub fn left_text(secs: i64) -> String {
    let secs = secs.max(0);
    if secs >= HOUR {
        format!(
            "{}:{:02}:{:02}",
            secs / HOUR,
            secs % HOUR / MINUTE,
            secs % MINUTE
        )
    } else {
        format!("{}:{:02}", secs / MINUTE, secs % MINUTE)
    }
}

pub fn due_text(due: i64, now: i64) -> String {
    let (Some(at), Some(today)) = (to_local(due), to_local(now)) else {
        return String::new();
    };
    let time = format!("{}:{:02}", at.wHour, at.wMinute);
    let days = day_number(&at) - day_number(&today);
    match days {
        0 => time,
        1 => format!("{} {time}", tr("завтра", "tomorrow")),
        _ if is_english() => format!("{}/{} {time}", at.wMonth, at.wDay),
        _ => format!("{:02}.{:02} {time}", at.wDay, at.wMonth),
    }
}

fn day_number(t: &SYSTEMTIME) -> i64 {
    let mut midnight = *t;
    midnight.wHour = 0;
    midnight.wMinute = 0;
    midnight.wSecond = 0;
    midnight.wMilliseconds = 0;
    file_secs(&midnight).map_or(0, |secs| secs / DAY)
}

const EPOCH_1601: i64 = 11_644_473_600;

fn to_filetime(unix: i64) -> FILETIME {
    let ticks = ((unix + EPOCH_1601) * 10_000_000) as u64;
    FILETIME {
        dwLowDateTime: ticks as u32,
        dwHighDateTime: (ticks >> 32) as u32,
    }
}

fn file_secs(t: &SYSTEMTIME) -> Option<i64> {
    let mut file = FILETIME::default();
    unsafe { SystemTimeToFileTime(t, &mut file).ok()? };
    let ticks = ((file.dwHighDateTime as u64) << 32) | file.dwLowDateTime as u64;
    Some(ticks as i64 / 10_000_000 - EPOCH_1601)
}

pub fn to_local(unix: i64) -> Option<SYSTEMTIME> {
    let mut utc = SYSTEMTIME::default();
    let mut local = SYSTEMTIME::default();
    unsafe {
        FileTimeToSystemTime(&to_filetime(unix), &mut utc).ok()?;
        SystemTimeToTzSpecificLocalTime(None, &utc, &mut local).ok()?;
    }
    Some(local)
}

fn at_local(now: i64, days: i64, hour: u8, minute: u8) -> Option<i64> {
    let today = to_local(now)?;
    let mut day = SYSTEMTIME::default();
    let shifted = file_secs(&today)? + days * DAY;
    unsafe { FileTimeToSystemTime(&to_filetime(shifted), &mut day).ok()? };
    day.wHour = hour as u16;
    day.wMinute = minute as u16;
    day.wSecond = 0;
    day.wMilliseconds = 0;
    let mut utc = SYSTEMTIME::default();
    unsafe { TzSpecificLocalTimeToSystemTime(None, &day, &mut utc).ok()? };
    file_secs(&utc)
}

struct Word<'a> {
    raw: &'a str,
    low: String,
}

fn words(text: &str) -> Vec<Word<'_>> {
    let mut out = Vec::new();
    for raw in text.split_whitespace() {
        let chars: Vec<(usize, char)> = raw.char_indices().collect();
        let digit = |k: usize| {
            let c = chars[k].1;
            c.is_ascii_digit()
                || (matches!(c, ':' | '.' | ',')
                    && k > 0
                    && chars[k - 1].1.is_ascii_digit()
                    && chars.get(k + 1).is_some_and(|n| n.1.is_ascii_digit()))
        };
        let mut start = 0;
        for k in 1..chars.len() {
            if digit(k) != digit(k - 1) {
                out.push(&raw[start..chars[k].0]);
                start = chars[k].0;
            }
        }
        out.push(&raw[start..]);
    }
    out.into_iter()
        .filter(|w| !w.is_empty())
        .map(|raw| Word {
            raw,
            low: raw.to_lowercase(),
        })
        .collect()
}

fn number_word(word: &str) -> Option<f64> {
    Some(match word {
        "один" | "одна" | "одну" | "one" | "a" | "an" => 1.0,
        "два" | "две" | "two" => 2.0,
        "три" | "three" => 3.0,
        "пять" | "five" => 5.0,
        "десять" | "ten" => 10.0,
        "пятнадцать" | "fifteen" => 15.0,
        "двадцать" | "twenty" => 20.0,
        "тридцать" | "thirty" => 30.0,
        "сорок" | "forty" => 40.0,
        "пару" | "couple" => 2.0,
        _ => return None,
    })
}

fn unit(word: &str) -> Option<i64> {
    let starts = |prefixes: &[&str]| prefixes.iter().any(|p| word.starts_with(p));
    if matches!(word, "с" | "s" | "сек" | "sec") || starts(&["секунд", "second", "secs"])
    {
        Some(1)
    } else if matches!(word, "м" | "m" | "мин" | "min" | "mins") || starts(&["минут", "minute"])
    {
        Some(MINUTE)
    } else if matches!(word, "ч" | "h" | "hr" | "hrs") || starts(&["час", "hour"]) {
        Some(HOUR)
    } else if matches!(word, "д" | "d" | "дн") || starts(&["день", "дня", "дней", "day"])
    {
        Some(DAY)
    } else {
        None
    }
}

fn number(word: &str) -> Option<f64> {
    if word.chars().next()?.is_ascii_digit() {
        word.replace(',', ".").parse().ok()
    } else {
        number_word(word)
    }
}

fn duration(w: &[Word], mut i: usize) -> Option<(i64, usize)> {
    let mut total = 0.0;
    let mut any = false;
    while let Some(word) = w.get(i) {
        let low = word.low.as_str();
        if matches!(low, "полчаса" | "half") {
            total += 1800.0;
            i += 1;
            if low == "half" {
                if w.get(i)
                    .is_some_and(|w| matches!(w.low.as_str(), "an" | "a"))
                {
                    i += 1;
                }
                if w.get(i).is_some_and(|w| unit(&w.low) == Some(HOUR)) {
                    i += 1;
                } else {
                    return None;
                }
            }
            any = true;
            continue;
        }
        if any && matches!(low, "и" | "and") {
            i += 1;
            continue;
        }
        if low.contains(':') {
            let parts: Option<Vec<i64>> = low.split(':').map(|p| p.parse().ok()).collect();
            let parts = parts?;
            let secs = match parts[..] {
                [m, s] if s < 60 => m * MINUTE + s,
                [h, m, s] if m < 60 && s < 60 => h * HOUR + m * MINUTE + s,
                _ => return None,
            };
            total += secs as f64;
            i += 1;
            any = true;
            continue;
        }
        if let Some(value) = number(low) {
            match w.get(i + 1).and_then(|next| unit(&next.low)) {
                Some(size) => {
                    total += value * size as f64;
                    i += 2;
                }
                None if value.fract() == 0.0 && low.chars().all(|c| c.is_ascii_digit()) => {
                    total += value * MINUTE as f64;
                    i += 1;
                    any = true;
                    break;
                }
                None => break,
            }
            any = true;
            continue;
        }
        if !any && let Some(size) = unit(low) {
            total += size as f64;
            i += 1;
            any = true;
            continue;
        }
        break;
    }
    let secs = total.round() as i64;
    (any && secs > 0 && secs <= MAX_SECS).then_some((secs, i))
}

fn clock(w: &[Word], i: usize) -> Option<(u8, u8, usize)> {
    let word = &w.get(i)?.low;
    let (hour, minute) = match word.split_once([':', '.']) {
        Some((h, m)) if m.len() == 2 => (h.parse::<u8>().ok()?, m.parse::<u8>().ok()?),
        Some(_) => return None,
        None if word.chars().all(|c| c.is_ascii_digit()) && word.len() <= 2 => {
            (word.parse::<u8>().ok()?, 0)
        }
        None => return None,
    };
    let mut next = i + 1;
    let mut hour = hour;
    if w.get(next).is_some_and(|w| unit(&w.low) == Some(HOUR)) {
        next += 1;
    }
    match w.get(next).map(|w| w.low.as_str()) {
        Some("утра" | "am" | "a.m.") => {
            hour %= 12;
            next += 1;
        }
        Some("вечера" | "дня" | "pm" | "p.m.") => {
            if hour < 12 {
                hour += 12;
            }
            next += 1;
        }
        Some("ночи") => {
            hour %= 12;
            next += 1;
        }
        _ => {}
    }
    (hour < 24 && minute < 60).then_some((hour, minute, next))
}

fn exact_clock(w: &[Word], i: usize) -> Option<(u8, u8, usize)> {
    let found = clock(w, i)?;
    let word = &w.get(i)?.low;
    (word.contains([':', '.']) || found.2 > i + 1).then_some(found)
}

fn at_clock((hour, minute, next): (u8, u8, usize)) -> (When, usize) {
    (
        When::At {
            days: None,
            hour,
            minute,
        },
        next,
    )
}

fn when(w: &[Word], i: usize) -> Option<(When, usize)> {
    let first = w.get(i)?.low.as_str();
    match first {
        "через" | "in" => duration(w, i + 1).map(|(secs, next)| (When::In(secs), next)),
        "на" | "до" | "by" | "until" => exact_clock(w, i + 1).map(at_clock),
        "в" | "во" | "at" | "к" => clock(w, i + 1).map(|(hour, minute, next)| {
            (
                When::At {
                    days: None,
                    hour,
                    minute,
                },
                next,
            )
        }),
        "сегодня" | "today" | "завтра" | "tomorrow" | "послезавтра" => {
            let days = match first {
                "сегодня" | "today" => 0,
                "завтра" | "tomorrow" => 1,
                _ => 2,
            };
            let mut next = i + 1;
            if w.get(next)
                .is_some_and(|w| matches!(w.low.as_str(), "в" | "во" | "at" | "к"))
            {
                next += 1;
            }
            match clock(w, next) {
                Some((hour, minute, next)) => Some((
                    When::At {
                        days: Some(days),
                        hour,
                        minute,
                    },
                    next,
                )),
                None if days > 0 && next == i + 1 => Some((
                    When::At {
                        days: Some(days),
                        hour: 9,
                        minute: 0,
                    },
                    next,
                )),
                None => None,
            }
        }
        _ => None,
    }
}

fn join(w: &[Word], from: usize, to: usize) -> String {
    let parts: Vec<&str> = w[from..to].iter().map(|w| w.raw).collect();
    parts
        .join(" ")
        .trim_matches(|c: char| c == ':' || c == ',' || c == '-' || c == '—' || c.is_whitespace())
        .to_string()
}

pub fn is_timer_word(word: &str) -> bool {
    matches!(word, "таймер" | "timer" | "засеки") || is_alarm_word(word)
}

fn is_alarm_word(word: &str) -> bool {
    matches!(word, "будильник" | "alarm")
}

pub fn is_remind_word(word: &str) -> bool {
    word.starts_with("напомн")
        || word.starts_with("напомин")
        || matches!(word, "remind" | "reminder")
}

pub fn parse(text: &str) -> Option<Request> {
    let w = words(text);
    let head = w.first()?.low.as_str();
    if is_timer_word(head) {
        let until = |i: usize| match w.get(i).map(|w| w.low.as_str()) {
            Some("до" | "until" | "till") => clock(&w, i + 1),
            _ if is_alarm_word(head) => {
                let skip = w
                    .get(i)
                    .is_some_and(|w| matches!(w.low.as_str(), "на" | "в" | "во" | "at" | "for"));
                clock(&w, i + usize::from(skip))
            }
            _ => None,
        };
        if let Some(found) = until(1) {
            let (when, next) = at_clock(found);
            return Some(Request {
                timer: true,
                when,
                text: join(&w, next, w.len()),
            });
        }
        if is_alarm_word(head) {
            return None;
        }
        let mut i = 1;
        if w.get(i)
            .is_some_and(|w| matches!(w.low.as_str(), "на" | "for"))
        {
            i += 1;
        }
        let (secs, next) = duration(&w, i)?;
        return Some(Request {
            timer: true,
            when: When::In(secs),
            text: join(&w, next, w.len()),
        });
    }
    if !is_remind_word(head) {
        return None;
    }
    let mut i = 1;
    while w
        .get(i)
        .is_some_and(|w| matches!(w.low.as_str(), "мне" | "me" | "to"))
    {
        i += 1;
    }
    if let Some((when, next)) = when(&w, i) {
        return Some(Request {
            timer: false,
            when,
            text: join(&w, next, w.len()),
        });
    }
    for start in i + 1..w.len() {
        if let Some((when, next)) = when(&w, start)
            && next == w.len()
        {
            return Some(Request {
                timer: false,
                when,
                text: join(&w, i, start),
            });
        }
    }
    None
}

pub fn path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(appdata).join("Moonlet").join("reminders.txt"))
}

fn parse_line(line: &str) -> Option<Reminder> {
    let mut parts = line.splitn(5, ' ');
    let id = parts.next()?.parse().ok()?;
    let created = parts.next()?.parse().ok()?;
    let due = parts.next()?.parse().ok()?;
    let timer = match parts.next()? {
        "t" => true,
        "r" => false,
        _ => return None,
    };
    let text = parts.next().unwrap_or("").to_string();
    Some(Reminder {
        id,
        created,
        due,
        timer,
        text,
    })
}

pub fn from_text(text: &str) -> Vec<Reminder> {
    let mut list: Vec<Reminder> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(parse_line)
        .collect();
    by_due(&mut list);
    list.truncate(MAX_REMINDERS);
    list
}

fn by_due(list: &mut [Reminder]) {
    for i in 1..list.len() {
        let mut j = i;
        while j > 0 && list[j - 1].due > list[j].due {
            list.swap(j - 1, j);
            j -= 1;
        }
    }
}

pub fn to_text(list: &[Reminder]) -> String {
    let mut text = String::from(
        "# Moonlet: timers and reminders (id created due t|r text). Times are Unix seconds.\n",
    );
    for r in list {
        let kind = if r.timer { 't' } else { 'r' };
        let line = r.text.replace(['\r', '\n'], " ");
        text += &format!("{} {} {} {kind} {line}\n", r.id, r.created, r.due);
    }
    text
}

pub fn load() -> Vec<Reminder> {
    path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map_or_else(Vec::new, |text| from_text(&text))
}

pub fn save(list: &[Reminder]) -> std::io::Result<()> {
    let Some(path) = path() else {
        return Ok(());
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, to_text(list))?;
    std::fs::rename(&temp, &path)
}

pub fn add(reminder: Reminder) -> bool {
    let mut list = load();
    if list.len() >= MAX_REMINDERS {
        return false;
    }
    list.push(reminder);
    by_due(&mut list);
    save(&list).is_ok()
}

pub fn remove(id: u64) {
    let mut list = load();
    let before = list.len();
    list.retain(|r| r.id != id);
    if list.len() != before {
        let _ = save(&list);
    }
}

pub struct Alarm(HANDLE);

impl Alarm {
    pub fn new() -> Option<Self> {
        unsafe {
            CreateWaitableTimerW(None, false, PCWSTR::null())
                .ok()
                .map(Self)
        }
    }

    pub fn handle(&self) -> HANDLE {
        self.0
    }

    pub fn set(&self, due: i64) {
        let file = to_filetime(due);
        let ticks = (((file.dwHighDateTime as u64) << 32) | file.dwLowDateTime as u64) as i64;
        unsafe {
            let _ = SetWaitableTimer(self.0, &ticks, 0, None, None, false);
        }
    }
}

impl Drop for Alarm {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(days: Option<u8>, hour: u8, minute: u8) -> When {
        When::At { days, hour, minute }
    }

    fn check(text: &str, timer: bool, when: When, rest: &str) {
        let request = parse(text).unwrap_or_else(|| panic!("{text}"));
        assert_eq!(
            request,
            Request {
                timer,
                when,
                text: rest.into()
            },
            "{text}"
        );
    }

    #[test]
    fn parses_timers() {
        check("таймер 10 мин", true, When::In(600), "");
        check("таймер 10", true, When::In(600), "");
        check("Таймер на 1 ч 30 мин чай", true, When::In(5400), "чай");
        check("таймер 90 сек", true, When::In(90), "");
        check("таймер 1:30", true, When::In(90), "");
        check("таймер 20мин", true, When::In(1200), "");
        check("timer 5 min pizza", true, When::In(300), "pizza");
        check("timer 2h", true, When::In(7200), "");
        check("таймер полчаса", true, When::In(1800), "");
        check("таймер до 11:10", true, at(None, 11, 10), "");
        check("таймер до 7 вечера пицца", true, at(None, 19, 0), "пицца");
        check("будильник 7:30", true, at(None, 7, 30), "");
        check("будильник на 6 утра", true, at(None, 6, 0), "");
        check("timer until 18:00", true, at(None, 18, 0), "");
        check("alarm at 7:15 wake up", true, at(None, 7, 15), "wake up");
        check("таймер на 1:30", true, When::In(90), "");
        assert!(parse("будильник").is_none());
        assert!(parse("таймер").is_none());
        assert!(parse("таймер чай").is_none());
        assert!(parse("таймер 0").is_none());
    }

    #[test]
    fn parses_reminders() {
        check(
            "напомни через 20 мин позвонить маме",
            false,
            When::In(1200),
            "позвонить маме",
        );
        check("напомни через час", false, When::In(3600), "");
        check(
            "Напомни мне в 15:30 встреча",
            false,
            at(None, 15, 30),
            "встреча",
        );
        check(
            "напомни завтра в 9 купить хлеб",
            false,
            at(Some(1), 9, 0),
            "купить хлеб",
        );
        check(
            "напомни завтра купить хлеб",
            false,
            at(Some(1), 9, 0),
            "купить хлеб",
        );
        check("напомни в 7 вечера кино", false, at(None, 19, 0), "кино");
        check(
            "напомни позвонить в банк через 2 часа",
            false,
            When::In(7200),
            "позвонить в банк",
        );
        check(
            "remind me in 15 minutes to stretch",
            false,
            When::In(900),
            "to stretch",
        );
        check("remind me at 9am standup", false, at(None, 9, 0), "standup");
        check(
            "remind tomorrow at 18:45 gym",
            false,
            at(Some(1), 18, 45),
            "gym",
        );
        check(
            "remind me to call mom in an hour",
            false,
            When::In(3600),
            "call mom",
        );
        check(
            "напомни на 11:00 проверить звук",
            false,
            at(None, 11, 0),
            "проверить звук",
        );
        check(
            "напомни проверить звук на 11:00",
            false,
            at(None, 11, 0),
            "проверить звук",
        );
        check(
            "напомни на 9 утра зарядку",
            false,
            at(None, 9, 0),
            "зарядку",
        );
        assert!(parse("напомни на 5 минут").is_none());
        assert!(parse("напомни").is_none());
        assert!(parse("напомни позвонить").is_none());
        assert!(parse("напомни в 25:00").is_none());
        assert!(parse("блокнот").is_none());
    }

    #[test]
    fn file_round_trip() {
        let list = vec![
            Reminder {
                id: 2,
                created: 100,
                due: 900,
                timer: false,
                text: "позвонить маме".into(),
            },
            Reminder {
                id: 1,
                created: 100,
                due: 700,
                timer: true,
                text: String::new(),
            },
        ];
        let back = from_text(&to_text(&list));
        assert_eq!(back.len(), 2);
        assert_eq!(back[0], list[1]);
        assert_eq!(back[1], list[0]);
        assert!(from_text("мусор\n1 2\n").is_empty());
    }

    #[test]
    fn formats_times() {
        assert_eq!(left_text(65), "1:05");
        assert_eq!(left_text(3725), "1:02:05");
        crate::lang::set(crate::lang::Language::Russian);
        assert_eq!(duration_text(5400), "1 ч 30 мин");
        assert_eq!(duration_text(90), "1 мин 30 сек");
    }

    #[test]
    fn resolves_local_times() {
        let now = super::now();
        let r = parse("напомни в 3:00 x").unwrap().resolve(now).unwrap();
        assert!(r.due > now && r.due <= now + DAY + HOUR);
        let local = to_local(r.due).unwrap();
        assert_eq!((local.wHour, local.wMinute), (3, 0));
        let r = parse("таймер 10 мин").unwrap().resolve(now).unwrap();
        assert_eq!(r.due - r.created, 600);
    }
}
