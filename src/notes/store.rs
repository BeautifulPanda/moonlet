use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Top,
    Desktop,
    Normal,
}

impl Mode {
    pub const ALL: [(Mode, &'static str); 3] = [
        (Mode::Top, "top"),
        (Mode::Desktop, "desktop"),
        (Mode::Normal, "normal"),
    ];

    pub fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(m, _)| *m == self)
            .map_or("top", |(_, name)| name)
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().find(|(_, n)| *n == name).map(|(m, _)| *m)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attach {
    pub exe: String,
    pub tab: String,
    pub corner: u8,
    pub dx: i32,
    pub dy: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub id: u64,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub zoom: u32,
    pub mode: Mode,
    pub attach: Option<Attach>,
}

pub fn dir() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(appdata).join("Moonlet").join("notes"))
}

fn text_path(id: u64) -> Option<PathBuf> {
    Some(dir()?.join(format!("{id}.txt")))
}

fn to_line(note: &Note) -> String {
    let mut line = format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t",
        note.id, note.x, note.y, note.w, note.h, note.zoom
    );
    match &note.attach {
        None => line += note.mode.name(),
        Some(a) => {
            let tab = a.tab.replace(['\t', '\r', '\n'], " ");
            line += &format!("window\t{}\t{}\t{}\t{}\t{tab}", a.corner, a.dx, a.dy, a.exe);
        }
    }
    line
}

fn from_line(line: &str) -> Option<Note> {
    let fields: Vec<&str> = line.split('\t').collect();
    let int = |i: usize| fields.get(i)?.trim().parse::<i32>().ok();
    let mut note = Note {
        id: fields.first()?.trim().parse().ok()?,
        x: int(1)?,
        y: int(2)?,
        w: int(3)?.max(1),
        h: int(4)?.max(1),
        zoom: (int(5)?.clamp(25, 400)) as u32,
        mode: Mode::Top,
        attach: None,
    };
    let mode = fields.get(6)?.trim();
    if mode == "window" {
        note.attach = Some(Attach {
            corner: (int(7)?.clamp(0, 3)) as u8,
            dx: int(8)?,
            dy: int(9)?,
            exe: fields.get(10)?.trim().to_lowercase(),
            tab: fields.get(11).map_or("", |t| t.trim()).to_string(),
        });
        if note.attach.as_ref()?.exe.is_empty() {
            note.attach = None;
        }
    } else {
        note.mode = Mode::from_name(mode)?;
    }
    Some(note)
}

pub fn from_text(text: &str) -> Vec<Note> {
    let mut notes: Vec<Note> = Vec::new();
    for note in text.lines().filter_map(from_line) {
        if !notes.iter().any(|n| n.id == note.id) {
            notes.push(note);
        }
    }
    notes
}

pub fn to_text(notes: &[Note]) -> String {
    let mut text = String::new();
    for note in notes {
        text += &to_line(note);
        text += "\r\n";
    }
    text
}

pub fn load() -> Vec<Note> {
    let Some(path) = dir().map(|d| d.join("layout.txt")) else {
        return Vec::new();
    };
    from_text(&std::fs::read_to_string(path).unwrap_or_default())
}

pub fn save(notes: &[Note]) {
    let Some(dir) = dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let temp = dir.join("layout.tmp");
    if std::fs::write(&temp, to_text(notes)).is_ok() {
        let _ = std::fs::rename(&temp, dir.join("layout.txt"));
    }
}

pub fn read_text(id: u64) -> String {
    text_path(id)
        .and_then(|path| std::fs::read(path).ok())
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default()
}

pub fn write_text(id: u64, text: &str) {
    let Some(path) = text_path(id) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let temp = path.with_extension("tmp");
    if std::fs::write(&temp, text).is_ok() {
        let _ = std::fs::rename(&temp, &path);
    }
}

#[cfg_attr(not(feature = "settings"), allow(dead_code))]
pub fn delete(id: u64) {
    if let Some(path) = text_path(id) {
        let _ = std::fs::remove_file(path);
    }
    let mut notes = load();
    let before = notes.len();
    notes.retain(|n| n.id != id);
    if notes.len() != before {
        save(&notes);
    }
}

pub fn next_id(notes: &[Note]) -> u64 {
    notes.iter().map(|n| n.id).max().unwrap_or(0) + 1
}

#[cfg_attr(not(feature = "settings"), allow(dead_code))]
pub fn first_line(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty());
    let line = line.unwrap_or("");
    let mut short: String = line.chars().take(48).collect();
    if line.chars().count() > 48 {
        short.push('…');
    }
    short
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(id: u64, attach: Option<Attach>) -> Note {
        Note {
            id,
            x: -100,
            y: 20,
            w: 240,
            h: 180,
            zoom: 110,
            mode: if attach.is_some() {
                Mode::Top
            } else {
                Mode::Desktop
            },
            attach,
        }
    }

    #[test]
    fn layout_round_trips() {
        let notes = vec![
            note(1, None),
            note(
                7,
                Some(Attach {
                    exe: "chrome.exe".into(),
                    tab: "YouTube".into(),
                    corner: 3,
                    dx: -250,
                    dy: -40,
                }),
            ),
            note(
                9,
                Some(Attach {
                    exe: "telegram.exe".into(),
                    tab: String::new(),
                    corner: 0,
                    dx: 12,
                    dy: 60,
                }),
            ),
        ];
        assert_eq!(from_text(&to_text(&notes)), notes);
    }

    #[test]
    fn bad_lines_are_skipped() {
        let text = "1\t0\t0\t200\t100\t100\ttop\nмусор\n2\t0\t0\t200\t100\t100\tsideways\n\
                    1\t5\t5\t200\t100\t100\tnormal\n3\t0\t0\t200\t100\t100\twindow\t0\t1\t2\t\t\n";
        let notes = from_text(text);
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].mode, Mode::Top);
        assert_eq!(notes[1].id, 3);
        assert_eq!(notes[1].attach, None);
        assert_eq!(next_id(&notes), 4);
    }

    #[test]
    fn titles_with_tabs_stay_in_one_field() {
        let mut n = note(
            1,
            Some(Attach {
                exe: "chrome.exe".into(),
                tab: "a\tb".into(),
                corner: 1,
                dx: 0,
                dy: 0,
            }),
        );
        let back = from_text(&to_text(std::slice::from_ref(&n)));
        n.attach.as_mut().unwrap().tab = "a b".into();
        assert_eq!(back, vec![n]);
    }

    #[test]
    fn first_line_skips_empty_lines() {
        assert_eq!(
            first_line("\r\n  \r\nКупить хлеб\r\nи молоко"),
            "Купить хлеб"
        );
        assert_eq!(first_line(""), "");
        assert_eq!(first_line(&"я".repeat(60)).chars().count(), 49);
    }
}
