//   window  0  normal  100  50  1000  700  C:\…\chrome.exe  GitHub - Google Chrome
//   url     https://github.com
//   folder  C:\Users\…\Documents

use std::path::PathBuf;

pub const MAX_NAME: usize = 40;
pub const MAX_SCENARIOS: usize = 30;

#[derive(Clone, Debug, PartialEq)]
pub struct Screen {
    pub device: String,
    pub work: [i32; 4],
    pub dpi: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Normal,
    Maximized,
    Minimized,
}

impl State {
    fn name(self) -> &'static str {
        match self {
            State::Normal => "normal",
            State::Maximized => "max",
            State::Minimized => "min",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "normal" => Some(State::Normal),
            "max" => Some(State::Maximized),
            "min" => Some(State::Minimized),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    pub screen: usize,
    pub state: State,
    pub rect: [i32; 4],
    pub exe: String,
    pub title: String,
    pub app: String,
    pub folder: String,
    pub urls: Vec<String>,
}

impl Window {
    pub fn exe_name(&self) -> String {
        crate::foreign::file_name(&self.exe)
    }

    #[cfg_attr(not(feature = "settings"), allow(dead_code))]
    pub fn label(&self) -> String {
        let title = self.title.trim();
        if title.is_empty() {
            self.exe_name().trim_end_matches(".exe").to_string()
        } else {
            title.to_string()
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scenario {
    pub name: String,
    pub screens: Vec<Screen>,
    pub windows: Vec<Window>,
}

impl Scenario {
    #[cfg_attr(not(feature = "settings"), allow(dead_code))]
    pub fn screens_used(&self) -> usize {
        let mut used: Vec<usize> = self.windows.iter().map(|w| w.screen).collect();
        used.sort_unstable();
        used.dedup();
        used.len()
    }
}

pub fn clean_name(text: &str) -> Option<String> {
    let name: String = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME)
        .collect();
    let name = name.trim().to_string();
    name.chars().any(char::is_alphanumeric).then_some(name)
}

fn file_stem(name: &str) -> String {
    let mut stem: String = name
        .to_lowercase()
        .chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    while stem.ends_with(['.', ' ']) {
        stem.pop();
    }
    let device = ["con", "prn", "aux", "nul"].contains(&stem.as_str())
        || ((stem.starts_with("com") || stem.starts_with("lpt"))
            && stem.len() == 4
            && stem.ends_with(|c: char| c.is_ascii_digit()));
    if device || stem.is_empty() {
        stem.insert(0, '_');
    }
    stem
}

pub fn dir() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(appdata).join("Moonlet").join("scenarios"))
}

fn path(name: &str) -> Option<PathBuf> {
    Some(dir()?.join(format!("{}.txt", file_stem(name))))
}

fn field(text: &str) -> String {
    text.replace(['\t', '\r', '\n'], " ")
}

pub fn to_text(scenario: &Scenario) -> String {
    let mut text = String::from("# Moonlet scenario\n");
    text += &format!("name\t{}\n", field(&scenario.name));
    for screen in &scenario.screens {
        let [l, t, r, b] = screen.work;
        text += &format!(
            "screen\t{}\t{l}\t{t}\t{r}\t{b}\t{}\n",
            field(&screen.device),
            screen.dpi
        );
    }
    for window in &scenario.windows {
        let [l, t, r, b] = window.rect;
        text += &format!(
            "window\t{}\t{}\t{l}\t{t}\t{r}\t{b}\t{}\t{}\n",
            window.screen,
            window.state.name(),
            field(&window.exe),
            field(&window.title)
        );
        if !window.app.is_empty() {
            text += &format!("app\t{}\n", field(&window.app));
        }
        if !window.folder.is_empty() {
            text += &format!("folder\t{}\n", field(&window.folder));
        }
        for url in &window.urls {
            text += &format!("url\t{}\n", field(url));
        }
    }
    text
}

fn numbers<const N: usize>(parts: &[&str]) -> Option<[i32; N]> {
    let mut out = [0; N];
    for (slot, part) in out.iter_mut().zip(parts) {
        *slot = part.trim().parse().ok()?;
    }
    (parts.len() >= N).then_some(out)
}

pub fn from_text(text: &str) -> Option<Scenario> {
    let mut scenario = Scenario {
        name: String::new(),
        screens: Vec::new(),
        windows: Vec::new(),
    };
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        let mut parts: Vec<&str> = line.split('\t').collect();
        let key = parts.remove(0).trim();
        let rest = parts.join("\t");
        match key {
            "name" => scenario.name = clean_name(&rest).unwrap_or_default(),
            "screen" if parts.len() >= 6 => {
                if let (Some(work), Ok(dpi)) = (numbers::<4>(&parts[1..5]), parts[5].trim().parse())
                {
                    scenario.screens.push(Screen {
                        device: parts[0].trim().to_string(),
                        work,
                        dpi,
                    });
                }
            }
            "window" if parts.len() >= 7 => {
                let (Ok(screen), Some(state), Some(rect)) = (
                    parts[0].trim().parse::<usize>(),
                    State::parse(parts[1].trim()),
                    numbers::<4>(&parts[2..6]),
                ) else {
                    continue;
                };
                let exe = parts[6].trim().to_string();
                if exe.is_empty() {
                    continue;
                }
                scenario.windows.push(Window {
                    screen,
                    state,
                    rect,
                    exe,
                    title: parts.get(7..).map(|p| p.join(" ")).unwrap_or_default(),
                    app: String::new(),
                    folder: String::new(),
                    urls: Vec::new(),
                });
            }
            "app" | "folder" | "url" => {
                let value = rest.trim().to_string();
                let Some(window) = scenario.windows.last_mut() else {
                    continue;
                };
                if value.is_empty() {
                    continue;
                }
                match key {
                    "app" => window.app = value,
                    "folder" => window.folder = value,
                    _ => window.urls.push(value),
                }
            }
            _ => {}
        }
    }
    let screens = scenario.screens.len();
    for window in &mut scenario.windows {
        if window.screen >= screens {
            window.screen = 0;
        }
    }
    (!scenario.name.is_empty()).then_some(scenario)
}

pub fn load_all() -> Vec<Scenario> {
    let Some(dir) = dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut list: Vec<Scenario> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "txt"))
        .filter_map(|e| std::fs::read(e.path()).ok())
        .filter_map(|bytes| from_text(&String::from_utf8_lossy(&bytes)))
        .take(MAX_SCENARIOS)
        .collect();
    list.sort_by_key(|s| s.name.to_lowercase());
    list
}

pub fn find(name: &str) -> Option<Scenario> {
    let name = clean_name(name)?.to_lowercase();
    let text = std::fs::read(path(&name)?).ok()?;
    from_text(&String::from_utf8_lossy(&text)).filter(|s| s.name.to_lowercase() == name)
}

pub fn save(scenario: &Scenario) -> std::io::Result<()> {
    let path = path(&scenario.name).ok_or(std::io::ErrorKind::NotFound)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, to_text(scenario))?;
    std::fs::rename(&temp, &path)
}

#[cfg_attr(not(feature = "settings"), allow(dead_code))]
pub fn delete(name: &str) {
    if let Some(path) = path(name) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Scenario {
        Scenario {
            name: "работа".into(),
            screens: vec![Screen {
                device: "\\\\.\\DISPLAY1".into(),
                work: [0, 0, 1920, 1040],
                dpi: 120,
            }],
            windows: vec![
                Window {
                    screen: 0,
                    state: State::Normal,
                    rect: [10, 20, 960, 1040],
                    exe: "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe".into(),
                    title: "GitHub\t- Google Chrome".into(),
                    app: String::new(),
                    folder: String::new(),
                    urls: vec!["https://github.com".into(), "ya.ru".into()],
                },
                Window {
                    screen: 0,
                    state: State::Maximized,
                    rect: [-5, 0, 800, 600],
                    exe: "C:\\Windows\\explorer.exe".into(),
                    title: "Документы".into(),
                    app: String::new(),
                    folder: "C:\\Users\\Me\\Documents".into(),
                    urls: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn scenario_round_trips() {
        let scenario = sample();
        let text = to_text(&scenario);
        let back = from_text(&text).unwrap();
        assert_eq!(back.windows[0].title, "GitHub - Google Chrome");
        let mut expected = scenario;
        expected.windows[0].title = "GitHub - Google Chrome".into();
        assert_eq!(back, expected);
        assert_eq!(back.windows[1].exe_name(), "explorer.exe");
        assert_eq!(back.screens_used(), 1);
    }

    #[test]
    fn bad_lines_are_skipped() {
        let text = "name\tигра\nwindow\tx\tnormal\t1\t2\t3\t4\tC:\\a.exe\t\n\
                    url\tlost\nwindow\t5\tmax\t1\t2\t3\t4\tC:\\b.exe\tB\nnonsense\n";
        let scenario = from_text(text).unwrap();
        assert_eq!(scenario.windows.len(), 1);
        assert_eq!(scenario.windows[0].screen, 0);
        assert!(scenario.windows[0].urls.is_empty());
        assert!(from_text("window\t0\tnormal\t1\t2\t3\t4\tC:\\a.exe\n").is_none());
    }

    #[test]
    fn names_and_files() {
        assert_eq!(clean_name("  моя   работа ").as_deref(), Some("моя работа"));
        assert_eq!(clean_name(" ... "), None);
        assert_eq!(
            clean_name(&"а".repeat(60)).unwrap().chars().count(),
            MAX_NAME
        );
        assert_eq!(file_stem("Работа: ноут/дом?"), "работа_ ноут_дом_");
        assert_eq!(file_stem("con"), "_con");
        assert_eq!(file_stem("com3"), "_com3");
        assert_eq!(file_stem("игра."), "игра");
    }
}
