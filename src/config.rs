use std::fmt;
use std::path::PathBuf;

use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR, w};

#[cfg(feature = "launcher")]
use crate::hotkey::Hotkey;
use crate::lang::{Language, tr};
#[cfg(feature = "orbit")]
use crate::tile::{MAX_TILES, Tile};
use crate::toml::{self, ParseError, Table};

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub autostart: bool,
    pub language: Language,
    #[cfg(feature = "update")]
    pub update: crate::update::Mode,
    #[cfg(feature = "orbit")]
    pub pause_on_battery: bool,
    #[cfg(feature = "orbit")]
    pub orbit_enabled: bool,
    #[cfg(feature = "orbit")]
    pub orbit: OrbitConfig,
    #[cfg(feature = "launcher")]
    pub launcher_enabled: bool,
    #[cfg(feature = "launcher")]
    pub launcher: LauncherConfig,
    #[cfg(feature = "timers")]
    pub timer_sound: String,
    #[cfg(feature = "notes")]
    pub notes_enabled: bool,
    #[cfg(feature = "notes")]
    pub notes_mode: crate::notes::store::Mode,
    #[cfg(feature = "scenarios")]
    pub scenarios_enabled: bool,
}

#[cfg(feature = "launcher")]
#[derive(Clone, Debug, PartialEq)]
pub struct LauncherConfig {
    pub hotkey: Hotkey,
    pub search: String,
}

#[cfg(feature = "launcher")]
impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            hotkey: Hotkey::DEFAULT,
            search: String::new(),
        }
    }
}

#[cfg_attr(not(feature = "orbit"), allow(clippy::derivable_impls))]
impl Default for Config {
    fn default() -> Self {
        Self {
            autostart: false,
            language: Language::Auto,
            #[cfg(feature = "update")]
            update: crate::update::Mode::Auto,
            #[cfg(feature = "orbit")]
            pause_on_battery: true,
            #[cfg(feature = "orbit")]
            orbit_enabled: true,
            #[cfg(feature = "orbit")]
            orbit: OrbitConfig::default(),
            #[cfg(feature = "launcher")]
            launcher_enabled: true,
            #[cfg(feature = "launcher")]
            launcher: LauncherConfig::default(),
            #[cfg(feature = "timers")]
            timer_sound: crate::sound::DROP.to_string(),
            #[cfg(feature = "notes")]
            notes_enabled: true,
            #[cfg(feature = "notes")]
            notes_mode: crate::notes::store::Mode::Top,
            #[cfg(feature = "scenarios")]
            scenarios_enabled: true,
        }
    }
}

#[cfg(feature = "orbit")]
#[derive(Clone, Debug, PartialEq)]
pub struct OrbitConfig {
    pub radius_x: f32,
    pub radius_y: f32,
    pub period_secs: f32,
    pub clockwise: bool,
    pub rotate: bool,
    pub core_radius: f32,
    pub satellite_radius: f32,
    pub layout_radius: f32,
    pub hover_delay_ms: u32,
    pub hover_padding: f32,
    pub satellites: Vec<SatelliteKind>,
    pub notices: NoticeConfig,
    pub tiles: Vec<Tile>,
}

#[cfg(feature = "orbit")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoticeConfig {
    pub charger: bool,
    pub low_battery: bool,
    pub track: bool,
    pub seconds: f32,
}

#[cfg(feature = "orbit")]
impl Default for NoticeConfig {
    fn default() -> Self {
        Self {
            charger: true,
            low_battery: true,
            track: false,
            seconds: 4.0,
        }
    }
}

#[cfg(feature = "orbit")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SatelliteKind {
    Clock,
    Music,
    Battery,
}

#[cfg(feature = "orbit")]
impl SatelliteKind {
    const ALL: [(SatelliteKind, &'static str); 3] = [
        (SatelliteKind::Clock, "clock"),
        (SatelliteKind::Music, "music"),
        (SatelliteKind::Battery, "battery"),
    ];

    pub fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(kind, _)| *kind == self)
            .map_or("", |(_, name)| name)
    }

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .find(|(_, n)| *n == name)
            .map(|(kind, _)| *kind)
    }
}

#[cfg(feature = "orbit")]
impl OrbitConfig {
    pub const CIRCLE: (f32, f32) = (30.0, 30.0);
    pub const OVAL: (f32, f32) = (44.0, 19.0);

    pub fn needs_rebuild(&self, other: &Self) -> bool {
        let mut same = other.clone();
        same.radius_x = self.radius_x;
        same.radius_y = self.radius_y;
        same.rotate = self.rotate;
        same.notices = self.notices;
        same.tiles = self.tiles.clone();
        *self != same
    }
}

#[cfg(feature = "orbit")]
impl Default for OrbitConfig {
    fn default() -> Self {
        let (radius_x, radius_y) = Self::OVAL;
        Self {
            radius_x,
            radius_y,
            period_secs: 40.0,
            clockwise: true,
            rotate: true,
            core_radius: 12.0,
            satellite_radius: 5.0,
            layout_radius: 46.0,
            hover_delay_ms: 120,
            hover_padding: 10.0,
            satellites: vec![
                SatelliteKind::Clock,
                SatelliteKind::Music,
                SatelliteKind::Battery,
            ],
            notices: NoticeConfig::default(),
            tiles: crate::tile::defaults(),
        }
    }
}

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    Parse(ParseError),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LoadError::Io(error) => write!(f, "{error}"),
            LoadError::Parse(error) => write!(f, "{error}"),
        }
    }
}

pub fn open_in_notepad() {
    let Some(path) = path() else {
        return;
    };
    let argument = HSTRING::from(format!("\"{}\"", path.display()));
    unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            w!("notepad.exe"),
            &argument,
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

pub fn path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(appdata).join("Moonlet").join("config.toml"))
}

impl Config {
    pub fn load() -> Result<Self, LoadError> {
        let Some(path) = path() else {
            return Ok(Self::default());
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::from_toml(&text).map_err(LoadError::Parse),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let config = Self::default();
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir).map_err(LoadError::Io)?;
                }
                std::fs::write(&path, config.to_toml()).map_err(LoadError::Io)?;
                Ok(config)
            }
            Err(error) => Err(LoadError::Io(error)),
        }
    }

    pub fn save_value(section: &str, key: &str, value: &str) -> std::io::Result<()> {
        let Some(path) = path() else {
            return Ok(());
        };
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        std::fs::write(&path, toml::set(&text, section, key, value))
    }

    pub fn from_toml(text: &str) -> Result<Self, ParseError> {
        let table = toml::parse(text)?;
        let read = Reader { table: &table };
        let mut config = Self::default();
        read.bool("general", "autostart", &mut config.autostart)?;
        read.language("general", "language", &mut config.language)?;
        #[cfg(feature = "update")]
        read.update_mode("update", "mode", &mut config.update)?;

        #[cfg(feature = "orbit")]
        {
            read.bool("general", "pause_on_battery", &mut config.pause_on_battery)?;
            read.bool("modules", "orbit", &mut config.orbit_enabled)?;
            let o = &mut config.orbit;
            read.number("orbit", "radius_x", 4.0, 200.0, &mut o.radius_x)?;
            read.number("orbit", "radius_y", 4.0, 200.0, &mut o.radius_y)?;
            read.number("orbit", "period", 2.0, 3600.0, &mut o.period_secs)?;
            read.bool("orbit", "clockwise", &mut o.clockwise)?;
            read.bool("orbit", "rotate", &mut o.rotate)?;
            read.number("orbit", "core_radius", 4.0, 60.0, &mut o.core_radius)?;
            read.number(
                "orbit",
                "satellite_radius",
                2.0,
                30.0,
                &mut o.satellite_radius,
            )?;
            read.number("orbit", "layout_radius", 10.0, 300.0, &mut o.layout_radius)?;
            let mut delay = o.hover_delay_ms as f32;
            read.number("orbit", "hover_delay", 0.0, 2000.0, &mut delay)?;
            o.hover_delay_ms = delay.round() as u32;
            read.number("orbit", "hover_padding", 0.0, 60.0, &mut o.hover_padding)?;
            read.satellites("orbit", "satellites", &mut o.satellites)?;
            let n = &mut o.notices;
            read.bool("notices", "charger", &mut n.charger)?;
            read.bool("notices", "low_battery", &mut n.low_battery)?;
            read.bool("notices", "track", &mut n.track)?;
            read.number("notices", "seconds", 1.0, 30.0, &mut n.seconds)?;
            read.tiles("hub", "tiles", &mut o.tiles)?;
        }
        #[cfg(feature = "launcher")]
        {
            read.bool("modules", "launcher", &mut config.launcher_enabled)?;
            read.hotkey("launcher", "hotkey", &mut config.launcher.hotkey)?;
            read.string("launcher", "search", &mut config.launcher.search)?;
        }
        #[cfg(feature = "timers")]
        read.string("timers", "sound", &mut config.timer_sound)?;
        #[cfg(feature = "notes")]
        {
            read.bool("modules", "notes", &mut config.notes_enabled)?;
            read.notes_mode("notes", "mode", &mut config.notes_mode)?;
        }
        #[cfg(feature = "scenarios")]
        read.bool("modules", "scenarios", &mut config.scenarios_enabled)?;
        Ok(config)
    }

    pub fn to_toml(&self) -> String {
        let mut text = comment("Настройки Moonlet.", "Moonlet settings.");
        text += &comment(
            "Файл перечитывается сразу после сохранения — перезапускать программу не нужно.",
            "The file is reloaded as soon as you save it, no restart needed.",
        );
        text += &comment(
            "Размеры — в пикселях при масштабе экрана 100%.",
            "Sizes are in pixels at 100% display scale.",
        );
        text += "\n[general]\n";
        text += &comment(
            "Запускать Moonlet вместе с Windows.",
            "Start Moonlet with Windows.",
        );
        text += &format!("autostart = {}\n", self.autostart);
        text += &comment(
            "Язык: \"auto\" (как у Windows), \"ru\" или \"en\".",
            "Language: \"auto\" (same as Windows), \"ru\" or \"en\".",
        );
        text += &format!("language = \"{}\"\n", self.language.name());
        #[cfg(feature = "orbit")]
        {
            let o = &self.orbit;
            text += &comment(
                "Останавливать анимации, когда ноутбук работает от батареи.",
                "Stop animations while the laptop runs on battery.",
            );
            text += &format!("pause_on_battery = {}\n\n", self.pause_on_battery);
            text += "[modules]\n";
            text += &comment(
                "Выключенный модуль не загружается и не тратит память.",
                "A disabled module is not loaded and uses no memory.",
            );
            text += &format!("orbit = {}\n", self.orbit_enabled);
            #[cfg(feature = "launcher")]
            {
                text += &comment(
                    "Командная строка по горячей клавише (работает вместе с орбитой).",
                    "Command line on a hotkey (works together with the orbit).",
                );
                text += &format!("launcher = {}\n", self.launcher_enabled);
            }
            #[cfg(feature = "notes")]
            {
                text += &comment("Заметки на рабочем столе.", "Desktop notes.");
                text += &format!("notes = {}\n", self.notes_enabled);
            }
            #[cfg(feature = "scenarios")]
            {
                text += &comment(
                    "Сценарии: «запомнить работа» и «работа» в командной строке.",
                    "Scenarios: \"remember work\" and \"work\" in the command line.",
                );
                text += &format!("scenarios = {}\n", self.scenarios_enabled);
            }
            text += "\n";
            text += "[orbit]\n";
            text += &comment(
                "Радиусы орбиты. Одинаковые — круг, разные — овал.",
                "Orbit radii. Equal values make a circle, different ones an oval.",
            );
            text += &format!("radius_x = {}\n", number(o.radius_x));
            text += &format!("radius_y = {}\n", number(o.radius_y));
            text += &comment("Секунд на один оборот.", "Seconds per revolution.");
            text += &format!("period = {}\n", number(o.period_secs));
            text += &format!("clockwise = {}\n", o.clockwise);
            text += &comment(
                "Вращение вообще (то же, что пункт «Вращение» в меню трея).",
                "Rotation on or off (same as \"Rotation\" in the tray menu).",
            );
            text += &format!("rotate = {}\n", o.rotate);
            text += &format!("core_radius = {}\n", number(o.core_radius));
            text += &format!("satellite_radius = {}\n", number(o.satellite_radius));
            text += &comment(
                "Радиус полукруга, по которому спутники встают при наведении.",
                "Radius of the half circle the satellites line up on when hovered.",
            );
            text += &format!("layout_radius = {}\n", number(o.layout_radius));
            text += &comment(
                "Задержка перед раскрытием (мс) и запас зоны наведения вокруг ядра.",
                "Hover delay before expanding (ms) and extra hover margin around the core.",
            );
            text += &format!("hover_delay = {}\n", o.hover_delay_ms);
            text += &format!("hover_padding = {}\n", number(o.hover_padding));
            text += &comment(
                "Спутники по порядку: порядок — их место в раскладке слева направо.",
                "Satellites in order: the order is their place in the layout, left to right.",
            );
            text += &comment(
                "Доступны: \"clock\" (часы), \"music\" (музыка), \"battery\" (батарея).",
                "Available: \"clock\", \"music\", \"battery\".",
            );
            let names: Vec<String> = o
                .satellites
                .iter()
                .map(|kind| format!("\"{}\"", kind.name()))
                .collect();
            text += &format!("satellites = [{}]\n\n", names.join(", "));
            text += &notices_toml(&o.notices);
            text += &hub_toml(&o.tiles);
        }
        #[cfg(feature = "launcher")]
        {
            text += &launcher_toml(&self.launcher);
        }
        #[cfg(feature = "timers")]
        {
            text += "\n[timers]\n";
            text += &comment(
                "Таймеры и напоминания ставятся из командной строки: «таймер 10 мин», «напомни в 15:30 …».",
                "Timers and reminders come from the command line: \"timer 10 min\", \"remind me at 15:30 …\".",
            );
            text += &comment(
                "Звук: \"drop\" (мягкая «Капля»), \"off\" (без звука) или путь к файлу .wav.",
                "Sound: \"drop\" (a soft chime), \"off\" (silent) or a path to a .wav file.",
            );
            text += &format!("sound = {}\n", quoted(&self.timer_sound));
        }
        #[cfg(feature = "notes")]
        {
            text += "\n[notes]\n";
            text += &comment(
                "Новая заметка: \"top\" — поверх окон, \"desktop\" — на рабочем столе, \"normal\" — обычное окно.",
                "A new note: \"top\" stays on top, \"desktop\" sits on the desktop, \"normal\" is a normal window.",
            );
            text += &comment(
                "Сами заметки лежат в папке notes рядом с этим файлом.",
                "The notes themselves live in the notes folder next to this file.",
            );
            text += &format!("mode = \"{}\"\n", self.notes_mode.name());
        }
        #[cfg(feature = "update")]
        {
            text += "\n[update]\n";
            text += &comment(
                "Обновления с GitHub: \"auto\" — ставить самому, \"check\" — только сообщать, \"off\" — выключены.",
                "Updates from GitHub: \"auto\" installs them, \"check\" only tells you, \"off\" turns them off.",
            );
            text += &format!("mode = \"{}\"\n", self.update.name());
        }
        text
    }
}

fn comment(ru: &'static str, en: &'static str) -> String {
    format!("# {}\n", tr(ru, en))
}

#[cfg(feature = "orbit")]
fn notices_toml(n: &NoticeConfig) -> String {
    let mut text = String::from("[notices]\n");
    text += &comment(
        "Временные спутники: вылетают при событии и сами улетают обратно.",
        "Temporary satellites: fly out on an event and fly back by themselves.",
    );
    text += &comment(
        "Зарядку подключили или отключили.",
        "Charger plugged in or unplugged.",
    );
    text += &format!("charger = {}\n", n.charger);
    text += &comment(
        "Заряд опустился до 20% и до 10%.",
        "Battery dropped to 20% and to 10%.",
    );
    text += &format!("low_battery = {}\n", n.low_battery);
    text += &comment("Заиграл другой трек.", "A different song started.");
    text += &format!("track = {}\n", n.track);
    text += &comment(
        "Сколько секунд держится (предупреждение о заряде — вдвое дольше).",
        "How many seconds it stays (battery warnings stay twice as long).",
    );
    text += &format!("seconds = {}\n", number(n.seconds));
    text
}

#[cfg(feature = "orbit")]
fn hub_toml(tiles: &[Tile]) -> String {
    let mut text = String::from(
        "
[hub]
",
    );
    text += &comment(
        "Плитки хаба (клик по ядру) по порядку, не больше 16. Действия Moonlet:",
        "Hub tiles (click the core) in order, up to 16. Moonlet actions:",
    );
    text += &comment(
        "\"screenshot\", \"launcher\", \"play\", \"next\", \"previous\", \"bluetooth\",",
        "\"screenshot\", \"launcher\", \"play\", \"next\", \"previous\", \"bluetooth\",",
    );
    text += &comment(
        "\"note\" (новая заметка), \"rotation\", \"shape\", \"settings\".",
        "\"note\" (new note), \"rotation\", \"shape\", \"settings\".",
    );
    text += &comment(
        "Свои плитки: \"app:путь\", \"file:путь\", \"folder:путь\", \"url:адрес\", \"cmd:команда строки\";",
        "Your own tiles: \"app:path\", \"file:path\", \"folder:path\", \"url:address\", \"cmd:command line text\";",
    );
    text += &comment(
        "своё имя — после |, например \"url:https://github.com|GitHub\". В путях \\ пишется дважды.",
        "a custom name goes after |, e.g. \"url:https://github.com|GitHub\". Write \\ twice in paths.",
    );
    text += &format!(
        "tiles = {}
",
        tiles_value(tiles)
    );
    text
}

#[cfg(feature = "orbit")]
pub fn tiles_value(tiles: &[Tile]) -> String {
    let items: Vec<String> = tiles.iter().map(|t| quoted(&t.to_text())).collect();
    format!("[{}]", items.join(", "))
}

#[cfg(feature = "launcher")]
fn launcher_toml(l: &LauncherConfig) -> String {
    let mut text = String::from("\n[launcher]\n");
    text += &comment(
        "Клавиша командной строки: Ctrl, Alt, Shift, Win и одна клавиша (буква, цифра, F1–F24, Space).",
        "Command line hotkey: Ctrl, Alt, Shift, Win and one key (a letter, a digit, F1–F24, Space).",
    );
    text += &format!("hotkey = \"{}\"\n", l.hotkey.name());
    text += &comment(
        "Поиск в интернете: пусто — поисковик браузера по умолчанию,",
        "Web search: empty uses the default browser's search engine,",
    );
    text += &comment(
        "или адрес, к которому дописывается запрос, например \"https://duckduckgo.com/?q=\".",
        "or an address the query is appended to, e.g. \"https://duckduckgo.com/?q=\".",
    );
    text += &format!("search = {}\n", quoted(&l.search));
    text
}

#[cfg(feature = "orbit")]
pub fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

// Formatted by hand: std float formatting adds tens of KB to the binary.
#[cfg_attr(not(feature = "orbit"), allow(dead_code))]
pub fn number(value: f32) -> String {
    let hundredths = (value as f64 * 100.0).round() as i64;
    let sign = if hundredths < 0 { "-" } else { "" };
    let (whole, frac) = (hundredths.abs() / 100, hundredths.abs() % 100);
    match frac {
        0 => format!("{sign}{whole}"),
        f if f % 10 == 0 => format!("{sign}{whole}.{}", f / 10),
        f => format!("{sign}{whole}.{f:02}"),
    }
}

struct Reader<'a> {
    table: &'a Table,
}

impl Reader<'_> {
    fn bool(&self, section: &str, key: &str, out: &mut bool) -> Result<(), ParseError> {
        if let Some(entry) = self.table.get(section, key) {
            *out = entry.value.as_bool().ok_or(ParseError::at(
                entry.line,
                tr("ожидалось true или false", "expected true or false"),
            ))?;
        }
        Ok(())
    }

    fn language(&self, section: &str, key: &str, out: &mut Language) -> Result<(), ParseError> {
        if let Some(entry) = self.table.get(section, key) {
            *out = entry
                .value
                .as_str()
                .and_then(Language::from_name)
                .ok_or(ParseError::at(
                    entry.line,
                    tr(
                        "ожидалось \"auto\", \"ru\" или \"en\"",
                        "expected \"auto\", \"ru\" or \"en\"",
                    ),
                ))?;
        }
        Ok(())
    }

    #[cfg(feature = "update")]
    fn update_mode(
        &self,
        section: &str,
        key: &str,
        out: &mut crate::update::Mode,
    ) -> Result<(), ParseError> {
        if let Some(entry) = self.table.get(section, key) {
            *out = entry
                .value
                .as_str()
                .and_then(crate::update::Mode::from_name)
                .ok_or(ParseError::at(
                    entry.line,
                    tr(
                        "ожидалось \"auto\", \"check\" или \"off\"",
                        "expected \"auto\", \"check\" or \"off\"",
                    ),
                ))?;
        }
        Ok(())
    }

    #[cfg(feature = "notes")]
    fn notes_mode(
        &self,
        section: &str,
        key: &str,
        out: &mut crate::notes::store::Mode,
    ) -> Result<(), ParseError> {
        if let Some(entry) = self.table.get(section, key) {
            *out = entry
                .value
                .as_str()
                .and_then(crate::notes::store::Mode::from_name)
                .ok_or(ParseError::at(
                    entry.line,
                    tr(
                        "ожидалось \"top\", \"desktop\" или \"normal\"",
                        "expected \"top\", \"desktop\" or \"normal\"",
                    ),
                ))?;
        }
        Ok(())
    }

    #[cfg_attr(not(feature = "orbit"), allow(dead_code))]
    fn number(
        &self,
        section: &str,
        key: &str,
        min: f32,
        max: f32,
        out: &mut f32,
    ) -> Result<(), ParseError> {
        if let Some(entry) = self.table.get(section, key) {
            let value = entry.value.as_f64().ok_or(ParseError::at(
                entry.line,
                tr("ожидалось число", "expected a number"),
            ))?;
            *out = (value as f32).clamp(min, max);
        }
        Ok(())
    }

    #[cfg(feature = "launcher")]
    fn string(&self, section: &str, key: &str, out: &mut String) -> Result<(), ParseError> {
        if let Some(entry) = self.table.get(section, key) {
            *out = entry
                .value
                .as_str()
                .ok_or(ParseError::at(
                    entry.line,
                    tr("ожидалась строка в кавычках", "expected a string in quotes"),
                ))?
                .to_string();
        }
        Ok(())
    }

    #[cfg(feature = "launcher")]
    fn hotkey(&self, section: &str, key: &str, out: &mut Hotkey) -> Result<(), ParseError> {
        if let Some(entry) = self.table.get(section, key) {
            *out = entry
                .value
                .as_str()
                .and_then(Hotkey::parse)
                .ok_or(ParseError::at(
                    entry.line,
                    tr(
                        "ожидалась клавиша вида \"Ctrl+Alt+Space\"",
                        "expected a hotkey like \"Ctrl+Alt+Space\"",
                    ),
                ))?;
        }
        Ok(())
    }

    #[cfg(feature = "orbit")]
    fn tiles(&self, section: &str, key: &str, out: &mut Vec<Tile>) -> Result<(), ParseError> {
        let Some(entry) = self.table.get(section, key) else {
            return Ok(());
        };
        let error = ParseError::at(
            entry.line,
            tr(
                "ожидался список плиток вида [\"screenshot\", \"url:https://…\"], не больше 16, без повторов",
                "expected a list of tiles like [\"screenshot\", \"url:https://…\"], up to 16, without repeats",
            ),
        );
        let items = entry.value.as_array().ok_or(error.clone())?;
        if items.len() > MAX_TILES {
            return Err(error);
        }
        let mut tiles = Vec::with_capacity(items.len());
        for item in items {
            let tile = item.as_str().and_then(Tile::parse).ok_or(error.clone())?;
            if tiles.contains(&tile) {
                return Err(error);
            }
            tiles.push(tile);
        }
        *out = tiles;
        Ok(())
    }

    #[cfg(feature = "orbit")]
    fn satellites(
        &self,
        section: &str,
        key: &str,
        out: &mut Vec<SatelliteKind>,
    ) -> Result<(), ParseError> {
        let Some(entry) = self.table.get(section, key) else {
            return Ok(());
        };
        let error = ParseError::at(
            entry.line,
            tr(
                "ожидался список спутников вида [\"clock\", \"music\", \"battery\"] без повторов",
                "expected a list of satellites like [\"clock\", \"music\", \"battery\"] without repeats",
            ),
        );
        let items = entry.value.as_array().ok_or(error.clone())?;
        let mut kinds = Vec::with_capacity(items.len());
        for item in items {
            let kind = item
                .as_str()
                .and_then(SatelliteKind::from_name)
                .ok_or(error.clone())?;
            if kinds.contains(&kind) {
                return Err(error);
            }
            kinds.push(kind);
        }
        *out = kinds;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_file_round_trips() {
        let config = Config::default();
        assert_eq!(Config::from_toml(&config.to_toml()).unwrap(), config);
    }

    #[test]
    fn missing_keys_fall_back_to_defaults() {
        assert_eq!(Config::from_toml("").unwrap(), Config::default());
    }

    #[test]
    fn file_comments_follow_the_language() {
        crate::lang::set(Language::English);
        let en = Config::default().to_toml();
        crate::lang::set(Language::Russian);
        let ru = Config::default().to_toml();
        assert!(en.starts_with("# Moonlet settings."));
        assert!(ru.starts_with("# Настройки Moonlet."));
        assert_eq!(Config::from_toml(&en).unwrap(), Config::default());
    }

    #[test]
    fn reads_language() {
        let config = Config::from_toml("[general]\nlanguage = \"en\"\n").unwrap();
        assert_eq!(config.language, Language::English);
        assert!(Config::from_toml("[general]\nlanguage = \"de\"\n").is_err());
    }

    #[cfg(feature = "orbit")]
    #[test]
    fn reads_and_clamps_orbit_values() {
        let text = "[orbit]\nradius_x = 30\nradius_y = 500\nsatellites = [\"battery\"]\n";
        let config = Config::from_toml(text).unwrap();
        assert_eq!(config.orbit.radius_x, 30.0);
        assert_eq!(config.orbit.radius_y, 200.0);
        assert_eq!(config.orbit.satellites, vec![SatelliteKind::Battery]);
    }

    #[cfg(feature = "orbit")]
    #[test]
    fn wrong_types_point_to_the_line() {
        let error = Config::from_toml("[orbit]\nrotate = 1\n").unwrap_err();
        assert_eq!(error.line, 2);
        assert!(Config::from_toml("[orbit]\nsatellites = [\"radio\"]\n").is_err());
        assert!(Config::from_toml("[orbit]\nsatellites = [\"clock\", \"clock\"]\n").is_err());
    }

    #[cfg(feature = "launcher")]
    #[test]
    fn reads_launcher_settings() {
        let text = "[launcher]\nhotkey = \"Win+Shift+K\"\nsearch = \"https://x.org/?q=\"\n";
        let config = Config::from_toml(text).unwrap();
        assert_eq!(config.launcher.hotkey.name(), "Shift+Win+K");
        assert_eq!(config.launcher.search, "https://x.org/?q=");
        assert!(Config::from_toml("[launcher]\nhotkey = \"Space\"\n").is_err());
        let mut config = Config::default();
        config.launcher.search = "a\\b\"c".to_string();
        assert_eq!(Config::from_toml(&config.to_toml()).unwrap(), config);
    }

    #[cfg(feature = "orbit")]
    #[test]
    fn reads_hub_tiles() {
        let text = "[hub]
tiles = [\"settings\", \"app:C:\\\\A\\\\T.lnk\", \"url:https://x.org|X\"]
";
        let config = Config::from_toml(text).unwrap();
        assert_eq!(config.orbit.tiles.len(), 3);
        assert_eq!(config.orbit.tiles[1].title(), "T");
        assert_eq!(Config::from_toml(&config.to_toml()).unwrap(), config);
        assert!(
            Config::from_toml(
                "[hub]
tiles = [\"radio\"]
"
            )
            .is_err()
        );
        assert!(
            Config::from_toml(
                "[hub]
tiles = [\"shape\", \"shape\"]
"
            )
            .is_err()
        );
        let many = vec!["\"cmd:a\""; 17].join(", ");
        assert!(
            Config::from_toml(&format!(
                "[hub]
tiles = [{many}]
"
            ))
            .is_err()
        );
        let empty = Config::from_toml(
            "[hub]
tiles = []
",
        )
        .unwrap();
        assert!(empty.orbit.tiles.is_empty());
    }

    #[cfg(feature = "notes")]
    #[test]
    fn reads_notes_settings() {
        use crate::notes::store::Mode;
        let text = "[modules]\nnotes = false\n[notes]\nmode = \"desktop\"\n";
        let config = Config::from_toml(text).unwrap();
        assert!(!config.notes_enabled);
        assert_eq!(config.notes_mode, Mode::Desktop);
        assert!(Config::from_toml("[notes]\nmode = \"floating\"\n").is_err());
    }

    #[cfg(feature = "orbit")]
    #[test]
    fn shape_change_does_not_need_rebuild() {
        let a = OrbitConfig::default();
        let mut b = a.clone();
        b.radius_x = 30.0;
        b.rotate = false;
        b.notices.track = true;
        b.tiles.clear();
        assert!(!a.needs_rebuild(&b));
        b.period_secs = 10.0;
        assert!(a.needs_rebuild(&b));
    }
}
