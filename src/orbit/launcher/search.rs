use std::path::{Path, PathBuf};

use super::calc;
use crate::apps::{App, MAX_LENGTH, PREFIX_SCORE, score};
use crate::lang::{is_english, tr};

pub const MAX_ITEMS: usize = 7;
const FILE_ROWS: usize = 4;
const FILE_KEEP: usize = 3;
const RECENT_BONUS: i32 = 500;
const PLACE_CHARS: usize = 34;

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Launch(PathBuf),
    OpenFile(PathBuf),
    Command(Command),
    Copy(String),
    Open(String),
    Search(String),
    Complete(String),
    #[cfg(feature = "timers")]
    Remind(crate::reminders::Request),
    #[cfg(feature = "notes")]
    Note(String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    PlayPause,
    Next,
    Previous,
    Volume(u8),
    Circle,
    Oval,
    Rotation,
    Screenshot,
    Settings,
    Bluetooth,
    BluetoothDevices,
    Sleep,
    Lock,
    Shutdown,
    Restart,
    EmptyBin,
    NewNote,
    ShowNotes,
}

impl Command {
    pub fn needs_confirm(self) -> bool {
        matches!(
            self,
            Command::Shutdown | Command::Restart | Command::EmptyBin
        )
    }
}

#[derive(Clone, Copy, Default)]
pub struct Context {
    pub music: bool,
    pub bluetooth: Option<bool>,
    #[cfg_attr(not(feature = "timers"), allow(dead_code))]
    pub reminders_full: bool,
    pub notes: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub name: String,
    key: String,
    pub path: PathBuf,
    pub folder: bool,
    pub recent: bool,
    place: String,
}

impl Found {
    pub fn new(name: String, path: PathBuf, folder: bool, recent: bool) -> Self {
        let profile = std::env::var_os("USERPROFILE").map(PathBuf::from);
        Self {
            key: name.to_lowercase(),
            place: place(&path, profile.as_deref()),
            name,
            path,
            folder,
            recent,
        }
    }

    fn item(&self) -> Item {
        Item {
            glyph: if self.folder { '\u{E8B7}' } else { '\u{E8A5}' },
            title: self.name.clone(),
            hint: self.place.clone(),
            strong: false,
            confirm: false,
            action: Action::OpenFile(self.path.clone()),
        }
    }
}

fn place(path: &Path, profile: Option<&Path>) -> String {
    let parent = path.parent().unwrap_or(path);
    let shown = match profile.and_then(|p| parent.strip_prefix(p).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Some(rest) => rest.to_string_lossy().into_owned(),
        None => parent.to_string_lossy().into_owned(),
    };
    let count = shown.chars().count();
    if count <= PLACE_CHARS {
        return shown;
    }
    let tail: String = shown.chars().skip(count - (PLACE_CHARS - 1)).collect();
    format!("…{tail}")
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub glyph: char,
    pub title: String,
    pub hint: String,
    pub strong: bool,
    pub confirm: bool,
    pub action: Action,
}

#[derive(Clone, Copy, PartialEq)]
enum Need {
    Nothing,
    Music,
    Bluetooth,
    Notes,
}

struct CommandInfo {
    command: Command,
    glyph: char,
    words: &'static [&'static str],
    title: (&'static str, &'static str),
    need: Need,
}

const COMMANDS: [CommandInfo; 17] = [
    CommandInfo {
        command: Command::PlayPause,
        glyph: '\u{E768}',
        words: &["пауза", "играть", "плей", "стоп", "pause", "play", "stop"],
        title: ("Музыка: пауза / играть", "Music: play / pause"),
        need: Need::Music,
    },
    CommandInfo {
        command: Command::Next,
        glyph: '\u{E893}',
        words: &["дальше", "следующий", "вперёд", "вперед", "next", "skip"],
        title: ("Музыка: следующий трек", "Music: next track"),
        need: Need::Music,
    },
    CommandInfo {
        command: Command::Previous,
        glyph: '\u{E892}',
        words: &["назад", "предыдущий", "prev", "previous", "back"],
        title: ("Музыка: предыдущий трек", "Music: previous track"),
        need: Need::Music,
    },
    CommandInfo {
        command: Command::Circle,
        glyph: '\u{EA3A}',
        words: &["круг", "circle"],
        title: ("Орбита: круг", "Orbit: circle"),
        need: Need::Nothing,
    },
    CommandInfo {
        command: Command::Oval,
        glyph: '\u{EA3A}',
        words: &["овал", "oval"],
        title: ("Орбита: овал", "Orbit: oval"),
        need: Need::Nothing,
    },
    CommandInfo {
        command: Command::Rotation,
        glyph: '\u{E7AD}',
        words: &["вращение", "вращать", "rotation", "rotate", "spin"],
        title: ("Орбита: вращение вкл. / выкл.", "Orbit: rotation on / off"),
        need: Need::Nothing,
    },
    CommandInfo {
        command: Command::Screenshot,
        glyph: '\u{E722}',
        words: &["скриншот", "снимок", "screenshot", "snip"],
        title: ("Скриншот области", "Screenshot an area"),
        need: Need::Nothing,
    },
    CommandInfo {
        command: Command::Settings,
        glyph: '\u{E713}',
        words: &["настройки", "settings", "config"],
        title: ("Настройки Moonlet", "Moonlet settings"),
        need: Need::Nothing,
    },
    CommandInfo {
        command: Command::Bluetooth,
        glyph: '\u{E702}',
        words: &["блютуз", "блютус", "bluetooth", "bt"],
        title: ("Bluetooth", "Bluetooth"),
        need: Need::Bluetooth,
    },
    CommandInfo {
        command: Command::BluetoothDevices,
        glyph: '\u{E772}',
        words: &[
            "блютуз",
            "блютус",
            "bluetooth",
            "устройства",
            "сопряжение",
            "devices",
            "pair",
        ],
        title: ("Bluetooth: устройства", "Bluetooth: devices"),
        need: Need::Bluetooth,
    },
    CommandInfo {
        command: Command::Sleep,
        glyph: '\u{E708}',
        words: &["сон", "спать", "спящий режим", "sleep", "suspend"],
        title: ("Спящий режим", "Sleep"),
        need: Need::Nothing,
    },
    CommandInfo {
        command: Command::Lock,
        glyph: '\u{E72E}',
        words: &["заблокировать", "блокировка", "lock"],
        title: ("Заблокировать компьютер", "Lock the PC"),
        need: Need::Nothing,
    },
    CommandInfo {
        command: Command::Shutdown,
        glyph: '\u{E7E8}',
        words: &[
            "выключить",
            "выключение",
            "shutdown",
            "shut down",
            "power off",
        ],
        title: ("Выключить компьютер", "Shut down"),
        need: Need::Nothing,
    },
    CommandInfo {
        command: Command::Restart,
        glyph: '\u{E777}',
        words: &[
            "перезагрузка",
            "перезагрузить",
            "рестарт",
            "restart",
            "reboot",
        ],
        title: ("Перезагрузить компьютер", "Restart"),
        need: Need::Nothing,
    },
    CommandInfo {
        command: Command::EmptyBin,
        glyph: '\u{E74D}',
        words: &[
            "корзина",
            "очистить корзину",
            "recycle bin",
            "empty bin",
            "trash",
        ],
        title: ("Очистить корзину", "Empty the Recycle Bin"),
        need: Need::Nothing,
    },
    CommandInfo {
        command: Command::NewNote,
        glyph: '\u{E70B}',
        words: &["заметка", "новая заметка", "new note"],
        title: ("Новая заметка", "New note"),
        need: Need::Notes,
    },
    CommandInfo {
        command: Command::ShowNotes,
        glyph: '\u{E70B}',
        words: &["заметки", "показать заметки", "show notes", "all notes"],
        title: ("Показать заметки", "Show notes"),
        need: Need::Notes,
    },
];

pub fn confirm_title(command: Command) -> &'static str {
    match command {
        Command::Shutdown => tr(
            "Enter ещё раз — выключить компьютер",
            "Press Enter again to shut down",
        ),
        Command::Restart => tr(
            "Enter ещё раз — перезагрузить компьютер",
            "Press Enter again to restart",
        ),
        _ => tr(
            "Enter ещё раз — очистить корзину",
            "Press Enter again to empty the bin",
        ),
    }
}

const VOLUME_WORDS: [&str; 5] = ["громкость", "звук", "volume", "vol", "sound"];
const VOLUME_GLYPH: char = '\u{E767}';

const LAYOUT_PENALTY: i32 = 150;
const LATIN_PENALTY: i32 = 100;
const UNINSTALL_PENALTY: i32 = 300;
const UNINSTALL_WORDS: [&str; 4] = ["uninstall", "удалить", "удаление", "деинсталл"];

pub fn recent_items(recent: &[Found]) -> Vec<Item> {
    recent.iter().take(MAX_ITEMS).map(Found::item).collect()
}

pub fn suggestions(text: &str, apps: &[App], context: Context, files: &[Found]) -> Vec<Item> {
    let query = text.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let lower = query.to_lowercase();
    let mut variants = vec![(lower.clone(), 0)];
    if let Some(other) = switch_layout(&lower).filter(|o| *o != lower) {
        variants.push((other, LAYOUT_PENALTY));
    }
    for index in 0..variants.len() {
        if let Some(latin) = to_latin(&variants[index].0) {
            variants.push((latin, variants[index].1 + LATIN_PENALTY));
        }
    }

    let mut items = Vec::new();
    if let Some(value) = calc::evaluate(query) {
        let comma = if is_english() { '.' } else { ',' };
        let group = if is_english() { ',' } else { '\u{2009}' };
        items.push(Item {
            glyph: '\u{E8EF}',
            title: format!("= {}", calc::format(value, comma, Some(group))),
            hint: tr("Enter — копировать", "Enter to copy").to_string(),
            strong: true,
            confirm: false,
            action: Action::Copy(calc::format(value, comma, None)),
        });
    }

    #[cfg(feature = "timers")]
    if let Some(item) = reminder_item(query, context.reminders_full) {
        items.push(item);
    }
    #[cfg(feature = "notes")]
    if context.notes
        && let Some(item) = note_item(query)
    {
        items.push(item);
    }

    let mut scored: Vec<(i32, Item)> = Vec::new();
    for (query, penalty) in &variants {
        let (query, penalty) = (query.as_str(), *penalty);
        scored.extend(
            command_items(query, context)
                .into_iter()
                .map(|(score, item)| (score - penalty, item)),
        );
        for app in apps {
            let best =
                score(&app.key, query).max(app.file_key.as_deref().and_then(|f| score(f, query)));
            if let Some(points) = best {
                let uninstall = UNINSTALL_WORDS.iter().any(|w| app.key.starts_with(w));
                let extra = if uninstall { UNINSTALL_PENALTY } else { 0 };
                scored.push((
                    points - penalty - extra,
                    Item {
                        glyph: '\u{E71D}',
                        title: app.name.clone(),
                        hint: tr("Программа", "App").to_string(),
                        strong: false,
                        confirm: false,
                        action: Action::Launch(app.path.clone()),
                    },
                ));
            }
        }
    }
    let mut found: Vec<(i32, Item)> = Vec::new();
    for file in files {
        let Some(points) = score(&file.key, &lower) else {
            continue;
        };
        let item = file.item();
        if found.iter().any(|(_, i)| i.action == item.action) {
            continue;
        }
        let bonus = if file.recent { RECENT_BONUS } else { 0 };
        found.push((points + bonus, item));
    }
    let url = as_url(query);
    let reserved = 1 + usize::from(url.is_some());
    let slots = MAX_ITEMS.saturating_sub(items.len() + reserved);
    let app_limit = items.len() + slots - found.len().min(FILE_ROWS).min(FILE_KEEP).min(slots);
    take_best(&mut items, &mut scored, app_limit);
    let file_limit = (items.len() + FILE_ROWS).min(MAX_ITEMS - reserved);
    take_best(&mut items, &mut found, file_limit);
    if let Some(url) = url {
        items.push(Item {
            glyph: '\u{E774}',
            title: query.to_string(),
            hint: tr("Открыть сайт", "Open site").to_string(),
            strong: false,
            confirm: false,
            action: Action::Open(url),
        });
    }
    let (open, close) = if is_english() {
        ("\u{201C}", "\u{201D}")
    } else {
        ("«", "»")
    };
    items.push(Item {
        glyph: '\u{E721}',
        title: format!(
            "{} {open}{query}{close}",
            tr("Искать в интернете:", "Search the web:")
        ),
        hint: tr("Интернет", "Web").to_string(),
        strong: false,
        confirm: false,
        action: Action::Search(query.to_string()),
    });
    items
}

fn take_best(items: &mut Vec<Item>, scored: &mut Vec<(i32, Item)>, limit: usize) {
    while items.len() < limit {
        let Some(best) =
            (0..scored.len()).reduce(|a, b| if better(&scored[b], &scored[a]) { b } else { a })
        else {
            break;
        };
        let (_, item) = scored.swap_remove(best);
        if !items.iter().any(|i: &Item| i.action == item.action) {
            items.push(item);
        }
    }
}

fn better((a, x): &(i32, Item), (b, y): &(i32, Item)) -> bool {
    let (x_len, y_len) = (x.title.chars().count(), y.title.chars().count());
    (a, y_len, &y.title) > (b, x_len, &x.title)
}

fn command_items(query: &str, context: Context) -> Vec<(i32, Item)> {
    let mut found = Vec::new();
    if query.chars().count() < 2 {
        return found;
    }
    let music = context.music;
    let available = |need: Need| match need {
        Need::Nothing => true,
        Need::Music => music,
        Need::Bluetooth => context.bluetooth.is_some(),
        Need::Notes => context.notes,
    };
    for info in COMMANDS.iter().filter(|info| available(info.need)) {
        let best = info.words.iter().filter_map(|w| score(w, query)).max();
        if let Some(points) = best.filter(|&p| p >= PREFIX_SCORE - MAX_LENGTH) {
            let title = match (info.command, context.bluetooth) {
                (Command::Bluetooth, Some(true)) => {
                    tr("Bluetooth: выключить", "Bluetooth: turn off")
                }
                (Command::Bluetooth, _) => tr("Bluetooth: включить", "Bluetooth: turn on"),
                _ => tr(info.title.0, info.title.1),
            };
            found.push((
                points,
                Item {
                    glyph: info.glyph,
                    title: title.to_string(),
                    hint: "Moonlet".to_string(),
                    strong: false,
                    confirm: info.command.needs_confirm(),
                    action: Action::Command(info.command),
                },
            ));
        }
    }
    if music && let Some(item) = volume_item(query) {
        found.push(item);
    }
    found
}

#[cfg(feature = "notes")]
fn note_item(query: &str) -> Option<Item> {
    let (first, rest) = query.trim().split_once(char::is_whitespace)?;
    let rest = rest.trim();
    if rest.is_empty() || !matches!(first.to_lowercase().as_str(), "заметка" | "note") {
        return None;
    }
    let (open, close) = if is_english() {
        ("\u{201C}", "\u{201D}")
    } else {
        ("«", "»")
    };
    let title = format!("{} {open}{rest}{close}", tr("Новая заметка:", "New note:"));
    Some(Item {
        glyph: '\u{E70B}',
        title,
        hint: tr("Enter — создать", "Enter to create").to_string(),
        strong: true,
        confirm: false,
        action: Action::Note(rest.to_string()),
    })
}

#[cfg(feature = "timers")]
fn reminder_item(query: &str, full: bool) -> Option<Item> {
    use crate::reminders::{self, is_remind_word, is_timer_word};
    let lower = query.to_lowercase();
    let first = lower.split_whitespace().next()?;
    let single = !lower.trim().contains(' ');
    let starts = |word: &str| first.chars().count() >= 3 && word.starts_with(first);
    let timer = is_timer_word(first) || (single && (starts("таймер") || starts("timer")));
    let remind = is_remind_word(first) || (single && (starts("напомни") || starts("remind")));
    if !timer && !remind {
        return None;
    }
    let item = |glyph, title: String, hint: &str, action| Item {
        glyph,
        title,
        hint: hint.to_string(),
        strong: false,
        confirm: false,
        action,
    };
    let glyph = if timer { '\u{E916}' } else { '\u{E823}' };
    if let Some(request) = reminders::parse(query)
        && request.resolve(reminders::now()).is_some()
    {
        if full {
            return Some(item(
                glyph,
                tr(
                    "Уже 20 напоминаний — отмените лишние в настройках",
                    "Already 20 reminders, cancel some in the settings",
                )
                .to_string(),
                "Moonlet",
                Action::Complete(query.to_string()),
            ));
        }
        return Some(Item {
            strong: true,
            ..item(
                glyph,
                request.title(),
                tr("Enter — запустить", "Enter to start"),
                Action::Remind(request),
            )
        });
    }
    let (title, word) = if timer {
        (
            tr(
                "Таймер: «таймер 10 мин» или «таймер до 11:10»",
                "Timer: \"timer 10 min\" or \"timer until 11:10\"",
            ),
            tr("таймер ", "timer "),
        )
    } else {
        (
            tr(
                "Напомнить: «через 20 мин …», «в 15:30 …», «завтра в 9 …»",
                "Remind: \"in 20 min …\", \"at 15:30 …\", \"tomorrow at 9 …\"",
            ),
            tr("напомни ", "remind me "),
        )
    };
    let text = if single {
        word.to_string()
    } else {
        query.to_string()
    };
    Some(item(
        glyph,
        title.to_string(),
        "Moonlet",
        Action::Complete(text),
    ))
}

fn volume_item(query: &str) -> Option<(i32, Item)> {
    let mut parts = query.split_whitespace();
    let word = parts.next()?;
    let number = parts.next();
    if parts.next().is_some() {
        return None;
    }
    let points = VOLUME_WORDS.iter().filter_map(|w| score(w, word)).max()?;
    if points < PREFIX_SCORE - MAX_LENGTH {
        return None;
    }
    let item = match number {
        Some(number) => {
            let level: u8 = number
                .trim_end_matches('%')
                .parse()
                .ok()
                .filter(|&n| n <= 100)?;
            Item {
                glyph: VOLUME_GLYPH,
                title: format!("{} {level}%", tr("Громкость:", "Volume:")),
                hint: "Moonlet".to_string(),
                strong: false,
                confirm: false,
                action: Action::Command(Command::Volume(level)),
            }
        }
        None => Item {
            glyph: VOLUME_GLYPH,
            title: tr(
                "Громкость: введите число 0–100",
                "Volume: type a number 0–100",
            )
            .to_string(),
            hint: "Moonlet".to_string(),
            strong: false,
            confirm: false,
            action: Action::Complete(format!("{} ", tr("громкость", "volume"))),
        },
    };
    Some((
        if number.is_some() {
            PREFIX_SCORE
        } else {
            points
        },
        item,
    ))
}

const EN: &str = "qwertyuiop[]asdfghjkl;'zxcvbnm,.`";
const RU: &str = "йцукенгшщзхъфывапролджэячсмитьбюё";

pub fn switch_layout(text: &str) -> Option<String> {
    let first = text.chars().find(|c| c.is_alphabetic())?;
    let (from, to) = if first.is_ascii() { (EN, RU) } else { (RU, EN) };
    let mut out = String::with_capacity(text.len() * 2);
    let mut changed = false;
    for c in text.chars() {
        match from.chars().position(|f| f == c) {
            Some(index) => {
                out.push(to.chars().nth(index)?);
                changed = true;
            }
            None => out.push(c),
        }
    }
    changed.then_some(out)
}

pub fn to_latin(text: &str) -> Option<String> {
    const LATIN: [&str; 33] = [
        "a", "b", "v", "g", "d", "e", "zh", "z", "i", "y", "k", "l", "m", "n", "o", "p", "r", "s",
        "t", "u", "f", "h", "c", "ch", "sh", "sch", "", "y", "", "e", "yu", "ya", "e",
    ];
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    for c in text.chars() {
        let index = match c {
            'а'..='я' => Some(c as usize - 'а' as usize),
            'ё' => Some(32),
            _ => None,
        };
        match index {
            Some(index) => {
                out.push_str(LATIN[index]);
                changed = true;
            }
            None => out.push(c),
        }
    }
    changed.then_some(out)
}

pub fn as_url(text: &str) -> Option<String> {
    if text.chars().any(char::is_whitespace) {
        return None;
    }
    let lower = text.to_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Some(text.to_string());
    }
    let host = text.split(['/', '?', '#']).next()?;
    let host = host.split(':').next()?;
    let (name, zone) = host.rsplit_once('.')?;
    let zone_ok = zone.chars().count() >= 2 && zone.chars().all(char::is_alphabetic);
    let name_ok = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '.');
    (zone_ok && name_ok).then(|| format!("https://{text}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, file: &str) -> App {
        let key = name.to_lowercase();
        App {
            name: name.to_string(),
            file_key: Some(file.to_lowercase()).filter(|f| *f != key),
            key,
            path: PathBuf::from(format!("{file}.lnk")),
        }
    }

    const MUSIC: Context = Context {
        music: true,
        bluetooth: Some(false),
        reminders_full: false,
        notes: true,
    };

    fn titles(text: &str, apps: &[App]) -> Vec<String> {
        suggestions(text, apps, MUSIC, &[])
            .into_iter()
            .map(|item| item.title)
            .collect()
    }

    fn file(name: &str, recent: bool) -> Found {
        Found::new(
            name.to_string(),
            PathBuf::from(format!("C:\\Docs\\{name}")),
            false,
            recent,
        )
    }

    #[test]
    fn files_go_under_apps() {
        let apps = [app("Отчёты", "Reports"), app("Отчёт 2", "Report2")];
        let files = [
            file("отчёт.docx", false),
            file("отчёт за май.xlsx", true),
            file("план.txt", true),
            file("отчёт.docx", false),
        ];
        let items = suggestions("отчёт", &apps, MUSIC, &files);
        let titles: Vec<&str> = items.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(&titles[..2], ["Отчёты", "Отчёт 2"]);
        assert_eq!(&titles[2..4], ["отчёт за май.xlsx", "отчёт.docx"]);
        assert_eq!(items.len(), 5);
        assert_eq!(items[2].hint, "C:\\Docs");
        let many: Vec<App> = (0..9)
            .map(|i| app(&format!("отчёт {i}"), &format!("r{i}")))
            .collect();
        let items = suggestions("отчёт", &many, MUSIC, &files);
        assert_eq!(items.len(), MAX_ITEMS);
        let files_shown = items
            .iter()
            .filter(|i| matches!(i.action, Action::OpenFile(_)))
            .count();
        assert_eq!(files_shown, 2);
        let recent = recent_items(&files);
        assert_eq!(recent.len(), 4);
    }

    #[test]
    fn shortens_places() {
        let profile = Path::new("C:\\Users\\Me");
        let at = |p: &str| place(Path::new(p), Some(profile));
        assert_eq!(at("C:\\Users\\Me\\Documents\\a.txt"), "Documents");
        assert_eq!(at("C:\\Users\\Me\\a.txt"), "~");
        assert_eq!(at("D:\\Work\\a.txt"), "D:\\Work");
        let long = at("C:\\Users\\Me\\Documents\\Очень длинная папка\\ещё одна папка\\x.txt");
        assert_eq!(long.chars().count(), PLACE_CHARS);
        assert!(long.starts_with('…') && long.ends_with("ещё одна папка"));
    }

    #[test]
    fn system_commands_need_confirmation() {
        let items = suggestions("выкл", &[], MUSIC, &[]);
        assert_eq!(items[0].action, Action::Command(Command::Shutdown));
        assert!(items[0].confirm);
        let items = suggestions("сон", &[], MUSIC, &[]);
        assert_eq!(items[0].action, Action::Command(Command::Sleep));
        assert!(!items[0].confirm);
        let items = suggestions("блютуз", &[], MUSIC, &[]);
        assert_eq!(items[0].title, "Bluetooth: включить");
        let none = Context {
            bluetooth: None,
            ..MUSIC
        };
        assert_eq!(suggestions("блютуз", &[], none, &[]).len(), 1);
    }

    #[test]
    fn ranks_names() {
        assert!(score("telegram", "tel") > score("hotel manager", "tel"));
        assert!(score("visual studio code", "vsc").is_some());
        assert!(score("visual studio code", "stu") > score("visual studio code", "vsc"));
        assert!(score("telegram", "tgm").is_some());
        assert_eq!(score("telegram", "x"), None);
        assert!(score("paint", "pa") > score("paint 3d editor", "pa"));
    }

    #[test]
    fn switches_keyboard_layout() {
        assert_eq!(switch_layout("ntktuhfv").as_deref(), Some("телеграм"));
        assert_eq!(switch_layout("еудупкфь").as_deref(), Some("telegram"));
        assert_eq!(switch_layout("123"), None);
        assert_eq!(to_latin("телеграм").as_deref(), Some("telegram"));
        assert_eq!(to_latin("ёжик щи").as_deref(), Some("ezhik schi"));
        assert_eq!(to_latin("code"), None);
    }

    #[test]
    fn finds_apps_through_the_wrong_layout() {
        let apps = [
            app("Телеграм", "Telegram"),
            app("Блокнот", "Notepad"),
            app("Деинсталлировать Spotify", "Uninstall Spotify"),
            app("Spotify", "Spotify"),
        ];
        assert_eq!(titles("ntktuhfv", &apps)[0], "Телеграм");
        let latin = [app("Telegram", "Telegram")];
        assert_eq!(titles("ntktuhfv", &latin)[0], "Telegram");
        assert_eq!(titles("телег", &latin)[0], "Telegram");
        assert_eq!(titles("spot", &apps)[0], "Spotify");
        assert_eq!(titles("блок", &apps)[0], "Блокнот");
        assert_eq!(titles("note", &apps)[0], "Блокнот");
        let last = suggestions("note", &apps, MUSIC, &[]).pop().unwrap();
        assert_eq!(last.action, Action::Search("note".into()));
    }

    #[test]
    fn shows_calculator_and_commands() {
        let items = suggestions("2+2*2", &[], MUSIC, &[]);
        assert_eq!(items[0].title, "= 6");
        assert_eq!(items[0].action, Action::Copy("6".into()));
        let items = suggestions("громкость 30", &[], MUSIC, &[]);
        assert_eq!(items[0].action, Action::Command(Command::Volume(30)));
        let items = suggestions("gfepf", &[], MUSIC, &[]);
        assert_eq!(items[0].action, Action::Command(Command::PlayPause));
        let items = suggestions("пауза", &[], Context::default(), &[]);
        assert_eq!(items.len(), 1);
        assert!(suggestions("   ", &[], MUSIC, &[]).is_empty());
    }

    #[cfg(feature = "notes")]
    #[test]
    fn offers_new_notes() {
        let first = |text: &str, context| suggestions(text, &[], context, &[]).remove(0);
        let item = first("заметка купить хлеб", MUSIC);
        assert_eq!(item.action, Action::Note("купить хлеб".into()));
        let item = first("note buy milk", MUSIC);
        assert_eq!(item.action, Action::Note("buy milk".into()));
        assert_eq!(
            first("заметка", MUSIC).action,
            Action::Command(Command::NewNote)
        );
        let titles: Vec<String> = suggestions("зам", &[], MUSIC, &[])
            .into_iter()
            .map(|i| i.title)
            .collect();
        assert!(titles.contains(&"Новая заметка".to_string()));
        assert_eq!(
            first("заметки", MUSIC).action,
            Action::Command(Command::ShowNotes)
        );
        let off = Context {
            notes: false,
            ..MUSIC
        };
        assert!(matches!(first("заметка", off).action, Action::Search(_)));
        assert!(matches!(first("заметки", off).action, Action::Search(_)));
    }

    #[cfg(feature = "timers")]
    #[test]
    fn offers_timers_and_reminders() {
        crate::lang::set(crate::lang::Language::Russian);
        let first = |text: &str| suggestions(text, &[], MUSIC, &[]).remove(0);
        let item = first("таймер 10 мин");
        assert!(matches!(item.action, Action::Remind(_)));
        assert_eq!(item.title, "Таймер на 10 мин");
        assert_eq!(
            first("напомни через 20 мин позвонить").title,
            "Напомнить через 20 мин: позвонить"
        );
        assert_eq!(first("тайм").action, Action::Complete("таймер ".into()));
        assert!(matches!(
            first("напомни позвонить").action,
            Action::Complete(_)
        ));
    }

    #[test]
    fn recognizes_urls() {
        assert_eq!(as_url("github.com").as_deref(), Some("https://github.com"));
        assert_eq!(
            as_url("ya.ru/maps?x=1").as_deref(),
            Some("https://ya.ru/maps?x=1")
        );
        assert_eq!(as_url("http://a.b").as_deref(), Some("http://a.b"));
        assert_eq!(as_url("1.5"), None);
        assert_eq!(as_url("hello world.com"), None);
        assert_eq!(as_url("telegram"), None);
    }
}
