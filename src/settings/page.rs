// In a trimmed build (no orbit or command line) some row kinds stay unused.
#![cfg_attr(
    not(all(feature = "orbit", feature = "launcher", feature = "update")),
    allow(dead_code)
)]

use crate::config::Config;
#[cfg(feature = "orbit")]
use crate::config::{OrbitConfig, SatelliteKind};
#[cfg(feature = "launcher")]
use crate::hotkey::Hotkey;
use crate::lang::{Language, tr};
#[cfg(feature = "orbit")]
use crate::tile::{Builtin, Kind, MAX_TILES, Tile};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    General,
    #[cfg(feature = "orbit")]
    Orbit,
    #[cfg(feature = "orbit")]
    Quick,
    #[cfg(feature = "launcher")]
    Launcher,
    #[cfg(feature = "timers")]
    Timers,
    #[cfg(feature = "notes")]
    Notes,
    #[cfg(feature = "scenarios")]
    Scenarios,
    About,
}

impl Section {
    pub fn all() -> Vec<Section> {
        vec![
            Section::General,
            #[cfg(feature = "orbit")]
            Section::Orbit,
            #[cfg(feature = "orbit")]
            Section::Quick,
            #[cfg(feature = "launcher")]
            Section::Launcher,
            #[cfg(feature = "timers")]
            Section::Timers,
            #[cfg(feature = "notes")]
            Section::Notes,
            #[cfg(feature = "scenarios")]
            Section::Scenarios,
            Section::About,
        ]
    }

    pub fn title(self) -> &'static str {
        match self {
            Section::General => tr("Общие", "General"),
            #[cfg(feature = "orbit")]
            Section::Orbit => tr("Орбита", "Orbit"),
            #[cfg(feature = "orbit")]
            Section::Quick => tr("Быстрый доступ", "Quick access"),
            #[cfg(feature = "launcher")]
            Section::Launcher => tr("Командная строка", "Command line"),
            #[cfg(feature = "timers")]
            Section::Timers => tr("Таймеры", "Timers"),
            #[cfg(feature = "notes")]
            Section::Notes => tr("Заметки", "Notes"),
            #[cfg(feature = "scenarios")]
            Section::Scenarios => tr("Сценарии", "Scenarios"),
            Section::About => tr("О программе", "About"),
        }
    }

    pub fn color(self) -> u32 {
        match self {
            Section::General => 0xF6AD55,
            #[cfg(feature = "orbit")]
            Section::Orbit => 0x9FB2FF,
            #[cfg(feature = "orbit")]
            Section::Quick => 0x4FD1C5,
            #[cfg(feature = "launcher")]
            Section::Launcher => 0xB794F4,
            #[cfg(feature = "timers")]
            Section::Timers => 0xF687B3,
            #[cfg(feature = "notes")]
            Section::Notes => 0xD9DCE3,
            #[cfg(feature = "scenarios")]
            Section::Scenarios => 0x68D391,
            Section::About => 0x6F7584,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Language,
    Autostart,
    #[cfg(feature = "orbit")]
    PauseOnBattery,
    #[cfg(feature = "update")]
    UpdateMode,
    #[cfg(feature = "update")]
    CheckNow,
    News,
    #[cfg(feature = "orbit")]
    OrbitModule,
    #[cfg(feature = "orbit")]
    Shape,
    #[cfg(feature = "orbit")]
    Rotate,
    #[cfg(feature = "orbit")]
    Direction,
    #[cfg(feature = "orbit")]
    Period,
    #[cfg(feature = "orbit")]
    HoverDelay,
    #[cfg(feature = "orbit")]
    Satellite(SatelliteKind),
    #[cfg(feature = "orbit")]
    Tile(usize),
    #[cfg(feature = "orbit")]
    AddKind,
    #[cfg(feature = "orbit")]
    AddBuiltin,
    #[cfg(feature = "orbit")]
    AppQuery,
    #[cfg(feature = "orbit")]
    AppPick,
    #[cfg(feature = "orbit")]
    Browse,
    #[cfg(feature = "orbit")]
    AddUrl,
    #[cfg(feature = "orbit")]
    AddCommand,
    #[cfg(feature = "orbit")]
    NoticeCharger,
    #[cfg(feature = "orbit")]
    NoticeLowBattery,
    #[cfg(feature = "orbit")]
    NoticeTrack,
    #[cfg(feature = "orbit")]
    NoticeSeconds,
    #[cfg(feature = "launcher")]
    LauncherModule,
    #[cfg(feature = "launcher")]
    Hotkey,
    #[cfg(feature = "launcher")]
    Search,
    #[cfg(feature = "launcher")]
    SearchUrl,
    #[cfg(feature = "timers")]
    TimerSound,
    #[cfg(feature = "timers")]
    TimerSoundFile,
    #[cfg(feature = "timers")]
    TimerSoundTest,
    #[cfg(feature = "timers")]
    CancelReminder(usize),
    #[cfg(feature = "notes")]
    NotesModule,
    #[cfg(feature = "notes")]
    NotesMode,
    #[cfg(feature = "notes")]
    NoteRow(usize),
    #[cfg(feature = "notes")]
    NotesFolder,
    #[cfg(feature = "scenarios")]
    ScenariosModule,
    #[cfg(feature = "scenarios")]
    ScenarioName,
    #[cfg(feature = "scenarios")]
    ScenarioCapture,
    #[cfg(feature = "scenarios")]
    ScenarioRow(usize),
    #[cfg(feature = "scenarios")]
    ScenarioWindow(usize),
    #[cfg(feature = "scenarios")]
    ScenarioUrl(usize, usize),
    #[cfg(feature = "scenarios")]
    ScenarioAddUrl(usize),
    #[cfg(feature = "scenarios")]
    ScenariosFolder,
    #[cfg(all(feature = "orbit", feature = "scenarios"))]
    AddScenario,
    OpenFile,
    Website,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slider {
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub unit: &'static str,
}

impl Slider {
    pub fn snap(&self, value: f32) -> f32 {
        let steps = ((value - self.min) / self.step).round();
        (self.min + steps * self.step).clamp(self.min, self.max)
    }

    pub fn fraction(&self) -> f32 {
        ((self.value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Control {
    Header,
    Toggle(bool),
    Choice(Vec<&'static str>, Option<usize>),
    List(Vec<&'static str>, usize),
    Slider(Slider),
    Hotkey(String),
    Field(String),
    Button(&'static str),
    Order { on: bool, up: bool, down: bool },
    Tile { up: bool, down: bool },
    Picks(Vec<String>),
    Info(String),
    Note { strong: bool },
}

pub struct Row {
    pub setting: Option<Setting>,
    pub label: String,
    pub hint: String,
    pub control: Control,
    pub dim: bool,
    pub warn: bool,
}

impl Row {
    fn new(setting: Setting, label: &str, hint: &str, control: Control) -> Self {
        Self {
            setting: Some(setting),
            label: label.to_string(),
            hint: hint.to_string(),
            control,
            dim: false,
            warn: false,
        }
    }

    fn caption(label: &str) -> Self {
        Self {
            setting: None,
            label: label.to_string(),
            hint: String::new(),
            control: Control::Header,
            dim: false,
            warn: false,
        }
    }

    fn info(label: &str) -> Self {
        Self {
            setting: None,
            label: label.to_string(),
            hint: String::new(),
            control: Control::Info(String::new()),
            dim: false,
            warn: false,
        }
    }

    fn dim(mut self, dim: bool) -> Self {
        self.dim = dim;
        self
    }

    fn message(mut self, ui: &Ui) -> Self {
        if let (Some((target, text, warn)), Some(setting)) = (ui.message, self.setting)
            && target == setting
        {
            self.hint = text.to_string();
            self.warn = warn;
        }
        self
    }

    pub fn focusable(&self) -> bool {
        self.setting.is_some()
            && !matches!(
                self.control,
                Control::Header | Control::Info(_) | Control::Note { .. }
            )
    }
}

#[derive(Default)]
pub struct Ui {
    pub custom_search: bool,
    pub message: Option<(Setting, &'static str, bool)>,
    pub file_error: Option<String>,
    pub has_battery: bool,
    pub news_open: bool,
    #[cfg(feature = "orbit")]
    pub add_kind: usize,
    #[cfg(feature = "orbit")]
    pub app_query: String,
    #[cfg(feature = "orbit")]
    pub apps: Vec<crate::apps::App>,
    #[cfg(feature = "timers")]
    pub reminders: Vec<crate::reminders::Reminder>,
    #[cfg(feature = "timers")]
    pub custom_sound: bool,
    #[cfg(feature = "notes")]
    pub notes: Vec<NoteInfo>,
    #[cfg(feature = "notes")]
    pub note_armed: Option<u64>,
    #[cfg(feature = "scenarios")]
    pub scenarios: Vec<crate::scenarios::store::Scenario>,
    #[cfg(feature = "scenarios")]
    pub scenario_name: String,
    #[cfg(feature = "scenarios")]
    pub scenario_note: String,
    #[cfg(feature = "scenarios")]
    pub scenario_warn: bool,
    #[cfg(feature = "scenarios")]
    pub replace_armed: bool,
    #[cfg(feature = "scenarios")]
    pub scenario_open: Option<usize>,
    #[cfg(feature = "scenarios")]
    pub scenario_armed: Option<usize>,
}

#[cfg(feature = "notes")]
#[derive(Clone, Debug, PartialEq)]
pub struct NoteInfo {
    pub id: u64,
    pub title: String,
    pub hint: String,
}

#[cfg(feature = "notes")]
pub fn note_infos() -> Vec<NoteInfo> {
    use crate::notes::store::{self, Mode};
    store::load()
        .into_iter()
        .map(|note| {
            let title = store::first_line(&store::read_text(note.id));
            let hint = match (&note.attach, note.mode) {
                (Some(a), _) => {
                    let name = if a.tab.is_empty() {
                        a.exe.trim_end_matches(".exe")
                    } else {
                        a.tab.as_str()
                    };
                    format!("{} {name}", tr("Приклеена к окну:", "Stuck to:"))
                }
                (None, Mode::Top) => tr("Поверх окон", "Always on top").to_string(),
                (None, Mode::Desktop) => tr("На рабочем столе", "On the desktop").to_string(),
                (None, Mode::Normal) => tr("Обычное окно", "Normal window").to_string(),
            };
            NoteInfo {
                id: note.id,
                title: if title.is_empty() {
                    tr("Пустая заметка", "Empty note").to_string()
                } else {
                    title
                },
                hint,
            }
        })
        .collect()
}

const NEWS: [(&str, &[(&str, &str)]); 2] = [
    (
        "0.7.0",
        &[
            (
                "У Moonlet теперь свой значок-полумесяц в Проводнике, на ярлыках и в диспетчере задач, а в свойствах файла — имя и версия.",
                "Moonlet now has its own crescent icon in File Explorer, on shortcuts and in Task Manager, and its name and version in the file properties.",
            ),
            (
                "Заметки: значок слева стал кнопкой цвета луны. Щелчок приклеивает заметку к окну под ней, а если окна нет — к рабочему столу; ещё щелчок — отклеивает. Правый щелчок — выбор режима. Кнопки «⋯» больше нет.",
                "Notes: the icon on the left is now a moon-colored button. A click sticks the note to the window under it, or to the desktop if there is none; another click unsticks it. Right-click to pick the mode. The \"⋯\" button is gone.",
            ),
            (
                "Сценарии: «запомнить работа» в строке запоминает открытые окна — программы, места, папки Проводника и вкладки браузера. Потом просто «работа»: недостающее запустится, окна встанут по местам. С несколькими экранами строка спросит, на какой.",
                "Scenarios: \"remember work\" in the command line saves the open windows: apps, places, File Explorer folders and browser tabs. Later just type \"work\": whatever is missing starts and the windows go back in place. With several screens the command line asks which one.",
            ),
            (
                "Раздел «Сценарии» в настройках: запомнить окна, убрать лишнее окно, добавить вкладку, удалить сценарий. Сценарий можно поставить плиткой в хаб.",
                "A Scenarios section in the settings: save the windows, drop a window, add a tab, delete a scenario. A scenario can also be a hub tile.",
            ),
        ],
    ),
    (
        "0.6.1",
        &[
            (
                "Заметки: «заметка купить хлеб» в строке, плитка хаба или «Новая заметка» в меню трея. Ссылки в тексте открываются щелчком, Ctrl+колёсико меняет размер текста.",
                "Notes: \"note buy milk\" in the command line, a hub tile or \"New note\" in the tray menu. Links in the text open with a click; Ctrl+wheel changes the text size.",
            ),
            (
                "Заметка держится поверх окон, на рабочем столе или как обычное окно — выбор в её меню «⋯». Можно приклеить к окну программы: поставьте заметку на окно — она будет ездить с ним, а у браузера появляться только на своей вкладке.",
                "A note stays on top, on the desktop or as a normal window, set in its \"⋯\" menu. Put a note over a program's window and stick it there: it moves with the window, and in a browser it shows up only on its own tab.",
            ),
            (
                "Раздел «Заметки» в настройках: режим новой заметки, список всех заметок и папка, где каждая заметка — обычный .txt.",
                "A Notes section in the settings: the mode for new notes, a list of all notes and the folder where each note is a plain .txt file.",
            ),
        ],
    ),
];

fn news(rows: &mut Vec<Row>) {
    for (version, items) in NEWS {
        rows.push(Row {
            setting: None,
            label: version.to_string(),
            hint: String::new(),
            control: Control::Note { strong: true },
            dim: false,
            warn: false,
        });
        for &(ru, en) in items {
            rows.push(Row {
                setting: None,
                label: tr(ru, en).to_string(),
                hint: String::new(),
                control: Control::Note { strong: false },
                dim: false,
                warn: false,
            });
        }
    }
}

#[cfg(feature = "launcher")]
const SEARCH: [(&str, &str); 5] = [
    ("", ""),
    ("Google", "https://www.google.com/search?q="),
    ("DuckDuckGo", "https://duckduckgo.com/?q="),
    ("Bing", "https://www.bing.com/search?q="),
    ("", "https://yandex.ru/search/?text="),
];

#[cfg(feature = "launcher")]
fn search_preset(url: &str) -> Option<usize> {
    SEARCH.iter().position(|(_, address)| *address == url)
}

pub fn rows(section: Section, config: &Config, ui: &Ui) -> Vec<Row> {
    let mut rows = Vec::new();
    if let Some(error) = &ui.file_error {
        rows.push(Row {
            setting: Some(Setting::OpenFile),
            label: tr("В config.toml ошибка", "config.toml has an error").to_string(),
            hint: error.clone(),
            control: Control::Button(tr("Открыть", "Open")),
            dim: false,
            warn: true,
        });
    }
    match section {
        Section::General => general(&mut rows, config, ui),
        #[cfg(feature = "orbit")]
        Section::Orbit => orbit(&mut rows, config),
        #[cfg(feature = "orbit")]
        Section::Quick => quick(&mut rows, config, ui),
        #[cfg(feature = "launcher")]
        Section::Launcher => launcher(&mut rows, config, ui),
        #[cfg(feature = "timers")]
        Section::Timers => timers(&mut rows, config, ui),
        #[cfg(feature = "notes")]
        Section::Notes => notes(&mut rows, config, ui),
        #[cfg(feature = "scenarios")]
        Section::Scenarios => scenarios(&mut rows, config, ui),
        Section::About => about(&mut rows),
    }
    rows
}

fn general(rows: &mut Vec<Row>, config: &Config, ui: &Ui) {
    let language = Language::ALL
        .iter()
        .position(|(lang, _)| *lang == config.language)
        .unwrap_or(0);
    rows.push(Row::caption(tr("Интерфейс", "Interface")));
    rows.push(Row::new(
        Setting::Language,
        tr("Язык", "Language"),
        "",
        Control::Choice(
            vec![tr("Как в Windows", "As Windows"), "Русский", "English"],
            Some(language),
        ),
    ));
    rows.push(Row::new(
        Setting::Autostart,
        tr("Запускать с Windows", "Start with Windows"),
        "",
        Control::Toggle(config.autostart),
    ));
    #[cfg(feature = "orbit")]
    if ui.has_battery {
        rows.push(Row::caption(tr("Батарея", "Battery")));
        rows.push(Row::new(
            Setting::PauseOnBattery,
            tr("Пауза от батареи", "Pause on battery"),
            tr(
                "Спутники стоят без зарядки",
                "Satellites stop when unplugged",
            ),
            Control::Toggle(config.pause_on_battery),
        ));
    }
    #[cfg(feature = "update")]
    {
        use crate::update::Mode;
        let mode = Mode::ALL
            .iter()
            .position(|(mode, _)| *mode == config.update)
            .unwrap_or(0);
        rows.push(Row::caption(tr("Обновления", "Updates")));
        rows.push(Row::new(
            Setting::UpdateMode,
            tr("Новые версии", "New versions"),
            "",
            Control::Choice(
                vec![
                    tr("Ставить сами", "Install"),
                    tr("Сообщать", "Notify"),
                    tr("Выкл.", "Off"),
                ],
                Some(mode),
            ),
        ));
        let label = format!(
            "{} {}",
            tr("Установлена", "Installed"),
            crate::update::current_version()
        );
        rows.push(
            Row::new(
                Setting::CheckNow,
                &label,
                "",
                Control::Button(tr("Проверить", "Check")),
            )
            .message(ui),
        );
    }
    rows.push(Row::new(
        Setting::News,
        &format!(
            "{} {}",
            tr("Что нового в", "What's new in"),
            env!("CARGO_PKG_VERSION")
        ),
        "",
        Control::Button(if ui.news_open {
            tr("Скрыть", "Hide")
        } else {
            tr("Показать", "Show")
        }),
    ));
    if ui.news_open {
        news(rows);
    }
}

#[cfg(feature = "orbit")]
fn orbit(rows: &mut Vec<Row>, config: &Config) {
    let o = &config.orbit;
    let off = !config.orbit_enabled;
    rows.push(Row::caption(tr("Модуль", "Module")));
    rows.push(Row::new(
        Setting::OrbitModule,
        tr("Орбита", "Orbit"),
        tr(
            "Выключенная не тратит ни памяти, ни видеокарты",
            "When off it uses no memory and no GPU",
        ),
        Control::Toggle(config.orbit_enabled),
    ));
    let shape = match (o.radius_x, o.radius_y) {
        radii if radii == OrbitConfig::CIRCLE => Some(0),
        radii if radii == OrbitConfig::OVAL => Some(1),
        _ => None,
    };
    rows.push(Row::caption(tr("Движение", "Motion")));
    rows.push(
        Row::new(
            Setting::Shape,
            tr("Форма", "Shape"),
            "",
            Control::Choice(vec![tr("Круг", "Circle"), tr("Овал", "Oval")], shape),
        )
        .dim(off),
    );
    rows.push(
        Row::new(
            Setting::Rotate,
            tr("Вращение", "Rotation"),
            "",
            Control::Toggle(o.rotate),
        )
        .dim(off),
    );
    rows.push(
        Row::new(
            Setting::Direction,
            tr("Направление", "Direction"),
            "",
            Control::Choice(
                vec![tr("По часовой", "Clockwise"), tr("Против", "Counter")],
                Some(if o.clockwise { 0 } else { 1 }),
            ),
        )
        .dim(off),
    );
    rows.push(
        Row::new(
            Setting::Period,
            tr("Один оборот", "One revolution"),
            "",
            Control::Slider(Slider {
                value: o.period_secs,
                min: 10.0,
                max: 120.0,
                step: 5.0,
                unit: tr("с", "s"),
            }),
        )
        .dim(off),
    );
    rows.push(Row::caption(tr("Наведение", "Hover")));
    rows.push(
        Row::new(
            Setting::HoverDelay,
            tr("Задержка", "Delay"),
            tr(
                "Защита от случайного раскрытия",
                "Guards against accidental expanding",
            ),
            Control::Slider(Slider {
                value: o.hover_delay_ms as f32,
                min: 0.0,
                max: 500.0,
                step: 10.0,
                unit: tr("мс", "ms"),
            }),
        )
        .dim(off),
    );
}

#[cfg(feature = "orbit")]
fn quick(rows: &mut Vec<Row>, config: &Config, ui: &Ui) {
    let o = &config.orbit;
    let off = !config.orbit_enabled;
    rows.push(Row::caption(tr(
        "На орбите — слева направо",
        "On the orbit, left to right",
    )));
    let count = o.satellites.len();
    for (index, kind) in satellite_order(&o.satellites).into_iter().enumerate() {
        let on = index < count;
        let (label, hint) = match kind {
            SatelliteKind::Clock => (tr("Часы", "Clock"), ""),
            SatelliteKind::Music => (tr("Музыка", "Music"), ""),
            SatelliteKind::Battery => (
                tr("Батарея", "Battery"),
                tr("Без батареи не показывается", "Hidden without a battery"),
            ),
        };
        rows.push(
            Row::new(
                Setting::Satellite(kind),
                label,
                hint,
                Control::Order {
                    on,
                    up: on && index > 0,
                    down: on && index + 1 < count,
                },
            )
            .dim(off),
        );
    }
    hub(rows, config, ui);
    let n = &o.notices;
    rows.push(Row::caption(tr(
        "Временные спутники",
        "Temporary satellites",
    )));
    rows.push(
        Row::new(
            Setting::NoticeCharger,
            tr("Зарядка", "Charger"),
            "",
            Control::Toggle(n.charger),
        )
        .dim(off),
    );
    rows.push(
        Row::new(
            Setting::NoticeLowBattery,
            tr("Низкий заряд", "Low battery"),
            tr("При 20% и 10%", "At 20% and 10%"),
            Control::Toggle(n.low_battery),
        )
        .dim(off),
    );
    rows.push(
        Row::new(
            Setting::NoticeTrack,
            tr("Новый трек", "New song"),
            "",
            Control::Toggle(n.track),
        )
        .dim(off),
    );
    rows.push(
        Row::new(
            Setting::NoticeSeconds,
            tr("Сколько держится", "How long it stays"),
            "",
            Control::Slider(Slider {
                value: n.seconds,
                min: 1.0,
                max: 30.0,
                step: 1.0,
                unit: tr("с", "s"),
            }),
        )
        .dim(off),
    );
}

#[cfg(feature = "orbit")]
fn hub(rows: &mut Vec<Row>, config: &Config, ui: &Ui) {
    let tiles = &config.orbit.tiles;
    let off = !config.orbit_enabled;
    rows.push(Row::caption(tr(
        "В хабе (клик по ядру) — по порядку",
        "In the hub (click the core), in order",
    )));
    if tiles.is_empty() {
        rows.push(Row::info(tr(
            "Пусто — в хабе будут только «Настройки»",
            "Empty: the hub will only have Settings",
        )));
    }
    for (index, tile) in tiles.iter().enumerate() {
        rows.push(
            Row::new(
                Setting::Tile(index),
                &tile.title(),
                &tile_hint(tile, config),
                Control::Tile {
                    up: index > 0,
                    down: index + 1 < tiles.len(),
                },
            )
            .dim(off),
        );
    }

    rows.push(Row::caption(tr("Добавить плитку", "Add a tile")));
    if tiles.len() >= MAX_TILES {
        rows.push(Row::info(tr(
            "В хабе уже 16 плиток — больше не поместится",
            "The hub already has 16 tiles, there is no room for more",
        )));
        return;
    }
    let kinds = add_kinds();
    let names = kinds
        .iter()
        .map(|kind| kind.map_or(tr("Действие", "Action"), Kind::title))
        .collect();
    let picked = ui.add_kind.min(kinds.len() - 1);
    rows.push(
        Row::new(
            Setting::AddKind,
            "",
            "",
            Control::Choice(names, Some(picked)),
        )
        .dim(off),
    );
    match kinds[picked] {
        None => {
            let free = free_builtins(tiles);
            if free.is_empty() {
                rows.push(Row::info(tr(
                    "Все действия Moonlet уже в хабе",
                    "All Moonlet actions are already in the hub",
                )));
            } else {
                let names = free.iter().map(|b| b.title().to_string()).collect();
                rows.push(Row::new(Setting::AddBuiltin, "", "", Control::Picks(names)).dim(off));
            }
        }
        Some(Kind::App) => {
            rows.push(
                Row::new(
                    Setting::AppQuery,
                    tr("Найти программу", "Find an app"),
                    tr("Программы из меню «Пуск»", "Apps from the Start menu"),
                    Control::Field(ui.app_query.clone()),
                )
                .dim(off),
            );
            let found = app_matches(ui);
            if !found.is_empty() {
                let names = found.iter().map(|app| app.name.clone()).collect();
                rows.push(Row::new(Setting::AppPick, "", "", Control::Picks(names)).dim(off));
            } else if !ui.app_query.trim().is_empty() {
                rows.push(Row::info(tr(
                    "Ничего не нашлось — выберите файл программы ниже",
                    "Nothing found, choose the app file below",
                )));
            }
            rows.push(
                Row::new(
                    Setting::Browse,
                    tr("Нет в «Пуске»?", "Not in the Start menu?"),
                    "",
                    Control::Button(tr("Выбрать файл…", "Choose a file…")),
                )
                .dim(off),
            );
        }
        Some(Kind::File) => rows.push(
            Row::new(
                Setting::Browse,
                tr(
                    "Документ, картинка, что угодно",
                    "A document, a picture, anything",
                ),
                tr(
                    "Откроется программой, как двойным щелчком",
                    "Opens in its app, like a double click",
                ),
                Control::Button(tr("Выбрать файл…", "Choose a file…")),
            )
            .dim(off),
        ),
        Some(Kind::Folder) => rows.push(
            Row::new(
                Setting::Browse,
                tr("Папка", "Folder"),
                tr("Откроется в Проводнике", "Opens in File Explorer"),
                Control::Button(tr("Выбрать папку…", "Choose a folder…")),
            )
            .dim(off),
        ),
        Some(Kind::Url) => rows.push(
            Row::new(
                Setting::AddUrl,
                tr("Адрес сайта", "Website address"),
                tr("Enter — добавить", "Enter to add"),
                Control::Field(String::new()),
            )
            .dim(off),
        ),
        Some(Kind::Command) => rows.push(
            Row::new(
                Setting::AddCommand,
                tr("Команда строки", "Command line text"),
                tr("Например, «громкость 30»", "For example, \"volume 30\""),
                Control::Field(String::new()),
            )
            .dim(off),
        ),
        #[cfg(feature = "scenarios")]
        Some(Kind::Scenario) => {
            let free = free_scenarios(ui, tiles);
            if free.is_empty() {
                rows.push(Row::info(if ui.scenarios.is_empty() {
                    tr(
                        "Сценариев пока нет — «запомнить работа» в строке",
                        "No scenarios yet: \"remember work\" in the command line",
                    )
                } else {
                    tr(
                        "Все сценарии уже в хабе",
                        "All scenarios are already in the hub",
                    )
                }));
            } else {
                let names = free.iter().map(|n| n.to_string()).collect();
                rows.push(Row::new(Setting::AddScenario, "", "", Control::Picks(names)).dim(off));
            }
        }
        #[cfg(not(feature = "scenarios"))]
        Some(Kind::Scenario) => {}
    }
}

#[cfg(feature = "orbit")]
fn tile_hint(tile: &Tile, config: &Config) -> String {
    let music = cfg!(feature = "music") && config.orbit.satellites.contains(&SatelliteKind::Music);
    #[cfg(feature = "launcher")]
    let launcher = config.launcher_enabled;
    #[cfg(not(feature = "launcher"))]
    let launcher = false;
    let text = match tile {
        Tile::Builtin(Builtin::Launcher) if !launcher => tr(
            "Командная строка выключена — плитки не видно",
            "The command line is off, the tile is hidden",
        ),
        Tile::Builtin(Builtin::PlayPause | Builtin::Next | Builtin::Previous) if !music => tr(
            "Видна, когда на орбите есть «Музыка»",
            "Shown while Music is on the orbit",
        ),
        Tile::Builtin(Builtin::Bluetooth) if !cfg!(feature = "bluetooth") => tr(
            "Bluetooth нет в этой сборке — плитки не видно",
            "Bluetooth is missing in this build, the tile is hidden",
        ),
        Tile::Builtin(Builtin::Bluetooth) => tr(
            "Зажать — устройства. Без адаптера не видна",
            "Hold for devices. Hidden without an adapter",
        ),
        Tile::Custom {
            kind: Kind::Command,
            ..
        } if !cfg!(feature = "launcher") => tr(
            "Нужна командная строка — её нет в этой сборке",
            "Needs the command line, missing in this build",
        ),
        Tile::Custom { kind, target, name } if *kind != Kind::App && name != target => {
            return format!("{} · {target}", kind.title());
        }
        Tile::Custom { kind, .. } => kind.title(),
        Tile::Builtin(_) => "",
    };
    text.to_string()
}

#[cfg(feature = "orbit")]
fn add_kinds() -> Vec<Option<Kind>> {
    let mut kinds = vec![
        None,
        Some(Kind::App),
        Some(Kind::File),
        Some(Kind::Folder),
        Some(Kind::Url),
    ];
    if cfg!(feature = "launcher") {
        kinds.push(Some(Kind::Command));
    }
    if cfg!(feature = "scenarios") {
        kinds.push(Some(Kind::Scenario));
    }
    kinds
}

#[cfg(feature = "orbit")]
fn free_builtins(tiles: &[Tile]) -> Vec<Builtin> {
    Builtin::ALL
        .iter()
        .map(|(builtin, _)| *builtin)
        .filter(|builtin| !tiles.contains(&Tile::Builtin(*builtin)))
        .collect()
}

#[cfg(feature = "orbit")]
const APP_MATCHES: usize = 6;

#[cfg(feature = "orbit")]
fn app_matches(ui: &Ui) -> Vec<&crate::apps::App> {
    let query = ui.app_query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<(i32, &crate::apps::App)> = ui
        .apps
        .iter()
        .filter_map(|app| {
            let file = app.file_key.as_deref();
            let points = crate::apps::score(&app.key, &query)
                .max(file.and_then(|f| crate::apps::score(f, &query)))?;
            Some((points, app))
        })
        .collect();
    found.sort_by_key(|(points, _)| std::cmp::Reverse(*points));
    found
        .into_iter()
        .take(APP_MATCHES)
        .map(|(_, app)| app)
        .collect()
}

#[cfg(feature = "orbit")]
fn add_tile(config: &mut Config, tile: Tile) -> Vec<Write> {
    let tiles = &mut config.orbit.tiles;
    let empty = matches!(&tile, Tile::Custom { target, .. } if target.trim().is_empty());
    if empty || tiles.len() >= MAX_TILES || tiles.contains(&tile) {
        return Vec::new();
    }
    tiles.push(tile);
    vec![tiles_write(tiles)]
}

#[cfg(feature = "orbit")]
fn tiles_write(tiles: &[Tile]) -> Write {
    write("hub", "tiles", crate::config::tiles_value(tiles))
}

#[cfg(feature = "orbit")]
fn satellite_order(enabled: &[SatelliteKind]) -> Vec<SatelliteKind> {
    let mut order = enabled.to_vec();
    for kind in [
        SatelliteKind::Clock,
        SatelliteKind::Music,
        SatelliteKind::Battery,
    ] {
        if !order.contains(&kind) {
            order.push(kind);
        }
    }
    order
}

#[cfg(feature = "launcher")]
fn launcher(rows: &mut Vec<Row>, config: &Config, ui: &Ui) {
    let l = &config.launcher;
    let off = !config.orbit_enabled || !config.launcher_enabled;
    rows.push(Row::caption(tr("Модуль", "Module")));
    rows.push(
        Row::new(
            Setting::LauncherModule,
            tr("Командная строка", "Command line"),
            if config.orbit_enabled {
                ""
            } else {
                tr(
                    "Работает только вместе с орбитой",
                    "Works only with the orbit",
                )
            },
            Control::Toggle(config.launcher_enabled),
        )
        .dim(!config.orbit_enabled),
    );
    rows.push(
        Row::new(
            Setting::Hotkey,
            tr("Горячая клавиша", "Hotkey"),
            "",
            Control::Hotkey(l.hotkey.name()),
        )
        .dim(off)
        .message(ui),
    );

    let custom = SEARCH.len();
    let selected = if ui.custom_search {
        custom
    } else {
        search_preset(&l.search).unwrap_or(custom)
    };
    let mut names: Vec<&'static str> = SEARCH.iter().map(|(name, _)| *name).collect();
    names[0] = tr("Поисковик браузера", "Browser's search engine");
    names[4] = tr("Яндекс", "Yandex");
    names.push(tr("Свой адрес", "Custom address"));
    rows.push(Row::caption(tr("Поиск в интернете", "Web search")));
    rows.push(Row::new(Setting::Search, "", "", Control::List(names, selected)).dim(off));
    if selected == custom {
        rows.push(
            Row::new(
                Setting::SearchUrl,
                tr("Адрес", "Address"),
                tr("Запрос допишется в конец", "The query goes at the end"),
                Control::Field(l.search.clone()),
            )
            .dim(off),
        );
    }
}

#[cfg(feature = "timers")]
fn sound_choice(config: &Config, ui: &Ui) -> usize {
    match config.timer_sound.as_str() {
        _ if ui.custom_sound => 1,
        crate::sound::OFF => 2,
        "" | crate::sound::DROP => 0,
        _ => 1,
    }
}

#[cfg(feature = "timers")]
fn timers(rows: &mut Vec<Row>, config: &Config, ui: &Ui) {
    use crate::reminders;
    rows.push(Row::caption(tr("Звук", "Sound")));
    let choice = sound_choice(config, ui);
    rows.push(Row::new(
        Setting::TimerSound,
        tr("Когда срабатывает", "When it goes off"),
        "",
        Control::Choice(
            vec![
                tr("Капля", "Chime"),
                tr("Свой файл", "Own file"),
                tr("Без звука", "Silent"),
            ],
            Some(choice),
        ),
    ));
    if choice == 1 {
        let path = std::path::Path::new(&config.timer_sound);
        let name = match path.file_name() {
            Some(name) if sound_choice(config, &Ui::default()) == 1 => {
                name.to_string_lossy().into_owned()
            }
            _ => tr("Файл .wav не выбран", "No .wav file chosen").to_string(),
        };
        rows.push(Row::new(
            Setting::TimerSoundFile,
            tr("Файл", "File"),
            &name,
            Control::Button(tr("Выбрать…", "Choose…")),
        ));
    }
    rows.push(
        Row::new(
            Setting::TimerSoundTest,
            tr("Проверить", "Try it"),
            "",
            Control::Button(tr("Послушать", "Play")),
        )
        .dim(choice == 2),
    );

    rows.push(Row::caption(tr("Активные", "Active")));
    if ui.reminders.is_empty() {
        rows.push(Row::info(tr(
            "Пусто. В строке: «таймер 10 мин», «напомни в 15:30 …»",
            "None. In the command line: \"timer 10 min\", \"remind me at 15:30 …\"",
        )));
    }
    let now = reminders::now();
    for (index, r) in ui.reminders.iter().enumerate() {
        let when = reminders::due_text(r.due, now);
        rows.push(Row::new(
            Setting::CancelReminder(index),
            &r.title(),
            &format!("{} {when}", tr("Сработает в", "Goes off at")),
            Control::Button(tr("Отменить", "Cancel")),
        ));
    }
}

#[cfg(feature = "notes")]
fn notes(rows: &mut Vec<Row>, config: &Config, ui: &Ui) {
    use crate::notes::store::Mode;
    let off = !config.notes_enabled;
    rows.push(Row::caption(tr("Модуль", "Module")));
    rows.push(Row::new(
        Setting::NotesModule,
        tr("Заметки", "Notes"),
        tr(
            "Создать: «заметка …» в строке, плитка хаба или меню трея",
            "Create one: \"note …\" in the command line, a hub tile or the tray menu",
        ),
        Control::Toggle(config.notes_enabled),
    ));
    let mode = Mode::ALL
        .iter()
        .position(|(m, _)| *m == config.notes_mode)
        .unwrap_or(0);
    rows.push(
        Row::new(
            Setting::NotesMode,
            tr("Новая заметка", "A new note"),
            "",
            Control::Choice(
                vec![
                    tr("Поверх окон", "On top"),
                    tr("На столе", "Desktop"),
                    tr("Обычное окно", "Normal"),
                ],
                Some(mode),
            ),
        )
        .dim(off),
    );
    rows.push(Row::info(tr(
        "Приклеить к окну — в меню «⋯» самой заметки",
        "Stick to a window: in the note's own \"⋯\" menu",
    )));

    rows.push(Row::caption(tr("Все заметки", "All notes")));
    if ui.notes.is_empty() {
        rows.push(Row::info(tr("Заметок пока нет", "No notes yet")));
    }
    for (index, note) in ui.notes.iter().enumerate() {
        rows.push(
            Row::new(
                Setting::NoteRow(index),
                &note.title,
                &note.hint,
                Control::Choice(vec![tr("Показать", "Show"), tr("Удалить", "Delete")], None),
            )
            .message(ui),
        );
    }
    rows.push(Row::new(
        Setting::NotesFolder,
        tr("Папка с заметками", "Notes folder"),
        tr(
            "Текст каждой заметки — файл .txt",
            "Each note is a .txt file",
        ),
        Control::Button(tr("Открыть", "Open")),
    ));
}

#[cfg(feature = "scenarios")]
fn scenarios(rows: &mut Vec<Row>, config: &Config, ui: &Ui) {
    use crate::scenarios::windows_text;
    let off = !config.scenarios_enabled;
    rows.push(Row::caption(tr("Модуль", "Module")));
    rows.push(Row::new(
        Setting::ScenariosModule,
        tr("Сценарии", "Scenarios"),
        tr(
            "«запомнить работа» в строке, потом просто «работа»",
            "\"remember work\" in the command line, then just \"work\"",
        ),
        Control::Toggle(config.scenarios_enabled),
    ));

    rows.push(Row::caption(tr("Новый сценарий", "New scenario")));
    rows.push(
        Row::new(
            Setting::ScenarioName,
            tr("Название", "Name"),
            tr("Например, «работа»", "For example, \"work\""),
            Control::Field(ui.scenario_name.clone()),
        )
        .dim(off),
    );
    let mut capture = Row::new(
        Setting::ScenarioCapture,
        tr("Открытые сейчас окна", "Windows open right now"),
        if ui.scenario_note.is_empty() {
            tr(
                "Программы, места окон, папки и вкладки браузера",
                "Apps, window places, folders and browser tabs",
            )
        } else {
            ui.scenario_note.as_str()
        },
        Control::Button(tr("Запомнить", "Remember")),
    )
    .dim(off);
    capture.warn = ui.scenario_warn;
    rows.push(capture);

    rows.push(Row::caption(tr("Все сценарии", "All scenarios")));
    if ui.scenarios.is_empty() {
        rows.push(Row::info(tr("Сценариев пока нет", "No scenarios yet")));
    }
    for (index, scenario) in ui.scenarios.iter().enumerate() {
        let mut hint = windows_text(scenario.windows.len());
        let screens = scenario.screens_used();
        if screens > 1 {
            hint += &format!(" · {} {screens}", tr("экранов:", "screens:"));
        }
        let open = ui.scenario_open == Some(index);
        rows.push(
            Row::new(
                Setting::ScenarioRow(index),
                &scenario.name,
                &hint,
                Control::Choice(
                    vec![
                        if open {
                            tr("Скрыть", "Hide")
                        } else {
                            tr("Окна", "Windows")
                        },
                        tr("Удалить", "Delete"),
                    ],
                    None,
                ),
            )
            .message(ui),
        );
    }

    rows.push(Row::new(
        Setting::ScenariosFolder,
        tr("Папка со сценариями", "Scenarios folder"),
        tr(
            "Каждый сценарий — файл .txt",
            "Each scenario is a .txt file",
        ),
        Control::Button(tr("Открыть", "Open")),
    ));

    if let Some(scenario) = ui.scenario_open.and_then(|i| ui.scenarios.get(i)) {
        rows.push(Row::caption(&format!(
            "{} «{}»",
            tr("Окна сценария", "Windows of"),
            scenario.name
        )));
        for (w, window) in scenario.windows.iter().enumerate() {
            rows.push(Row::new(
                Setting::ScenarioWindow(w),
                &window.label(),
                &window_hint(window),
                Control::Button(tr("Убрать", "Remove")),
            ));
            if !crate::foreign::is_browser(&window.exe_name()) {
                continue;
            }
            for (u, url) in window.urls.iter().enumerate() {
                rows.push(Row::new(
                    Setting::ScenarioUrl(w, u),
                    url,
                    tr("Вкладка", "Tab"),
                    Control::Button(tr("Убрать", "Remove")),
                ));
            }
            rows.push(Row::new(
                Setting::ScenarioAddUrl(w),
                tr("Добавить вкладку", "Add a tab"),
                tr("Адрес — Enter", "Address, then Enter"),
                Control::Field(String::new()),
            ));
        }
    }
}

#[cfg(feature = "scenarios")]
fn window_hint(window: &crate::scenarios::store::Window) -> String {
    use crate::scenarios::store::State;
    let exe = window.exe_name();
    let mut parts = vec![exe.trim_end_matches(".exe").to_string()];
    match window.state {
        State::Maximized => parts.push(tr("развёрнуто", "maximized").to_string()),
        State::Minimized => parts.push(tr("свёрнуто", "minimized").to_string()),
        State::Normal => {}
    }
    if !window.folder.is_empty() {
        parts.push(window.folder.clone());
    }
    parts.join(" · ")
}

#[cfg(all(feature = "orbit", feature = "scenarios"))]
fn free_scenarios<'a>(ui: &'a Ui, tiles: &[Tile]) -> Vec<&'a str> {
    ui.scenarios
        .iter()
        .map(|s| s.name.as_str())
        .filter(|name| !tiles.contains(&Tile::custom(Kind::Scenario, name)))
        .collect()
}

#[cfg(feature = "scenarios")]
pub fn edit_open_scenario(ui: &mut Ui, edit: impl FnOnce(&mut crate::scenarios::store::Scenario)) {
    use crate::scenarios::store;
    let Some(index) = ui.scenario_open else {
        return;
    };
    let Some(mut scenario) = ui.scenarios.get(index).cloned() else {
        return;
    };
    edit(&mut scenario);
    let _ = store::save(&scenario);
    reload_scenarios(ui);
}

#[cfg(feature = "scenarios")]
pub fn reload_scenarios(ui: &mut Ui) {
    let open = ui
        .scenario_open
        .and_then(|i| ui.scenarios.get(i))
        .map(|s| s.name.clone());
    ui.scenarios = crate::scenarios::store::load_all();
    ui.scenario_open = open.and_then(|name| ui.scenarios.iter().position(|s| s.name == name));
}

fn about(rows: &mut Vec<Row>) {
    rows.push(Row::caption("Moonlet"));
    rows.push(Row {
        setting: None,
        label: tr("Версия", "Version").to_string(),
        hint: String::new(),
        control: Control::Info(env!("CARGO_PKG_VERSION").to_string()),
        dim: false,
        warn: false,
    });
    rows.push(Row::new(
        Setting::OpenFile,
        tr("Файл настроек", "Settings file"),
        tr("config.toml — для опытных", "config.toml, for power users"),
        Control::Button(tr("Открыть", "Open")),
    ));
    rows.push(Row::new(
        Setting::Website,
        tr("Исходный код", "Source code"),
        tr("Лицензия MIT", "MIT license"),
        Control::Button("GitHub"),
    ));
}

#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    Toggle,
    Pick(usize),
    Value(f32),
    Text(String),
    #[cfg(feature = "launcher")]
    Hotkey(Hotkey),
    Move(isize),
    Remove,
}

#[derive(Debug, PartialEq)]
pub struct Write {
    pub section: &'static str,
    pub key: &'static str,
    pub value: String,
}

fn write(section: &'static str, key: &'static str, value: impl Into<String>) -> Write {
    Write {
        section,
        key,
        value: value.into(),
    }
}

fn flag(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

pub fn apply(config: &mut Config, ui: &mut Ui, setting: Setting, change: Change) -> Vec<Write> {
    #[cfg(feature = "orbit")]
    use crate::config::number;
    use Change::*;
    match (setting, change) {
        (Setting::Language, Pick(i)) => {
            let (language, name) = Language::ALL[i.min(2)];
            config.language = language;
            vec![write("general", "language", format!("\"{name}\""))]
        }
        #[cfg(feature = "timers")]
        (Setting::TimerSound, Pick(i)) => {
            ui.custom_sound = i == 1;
            if i == 1 {
                return Vec::new();
            }
            let value = if i == 2 {
                crate::sound::OFF
            } else {
                crate::sound::DROP
            };
            config.timer_sound = value.to_string();
            vec![write("timers", "sound", format!("\"{value}\""))]
        }
        #[cfg(feature = "timers")]
        (Setting::TimerSoundFile, Text(path)) => {
            ui.custom_sound = false;
            config.timer_sound = path;
            vec![write(
                "timers",
                "sound",
                crate::config::quoted(&config.timer_sound),
            )]
        }
        #[cfg(feature = "scenarios")]
        (Setting::ScenariosModule, Toggle) => {
            config.scenarios_enabled = !config.scenarios_enabled;
            vec![write(
                "modules",
                "scenarios",
                flag(config.scenarios_enabled),
            )]
        }
        #[cfg(feature = "scenarios")]
        (Setting::ScenarioName, Text(text)) => {
            ui.scenario_name = text.trim().to_string();
            ui.replace_armed = false;
            Vec::new()
        }
        #[cfg(feature = "scenarios")]
        (Setting::ScenarioAddUrl(w), Text(text)) => {
            let Some(url) = crate::scenarios::capture::normalize_url(&text) else {
                return Vec::new();
            };
            edit_open_scenario(ui, |s| {
                if let Some(window) = s.windows.get_mut(w)
                    && !window.urls.contains(&url)
                {
                    window.urls.push(url);
                }
            });
            Vec::new()
        }
        #[cfg(all(feature = "orbit", feature = "scenarios"))]
        (Setting::AddScenario, Pick(i)) => {
            let Some(name) = free_scenarios(ui, &config.orbit.tiles)
                .get(i)
                .map(|n| n.to_string())
            else {
                return Vec::new();
            };
            add_tile(config, Tile::custom(Kind::Scenario, &name))
        }
        #[cfg(feature = "notes")]
        (Setting::NotesModule, Toggle) => {
            config.notes_enabled = !config.notes_enabled;
            vec![write("modules", "notes", flag(config.notes_enabled))]
        }
        #[cfg(feature = "notes")]
        (Setting::NotesMode, Pick(i)) => {
            let (mode, name) = crate::notes::store::Mode::ALL[i.min(2)];
            config.notes_mode = mode;
            vec![write("notes", "mode", format!("\"{name}\""))]
        }
        (Setting::Autostart, Toggle) => {
            config.autostart = !config.autostart;
            vec![write("general", "autostart", flag(config.autostart))]
        }
        #[cfg(feature = "orbit")]
        (Setting::PauseOnBattery, Toggle) => {
            config.pause_on_battery = !config.pause_on_battery;
            vec![write(
                "general",
                "pause_on_battery",
                flag(config.pause_on_battery),
            )]
        }
        #[cfg(feature = "update")]
        (Setting::UpdateMode, Pick(i)) => {
            let (mode, name) = crate::update::Mode::ALL[i.min(2)];
            config.update = mode;
            vec![write("update", "mode", format!("\"{name}\""))]
        }
        #[cfg(feature = "orbit")]
        (Setting::OrbitModule, Toggle) => {
            config.orbit_enabled = !config.orbit_enabled;
            vec![write("modules", "orbit", flag(config.orbit_enabled))]
        }
        #[cfg(feature = "orbit")]
        (Setting::Shape, Pick(i)) => {
            let (rx, ry) = if i == 0 {
                OrbitConfig::CIRCLE
            } else {
                OrbitConfig::OVAL
            };
            config.orbit.radius_x = rx;
            config.orbit.radius_y = ry;
            vec![
                write("orbit", "radius_x", number(rx)),
                write("orbit", "radius_y", number(ry)),
            ]
        }
        #[cfg(feature = "orbit")]
        (Setting::Rotate, Toggle) => {
            config.orbit.rotate = !config.orbit.rotate;
            vec![write("orbit", "rotate", flag(config.orbit.rotate))]
        }
        #[cfg(feature = "orbit")]
        (Setting::Direction, Pick(i)) => {
            config.orbit.clockwise = i == 0;
            vec![write("orbit", "clockwise", flag(config.orbit.clockwise))]
        }
        #[cfg(feature = "orbit")]
        (Setting::Period, Value(v)) => {
            config.orbit.period_secs = v;
            vec![write("orbit", "period", number(v))]
        }
        #[cfg(feature = "orbit")]
        (Setting::HoverDelay, Value(v)) => {
            config.orbit.hover_delay_ms = v.round() as u32;
            vec![write(
                "orbit",
                "hover_delay",
                config.orbit.hover_delay_ms.to_string(),
            )]
        }
        #[cfg(feature = "orbit")]
        (Setting::Satellite(kind), change) => {
            let list = &mut config.orbit.satellites;
            let at = list.iter().position(|k| *k == kind);
            match (change, at) {
                (Toggle, Some(at)) => {
                    list.remove(at);
                }
                (Toggle, None) => list.push(kind),
                (Move(delta), Some(at)) => {
                    let to = at as isize + delta;
                    if to < 0 || to as usize >= list.len() {
                        return Vec::new();
                    }
                    list.swap(at, to as usize);
                }
                _ => return Vec::new(),
            }
            let names: Vec<String> = list
                .iter()
                .map(|kind| format!("\"{}\"", kind.name()))
                .collect();
            vec![write(
                "orbit",
                "satellites",
                format!("[{}]", names.join(", ")),
            )]
        }
        #[cfg(feature = "orbit")]
        (Setting::Tile(index), Move(delta)) => {
            let tiles = &mut config.orbit.tiles;
            let to = index as isize + delta;
            if index >= tiles.len() || to < 0 || to as usize >= tiles.len() {
                return Vec::new();
            }
            tiles.swap(index, to as usize);
            vec![tiles_write(tiles)]
        }
        #[cfg(feature = "orbit")]
        (Setting::Tile(index), Remove) => {
            let tiles = &mut config.orbit.tiles;
            if index >= tiles.len() {
                return Vec::new();
            }
            tiles.remove(index);
            vec![tiles_write(tiles)]
        }
        #[cfg(feature = "orbit")]
        (Setting::AddKind, Pick(i)) => {
            ui.add_kind = i;
            if add_kinds().get(i) == Some(&Some(Kind::App)) && ui.apps.is_empty() {
                ui.apps = crate::apps::scan();
            }
            Vec::new()
        }
        #[cfg(feature = "orbit")]
        (Setting::AddBuiltin, Pick(i)) => match free_builtins(&config.orbit.tiles).get(i) {
            Some(&builtin) => add_tile(config, Tile::Builtin(builtin)),
            None => Vec::new(),
        },
        #[cfg(feature = "orbit")]
        (Setting::AppQuery, Text(text)) => {
            ui.app_query = text;
            Vec::new()
        }
        #[cfg(feature = "orbit")]
        (Setting::AppPick, Pick(i)) => {
            let Some(app) = app_matches(ui).get(i).copied() else {
                return Vec::new();
            };
            let tile = Tile::Custom {
                kind: Kind::App,
                target: app.path.to_string_lossy().into_owned(),
                name: app.name.clone(),
            };
            ui.app_query.clear();
            add_tile(config, tile)
        }
        #[cfg(feature = "orbit")]
        (Setting::Browse, Text(path)) => match add_kinds().get(ui.add_kind).copied().flatten() {
            Some(kind @ (Kind::App | Kind::File | Kind::Folder)) => {
                add_tile(config, Tile::custom(kind, &path))
            }
            _ => Vec::new(),
        },
        #[cfg(feature = "orbit")]
        (Setting::AddUrl, Text(text)) => {
            let text = text.trim();
            if text.is_empty() {
                return Vec::new();
            }
            let url = if text.contains("://") {
                text.to_string()
            } else {
                format!("https://{text}")
            };
            add_tile(config, Tile::custom(Kind::Url, &url))
        }
        #[cfg(feature = "orbit")]
        (Setting::AddCommand, Text(text)) => add_tile(config, Tile::custom(Kind::Command, &text)),
        #[cfg(feature = "orbit")]
        (Setting::NoticeCharger, Toggle) => {
            let n = &mut config.orbit.notices;
            n.charger = !n.charger;
            vec![write("notices", "charger", flag(n.charger))]
        }
        #[cfg(feature = "orbit")]
        (Setting::NoticeLowBattery, Toggle) => {
            let n = &mut config.orbit.notices;
            n.low_battery = !n.low_battery;
            vec![write("notices", "low_battery", flag(n.low_battery))]
        }
        #[cfg(feature = "orbit")]
        (Setting::NoticeTrack, Toggle) => {
            let n = &mut config.orbit.notices;
            n.track = !n.track;
            vec![write("notices", "track", flag(n.track))]
        }
        #[cfg(feature = "orbit")]
        (Setting::NoticeSeconds, Value(v)) => {
            config.orbit.notices.seconds = v;
            vec![write("notices", "seconds", number(v))]
        }
        #[cfg(feature = "launcher")]
        (Setting::LauncherModule, Toggle) => {
            config.launcher_enabled = !config.launcher_enabled;
            vec![write("modules", "launcher", flag(config.launcher_enabled))]
        }
        #[cfg(feature = "launcher")]
        (Setting::Hotkey, Hotkey(key)) => {
            config.launcher.hotkey = key;
            vec![write("launcher", "hotkey", format!("\"{}\"", key.name()))]
        }
        #[cfg(feature = "launcher")]
        (Setting::Search, Pick(i)) => {
            let Some(&(_, url)) = SEARCH.get(i) else {
                ui.custom_search = true;
                return Vec::new();
            };
            ui.custom_search = false;
            config.launcher.search = url.to_string();
            vec![write("launcher", "search", crate::config::quoted(url))]
        }
        #[cfg(feature = "launcher")]
        (Setting::SearchUrl, Text(text)) => {
            let text = text.trim().to_string();
            let written = crate::config::quoted(&text);
            config.launcher.search = text;
            vec![write("launcher", "search", written)]
        }
        _ => {
            let _ = &ui;
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toml;

    #[test]
    fn every_change_reads_back() {
        let mut config = Config::default();
        let mut ui = Ui::default();
        let mut text = config.to_toml();
        #[allow(unused_mut)]
        let mut steps: Vec<(Setting, Change)> = vec![
            (Setting::Language, Change::Pick(2)),
            (Setting::Autostart, Change::Toggle),
        ];
        #[cfg(feature = "orbit")]
        steps.extend([
            (Setting::PauseOnBattery, Change::Toggle),
            (Setting::OrbitModule, Change::Toggle),
            (Setting::Shape, Change::Pick(0)),
            (Setting::Rotate, Change::Toggle),
            (Setting::Direction, Change::Pick(1)),
            (Setting::Period, Change::Value(65.0)),
            (Setting::HoverDelay, Change::Value(250.0)),
            (Setting::Satellite(SatelliteKind::Music), Change::Toggle),
            (Setting::Satellite(SatelliteKind::Battery), Change::Move(-1)),
            (Setting::NoticeCharger, Change::Toggle),
            (Setting::NoticeLowBattery, Change::Toggle),
            (Setting::NoticeTrack, Change::Toggle),
            (Setting::NoticeSeconds, Change::Value(7.0)),
            (Setting::Tile(2), Change::Move(-1)),
            (Setting::Tile(0), Change::Remove),
            (Setting::AddKind, Change::Pick(4)),
            (Setting::AddUrl, Change::Text("github.com".into())),
            (Setting::AddKind, Change::Pick(3)),
            (
                Setting::Browse,
                Change::Text("C:\\Users\\Me\\Downloads".into()),
            ),
            (Setting::AddKind, Change::Pick(0)),
            (Setting::AddBuiltin, Change::Pick(0)),
        ]);
        #[cfg(feature = "update")]
        steps.push((Setting::UpdateMode, Change::Pick(1)));
        #[cfg(feature = "timers")]
        steps.extend([
            (Setting::TimerSound, Change::Pick(2)),
            (
                Setting::TimerSoundFile,
                Change::Text("C:\\Media\\bell.wav".into()),
            ),
            (Setting::TimerSound, Change::Pick(0)),
        ]);
        #[cfg(feature = "launcher")]
        steps.extend([
            (Setting::LauncherModule, Change::Toggle),
            (
                Setting::Hotkey,
                Change::Hotkey(Hotkey::parse("Ctrl+Shift+K").unwrap()),
            ),
            (Setting::Search, Change::Pick(2)),
            (
                Setting::SearchUrl,
                Change::Text(" https://a.b/?q=\"x\" ".into()),
            ),
        ]);
        for (setting, change) in steps {
            for w in apply(&mut config, &mut ui, setting, change) {
                text = toml::set(&text, w.section, w.key, &w.value);
            }
            assert_eq!(Config::from_toml(&text).unwrap(), config, "{setting:?}");
        }
        assert_ne!(config, Config::default());
    }

    #[cfg(feature = "orbit")]
    #[test]
    fn satellites_keep_their_order() {
        let mut config = Config::default();
        let mut ui = Ui::default();
        let kinds = |c: &Config| c.orbit.satellites.clone();
        apply(
            &mut config,
            &mut ui,
            Setting::Satellite(SatelliteKind::Clock),
            Change::Move(-1),
        );
        assert_eq!(kinds(&config)[0], SatelliteKind::Clock);
        apply(
            &mut config,
            &mut ui,
            Setting::Satellite(SatelliteKind::Clock),
            Change::Move(1),
        );
        assert_eq!(kinds(&config)[1], SatelliteKind::Clock);
        apply(
            &mut config,
            &mut ui,
            Setting::Satellite(SatelliteKind::Music),
            Change::Toggle,
        );
        assert_eq!(
            kinds(&config),
            vec![SatelliteKind::Clock, SatelliteKind::Battery]
        );
        let order = satellite_order(&kinds(&config));
        assert_eq!(order[2], SatelliteKind::Music);
    }

    #[cfg(feature = "orbit")]
    #[test]
    fn hub_tiles_are_added_moved_and_removed() {
        let mut config = Config::default();
        let mut ui = Ui::default();
        let names =
            |c: &Config| -> Vec<String> { c.orbit.tiles.iter().map(Tile::to_text).collect() };
        apply(&mut config, &mut ui, Setting::AddBuiltin, Change::Pick(0));
        assert_eq!(names(&config)[4], "launcher");
        let before = config.orbit.tiles.len();
        apply(&mut config, &mut ui, Setting::AddKind, Change::Pick(4));
        apply(
            &mut config,
            &mut ui,
            Setting::AddUrl,
            Change::Text("x.org".into()),
        );
        apply(
            &mut config,
            &mut ui,
            Setting::AddUrl,
            Change::Text("x.org".into()),
        );
        assert_eq!(config.orbit.tiles.len(), before + 1);
        assert_eq!(names(&config)[before], "url:https://x.org");
        apply(&mut config, &mut ui, Setting::AddKind, Change::Pick(4));
        assert!(
            apply(
                &mut config,
                &mut ui,
                Setting::AddUrl,
                Change::Text("  ".into())
            )
            .is_empty()
        );
        apply(
            &mut config,
            &mut ui,
            Setting::Tile(before),
            Change::Move(-1),
        );
        assert_eq!(names(&config)[before - 1], "url:https://x.org");
        assert!(apply(&mut config, &mut ui, Setting::Tile(0), Change::Move(-1)).is_empty());
        apply(&mut config, &mut ui, Setting::Tile(0), Change::Remove);
        assert_eq!(names(&config)[0], "rotation");
        apply(&mut config, &mut ui, Setting::AddKind, Change::Pick(4));
        for i in 0..20 {
            apply(
                &mut config,
                &mut ui,
                Setting::AddUrl,
                Change::Text(format!("s{i}.org")),
            );
        }
        assert_eq!(config.orbit.tiles.len(), MAX_TILES);
        let rows = rows(Section::Quick, &config, &ui);
        assert!(!rows.iter().any(|row| row.setting == Some(Setting::AddKind)));
    }

    #[cfg(feature = "orbit")]
    #[test]
    fn apps_are_found_by_name() {
        let mut ui = Ui::default();
        let app = |name: &str, path: &str| crate::apps::App {
            name: name.to_string(),
            key: name.to_lowercase(),
            file_key: None,
            path: path.into(),
        };
        ui.apps = vec![app("Telegram", "t.lnk"), app("Блокнот", "n.lnk")];
        ui.app_query = "тел".into();
        assert!(app_matches(&ui).is_empty());
        ui.app_query = "tel".into();
        assert_eq!(app_matches(&ui)[0].name, "Telegram");
        let mut config = Config::default();
        ui.add_kind = 1;
        apply(&mut config, &mut ui, Setting::AppPick, Change::Pick(0));
        assert_eq!(
            config.orbit.tiles.last().unwrap().to_text(),
            "app:t.lnk|Telegram"
        );
        assert!(ui.app_query.is_empty());
    }

    #[cfg(feature = "launcher")]
    #[test]
    fn custom_search_shows_the_field() {
        let mut config = Config::default();
        let mut ui = Ui::default();
        let has_field = |rows: &[Row]| {
            rows.iter()
                .any(|row| row.setting == Some(Setting::SearchUrl))
        };
        assert!(!has_field(&rows(Section::Launcher, &config, &ui)));
        assert!(apply(&mut config, &mut ui, Setting::Search, Change::Pick(5)).is_empty());
        assert!(has_field(&rows(Section::Launcher, &config, &ui)));
        apply(&mut config, &mut ui, Setting::Search, Change::Pick(1));
        assert!(!has_field(&rows(Section::Launcher, &config, &ui)));
    }

    #[test]
    fn news_list_opens_under_the_button() {
        let config = Config::default();
        let mut ui = Ui::default();
        let notes = |ui: &Ui| {
            rows(Section::General, &config, ui)
                .iter()
                .filter(|row| matches!(row.control, Control::Note { .. }))
                .count()
        };
        assert_eq!(notes(&ui), 0);
        ui.news_open = true;
        let count: usize = NEWS.iter().map(|(_, items)| 1 + items.len()).sum();
        assert_eq!(notes(&ui), count);
        assert_eq!(NEWS[0].0, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn slider_snaps_to_steps() {
        let slider = Slider {
            value: 40.0,
            min: 10.0,
            max: 120.0,
            step: 5.0,
            unit: "",
        };
        assert_eq!(slider.snap(42.4), 40.0);
        assert_eq!(slider.snap(43.0), 45.0);
        assert_eq!(slider.snap(500.0), 120.0);
        assert!((slider.fraction() - 30.0 / 110.0).abs() < 1e-6);
    }
}
