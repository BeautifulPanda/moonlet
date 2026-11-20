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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    General,
    #[cfg(feature = "orbit")]
    Orbit,
    #[cfg(feature = "orbit")]
    Satellites,
    #[cfg(feature = "launcher")]
    Launcher,
    About,
}

impl Section {
    pub fn all() -> Vec<Section> {
        vec![
            Section::General,
            #[cfg(feature = "orbit")]
            Section::Orbit,
            #[cfg(feature = "orbit")]
            Section::Satellites,
            #[cfg(feature = "launcher")]
            Section::Launcher,
            Section::About,
        ]
    }

    pub fn title(self) -> &'static str {
        match self {
            Section::General => tr("Общие", "General"),
            #[cfg(feature = "orbit")]
            Section::Orbit => tr("Орбита", "Orbit"),
            #[cfg(feature = "orbit")]
            Section::Satellites => tr("Спутники", "Satellites"),
            #[cfg(feature = "launcher")]
            Section::Launcher => tr("Командная строка", "Command line"),
            Section::About => tr("О программе", "About"),
        }
    }

    pub fn color(self) -> u32 {
        match self {
            Section::General => 0xF6AD55,
            #[cfg(feature = "orbit")]
            Section::Orbit => 0x9FB2FF,
            #[cfg(feature = "orbit")]
            Section::Satellites => 0x4FD1C5,
            #[cfg(feature = "launcher")]
            Section::Launcher => 0xB794F4,
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
}

const NEWS: [(&str, &[(&str, &str)]); 2] = [
    (
        "0.3.1",
        &[(
            "«Что нового» прямо в настройках; клик по уведомлению «Moonlet обновлён» открывает этот список.",
            "\"What's new\" right in the settings; clicking the \"Moonlet updated\" notification opens this list.",
        )],
    ),
    (
        "0.3.0",
        &[
            (
                "Окно настроек вместо Блокнота: язык, автозапуск, обновления, орбита, спутники и их порядок, временные спутники, клавиша и поиск командной строки.",
                "A settings window instead of Notepad: language, start with Windows, updates, the orbit, satellites and their order, temporary satellites, the command line hotkey and search.",
            ),
            (
                "Правки сразу сохраняются в config.toml, комментарии в файле остаются.",
                "Changes are saved to config.toml at once; comments in the file are kept.",
            ),
            (
                "В меню трея — «Открыть config.toml» для ручной правки.",
                "The tray menu has \"Open config.toml\" for editing by hand.",
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
        Section::Satellites => satellites(&mut rows, config),
        #[cfg(feature = "launcher")]
        Section::Launcher => launcher(&mut rows, config, ui),
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
fn satellites(rows: &mut Vec<Row>, config: &Config) {
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
        ]);
        #[cfg(feature = "update")]
        steps.push((Setting::UpdateMode, Change::Pick(1)));
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
