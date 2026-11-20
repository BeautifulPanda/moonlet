use std::path::PathBuf;

use super::calc;
use crate::apps::{App, MAX_LENGTH, PREFIX_SCORE, score};
use crate::lang::{is_english, tr};

pub const MAX_ITEMS: usize = 7;

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Launch(PathBuf),
    Command(Command),
    Copy(String),
    Open(String),
    Search(String),
    Complete(String),
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub glyph: char,
    pub title: String,
    pub hint: &'static str,
    pub strong: bool,
    pub action: Action,
}

struct CommandInfo {
    command: Command,
    glyph: char,
    words: &'static [&'static str],
    title: (&'static str, &'static str),
    music: bool,
}

const COMMANDS: [CommandInfo; 8] = [
    CommandInfo {
        command: Command::PlayPause,
        glyph: '\u{E768}',
        words: &["пауза", "играть", "плей", "стоп", "pause", "play", "stop"],
        title: ("Музыка: пауза / играть", "Music: play / pause"),
        music: true,
    },
    CommandInfo {
        command: Command::Next,
        glyph: '\u{E893}',
        words: &["дальше", "следующий", "вперёд", "вперед", "next", "skip"],
        title: ("Музыка: следующий трек", "Music: next track"),
        music: true,
    },
    CommandInfo {
        command: Command::Previous,
        glyph: '\u{E892}',
        words: &["назад", "предыдущий", "prev", "previous", "back"],
        title: ("Музыка: предыдущий трек", "Music: previous track"),
        music: true,
    },
    CommandInfo {
        command: Command::Circle,
        glyph: '\u{EA3A}',
        words: &["круг", "circle"],
        title: ("Орбита: круг", "Orbit: circle"),
        music: false,
    },
    CommandInfo {
        command: Command::Oval,
        glyph: '\u{EA3A}',
        words: &["овал", "oval"],
        title: ("Орбита: овал", "Orbit: oval"),
        music: false,
    },
    CommandInfo {
        command: Command::Rotation,
        glyph: '\u{E7AD}',
        words: &["вращение", "вращать", "rotation", "rotate", "spin"],
        title: ("Орбита: вращение вкл. / выкл.", "Orbit: rotation on / off"),
        music: false,
    },
    CommandInfo {
        command: Command::Screenshot,
        glyph: '\u{E722}',
        words: &["скриншот", "снимок", "screenshot", "snip"],
        title: ("Скриншот области", "Screenshot an area"),
        music: false,
    },
    CommandInfo {
        command: Command::Settings,
        glyph: '\u{E713}',
        words: &["настройки", "settings", "config"],
        title: ("Настройки Moonlet", "Moonlet settings"),
        music: false,
    },
];

const VOLUME_WORDS: [&str; 5] = ["громкость", "звук", "volume", "vol", "sound"];
const VOLUME_GLYPH: char = '\u{E767}';

const LAYOUT_PENALTY: i32 = 150;
const LATIN_PENALTY: i32 = 100;
const UNINSTALL_PENALTY: i32 = 300;
const UNINSTALL_WORDS: [&str; 4] = ["uninstall", "удалить", "удаление", "деинсталл"];

pub fn suggestions(text: &str, apps: &[App], music: bool) -> Vec<Item> {
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
            hint: tr("Enter — копировать", "Enter to copy"),
            strong: true,
            action: Action::Copy(calc::format(value, comma, None)),
        });
    }

    let mut scored: Vec<(i32, Item)> = Vec::new();
    for (query, penalty) in &variants {
        let (query, penalty) = (query.as_str(), *penalty);
        scored.extend(
            command_items(query, music)
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
                        hint: tr("Программа", "App"),
                        strong: false,
                        action: Action::Launch(app.path.clone()),
                    },
                ));
            }
        }
    }
    let url = as_url(query);
    let reserved = 1 + usize::from(url.is_some());
    while items.len() + reserved < MAX_ITEMS {
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
    if let Some(url) = url {
        items.push(Item {
            glyph: '\u{E774}',
            title: query.to_string(),
            hint: tr("Открыть сайт", "Open site"),
            strong: false,
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
        hint: tr("Интернет", "Web"),
        strong: false,
        action: Action::Search(query.to_string()),
    });
    items
}

fn better((a, x): &(i32, Item), (b, y): &(i32, Item)) -> bool {
    let (x_len, y_len) = (x.title.chars().count(), y.title.chars().count());
    (a, y_len, &y.title) > (b, x_len, &x.title)
}

fn command_items(query: &str, music: bool) -> Vec<(i32, Item)> {
    let mut found = Vec::new();
    if query.chars().count() < 2 {
        return found;
    }
    for info in COMMANDS.iter().filter(|info| music || !info.music) {
        let best = info.words.iter().filter_map(|w| score(w, query)).max();
        if let Some(points) = best.filter(|&p| p >= PREFIX_SCORE - MAX_LENGTH) {
            found.push((
                points,
                Item {
                    glyph: info.glyph,
                    title: tr(info.title.0, info.title.1).to_string(),
                    hint: "Moonlet",
                    strong: false,
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
                hint: "Moonlet",
                strong: false,
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
            hint: "Moonlet",
            strong: false,
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

    fn titles(text: &str, apps: &[App]) -> Vec<String> {
        suggestions(text, apps, true)
            .into_iter()
            .map(|item| item.title)
            .collect()
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
        let last = suggestions("note", &apps, true).pop().unwrap();
        assert_eq!(last.action, Action::Search("note".into()));
    }

    #[test]
    fn shows_calculator_and_commands() {
        let items = suggestions("2+2*2", &[], true);
        assert_eq!(items[0].title, "= 6");
        assert_eq!(items[0].action, Action::Copy("6".into()));
        let items = suggestions("громкость 30", &[], true);
        assert_eq!(items[0].action, Action::Command(Command::Volume(30)));
        let items = suggestions("gfepf", &[], true);
        assert_eq!(items[0].action, Action::Command(Command::PlayPause));
        let items = suggestions("пауза", &[], false);
        assert_eq!(items.len(), 1);
        assert!(suggestions("   ", &[], true).is_empty());
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
