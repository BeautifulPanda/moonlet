use std::path::{Path, PathBuf};

use windows::Win32::UI::Shell::SHLoadIndirectString;
use windows::core::HSTRING;

pub struct App {
    pub name: String,
    pub key: String,
    pub file_key: Option<String>,
    pub path: PathBuf,
}

const MAX_DEPTH: u32 = 4;

pub fn scan() -> Vec<App> {
    let mut apps = Vec::new();
    for (variable, tail) in [
        ("APPDATA", r"Microsoft\Windows\Start Menu\Programs"),
        ("ProgramData", r"Microsoft\Windows\Start Menu\Programs"),
    ] {
        if let Some(base) = std::env::var_os(variable) {
            walk(&PathBuf::from(base).join(tail), 0, &mut apps);
        }
    }
    let mut unique: Vec<App> = Vec::with_capacity(apps.len());
    for app in apps {
        if !unique.iter().any(|u| u.key == app.key) {
            unique.push(app);
        }
    }
    unique
}

fn walk(dir: &Path, depth: u32, apps: &mut Vec<App>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let names = localized_names(dir);
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            if depth < MAX_DEPTH {
                walk(&path, depth + 1, apps);
            }
            continue;
        }
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        if !matches!(extension.as_deref(), Some("lnk" | "url")) {
            continue;
        }
        let (Some(stem), Some(file)) = (
            path.file_stem().and_then(|s| s.to_str()),
            path.file_name().and_then(|s| s.to_str()),
        ) else {
            continue;
        };
        let file = file.to_lowercase();
        let name = names
            .iter()
            .find(|(f, _)| *f == file)
            .map_or_else(|| stem.to_string(), |(_, n)| n.clone());
        let key = name.to_lowercase();
        let file_key = Some(stem.to_lowercase()).filter(|f| *f != key);
        apps.push(App {
            name,
            key,
            file_key,
            path,
        });
    }
}

fn localized_names(dir: &Path) -> Vec<(String, String)> {
    let mut names = Vec::new();
    let Ok(bytes) = std::fs::read(dir.join("desktop.ini")) else {
        return names;
    };
    let text = decode(&bytes);
    let mut in_section = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_section = line.eq_ignore_ascii_case("[LocalizedFileNames]");
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((file, value)) = line.split_once('=') else {
            continue;
        };
        let name = if value.starts_with('@') {
            load_indirect(value)
        } else {
            Some(value.to_string())
        };
        if let Some(name) = name.filter(|n| !n.is_empty()) {
            names.push((file.trim().to_lowercase(), name));
        }
    }
    names
}

fn decode(bytes: &[u8]) -> String {
    match bytes {
        [0xFF, 0xFE, rest @ ..] => {
            let wide: Vec<u16> = rest
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&pair| u16::from_le_bytes(pair))
                .collect();
            String::from_utf16_lossy(&wide)
        }
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

fn load_indirect(source: &str) -> Option<String> {
    let mut buffer = [0u16; 260];
    unsafe {
        SHLoadIndirectString(&HSTRING::from(source), &mut buffer, None).ok()?;
    }
    let length = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..length]))
}

pub const PREFIX_SCORE: i32 = 1000;
pub const MAX_LENGTH: i32 = 60;

pub fn score(name: &str, query: &str) -> Option<i32> {
    if query.is_empty() || name.is_empty() {
        return None;
    }
    let length = (name.chars().count() as i32).min(MAX_LENGTH);
    let query_length = query.chars().count();
    let points = if name.starts_with(query) {
        PREFIX_SCORE
    } else if word_starts(name).any(|i| name[i..].starts_with(query)) {
        800
    } else if query_length >= 2 && initials(name).starts_with(query) {
        700
    } else if name.contains(query) {
        600
    } else if query_length >= 3 && is_subsequence(query, name) {
        300
    } else {
        return None;
    };
    Some(points - length)
}

fn word_starts(name: &str) -> impl Iterator<Item = usize> + '_ {
    let mut previous = None;
    name.char_indices().filter_map(move |(i, c)| {
        let start = c.is_alphanumeric() && !previous.is_some_and(char::is_alphanumeric);
        previous = Some(c);
        start.then_some(i)
    })
}

fn initials(name: &str) -> String {
    word_starts(name)
        .filter_map(|i| name[i..].chars().next())
        .collect()
}

fn is_subsequence(query: &str, name: &str) -> bool {
    let mut rest = name.chars();
    query.chars().all(|q| rest.any(|c| c == q))
}
