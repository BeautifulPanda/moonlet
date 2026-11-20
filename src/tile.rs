//   tiles = ["screenshot", "app:C:\\...\\Telegram.lnk", "url:https://github.com|GitHub"]

use crate::lang::tr;

pub const MAX_TILES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Builtin {
    Screenshot,
    Rotation,
    Shape,
    Settings,
    Launcher,
    PlayPause,
    Next,
    Previous,
}

impl Builtin {
    pub const ALL: [(Builtin, &'static str); 8] = [
        (Builtin::Screenshot, "screenshot"),
        (Builtin::Launcher, "launcher"),
        (Builtin::PlayPause, "play"),
        (Builtin::Next, "next"),
        (Builtin::Previous, "previous"),
        (Builtin::Rotation, "rotation"),
        (Builtin::Shape, "shape"),
        (Builtin::Settings, "settings"),
    ];

    pub fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(b, _)| *b == self)
            .map_or("", |(_, name)| name)
    }

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().find(|(_, n)| *n == name).map(|(b, _)| *b)
    }

    #[cfg_attr(not(feature = "settings"), allow(dead_code))]
    pub fn title(self) -> &'static str {
        match self {
            Builtin::Screenshot => tr("Скриншот области", "Screenshot an area"),
            Builtin::Rotation => tr("Вращение вкл. / выкл.", "Rotation on / off"),
            Builtin::Shape => tr("Форма орбиты", "Orbit shape"),
            Builtin::Settings => tr("Настройки", "Settings"),
            Builtin::Launcher => tr("Командная строка", "Command line"),
            Builtin::PlayPause => tr("Музыка: пауза / играть", "Music: play / pause"),
            Builtin::Next => tr("Следующий трек", "Next track"),
            Builtin::Previous => tr("Предыдущий трек", "Previous track"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    App,
    File,
    Folder,
    Url,
    Command,
}

impl Kind {
    pub const ALL: [(Kind, &'static str); 5] = [
        (Kind::App, "app"),
        (Kind::File, "file"),
        (Kind::Folder, "folder"),
        (Kind::Url, "url"),
        (Kind::Command, "cmd"),
    ];

    pub fn prefix(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(k, _)| *k == self)
            .map_or("", |(_, p)| p)
    }

    pub fn glyph(self) -> Option<char> {
        match self {
            Kind::Folder => Some('\u{E8B7}'),
            Kind::Url => Some('\u{E774}'),
            Kind::Command => Some('\u{E756}'),
            Kind::App | Kind::File => None,
        }
    }

    #[cfg_attr(not(feature = "settings"), allow(dead_code))]
    pub fn title(self) -> &'static str {
        match self {
            Kind::App => tr("Программа", "App"),
            Kind::File => tr("Файл", "File"),
            Kind::Folder => tr("Папка", "Folder"),
            Kind::Url => tr("Сайт", "Website"),
            Kind::Command => tr("Команда", "Command"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tile {
    Builtin(Builtin),
    Custom {
        kind: Kind,
        target: String,
        name: String,
    },
}

impl Tile {
    pub fn custom(kind: Kind, target: &str) -> Self {
        let target = target.trim().to_string();
        Tile::Custom {
            kind,
            name: default_name(kind, &target),
            target,
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if let Some(builtin) = Builtin::from_name(text) {
            return Some(Tile::Builtin(builtin));
        }
        let (prefix, rest) = text.split_once(':')?;
        let kind = Kind::ALL
            .iter()
            .find(|(_, p)| *p == prefix.trim())
            .map(|(k, _)| *k)?;
        let (target, name) = match rest.rsplit_once('|') {
            Some((target, name)) => (target.trim(), name.trim()),
            None => (rest.trim(), ""),
        };
        if target.is_empty() {
            return None;
        }
        let mut tile = Tile::custom(kind, target);
        if let Tile::Custom { name: n, .. } = &mut tile
            && !name.is_empty()
        {
            *n = name.to_string();
        }
        Some(tile)
    }

    pub fn to_text(&self) -> String {
        match self {
            Tile::Builtin(builtin) => builtin.name().to_string(),
            Tile::Custom { kind, target, name } => {
                let mut text = format!("{}:{target}", kind.prefix());
                if *name != default_name(*kind, target) {
                    text.push('|');
                    text.push_str(name);
                }
                text
            }
        }
    }

    #[cfg_attr(not(feature = "settings"), allow(dead_code))]
    pub fn title(&self) -> String {
        match self {
            Tile::Builtin(builtin) => builtin.title().to_string(),
            Tile::Custom { name, .. } => name.clone(),
        }
    }
}

pub fn default_name(kind: Kind, target: &str) -> String {
    let target = target.trim();
    match kind {
        Kind::App | Kind::File | Kind::Folder => {
            let trimmed = target.trim_end_matches(['\\', '/']);
            let file = trimmed.rsplit(['\\', '/']).next().unwrap_or(trimmed);
            let stem = match (kind, file.rsplit_once('.')) {
                (Kind::App, Some((stem, _))) if !stem.is_empty() => stem,
                _ => file,
            };
            if stem.is_empty() {
                target.to_string()
            } else {
                stem.to_string()
            }
        }
        Kind::Url => {
            let rest = target.split_once("://").map_or(target, |(_, rest)| rest);
            let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
            let host = host.strip_prefix("www.").unwrap_or(host);
            if host.is_empty() {
                target.to_string()
            } else {
                host.to_string()
            }
        }
        Kind::Command => target.to_string(),
    }
}

pub fn letter(name: &str) -> char {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .map_or('?', |c| c.to_uppercase().next().unwrap_or(c))
}

pub fn defaults() -> Vec<Tile> {
    [
        Builtin::Screenshot,
        Builtin::Rotation,
        Builtin::Shape,
        Builtin::Settings,
    ]
    .into_iter()
    .map(Tile::Builtin)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_writes_back() {
        for text in [
            "screenshot",
            "app:C:\\Apps\\Telegram.lnk",
            "file:D:\\Docs\\plan.pdf",
            "folder:C:\\Users\\Me\\Downloads\\",
            "url:https://github.com/x|GitHub",
            "cmd:громкость 30",
        ] {
            let tile = Tile::parse(text).unwrap();
            assert_eq!(Tile::parse(&tile.to_text()).unwrap(), tile);
        }
        assert_eq!(Tile::parse("radio"), None);
        assert_eq!(Tile::parse("app:"), None);
        assert_eq!(Tile::parse("url: |Name"), None);
    }

    #[test]
    fn names_come_from_the_target() {
        let name = |text: &str| Tile::parse(text).unwrap().title();
        assert_eq!(name("app:C:\\Apps\\Telegram.lnk"), "Telegram");
        assert_eq!(name("file:D:\\Docs\\plan.pdf"), "plan.pdf");
        assert_eq!(name("folder:C:\\Users\\Me\\Downloads\\"), "Downloads");
        assert_eq!(name("folder:D:\\"), "D:");
        assert_eq!(name("url:https://www.youtube.com/watch?v=1"), "youtube.com");
        assert_eq!(name("url:https://github.com|Код"), "Код");
        assert_eq!(name("cmd:громкость 30"), "громкость 30");
        assert_eq!(
            Tile::parse("url:https://github.com|github.com")
                .unwrap()
                .to_text(),
            "url:https://github.com"
        );
    }

    #[test]
    fn letters_skip_punctuation() {
        assert_eq!(letter("telegram"), 'T');
        assert_eq!(letter("«блокнот»"), 'Б');
    }
}
