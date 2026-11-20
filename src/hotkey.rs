use windows::Win32::UI::Input::KeyboardAndMouse::{MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hotkey {
    pub modifiers: u32,
    pub key: u32,
}

const MODIFIERS: [(&str, u32); 5] = [
    ("Ctrl", MOD_CONTROL.0),
    ("Control", MOD_CONTROL.0),
    ("Alt", MOD_ALT.0),
    ("Shift", MOD_SHIFT.0),
    ("Win", MOD_WIN.0),
];

const NAMED: [(&str, u32); 8] = [
    ("Space", 0x20),
    ("Enter", 0x0D),
    ("Tab", 0x09),
    ("Insert", 0x2D),
    ("Home", 0x24),
    ("End", 0x23),
    ("PageUp", 0x21),
    ("PageDown", 0x22),
];

impl Hotkey {
    pub const DEFAULT: Hotkey = Hotkey {
        modifiers: MOD_CONTROL.0 | MOD_ALT.0,
        key: 0x20,
    };

    pub fn parse(text: &str) -> Option<Self> {
        let mut modifiers = 0;
        let mut key = None;
        for part in text.split('+').map(str::trim) {
            if let Some(&(_, flag)) = MODIFIERS
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(part))
            {
                modifiers |= flag;
            } else if key.is_none() {
                key = Some(key_code(part)?);
            } else {
                return None;
            }
        }
        (modifiers != 0).then_some(Self {
            modifiers,
            key: key?,
        })
    }

    pub fn name(self) -> String {
        let mut parts: Vec<String> = Vec::new();
        for (name, flag) in [
            ("Ctrl", MOD_CONTROL.0),
            ("Alt", MOD_ALT.0),
            ("Shift", MOD_SHIFT.0),
            ("Win", MOD_WIN.0),
        ] {
            if self.modifiers & flag != 0 {
                parts.push(name.to_string());
            }
        }
        parts.push(key_name(self.key));
        parts.join("+")
    }
}

fn key_code(name: &str) -> Option<u32> {
    if let Some(&(_, code)) = NAMED.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)) {
        return Some(code);
    }
    let upper = name.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    if bytes.len() == 1 && bytes[0].is_ascii_alphanumeric() {
        return Some(bytes[0] as u32);
    }
    let number: u32 = upper.strip_prefix('F')?.parse().ok()?;
    (1..=24).contains(&number).then_some(0x6F + number)
}

fn key_name(code: u32) -> String {
    if let Some((name, _)) = NAMED.iter().find(|(_, c)| *c == code) {
        return name.to_string();
    }
    match code {
        0x30..=0x39 | 0x41..=0x5A => char::from(code as u8).to_string(),
        0x70..=0x87 => format!("F{}", code - 0x6F),
        _ => format!("{code}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hotkeys() {
        assert_eq!(Hotkey::parse("Ctrl+Alt+Space"), Some(Hotkey::DEFAULT));
        assert_eq!(Hotkey::parse(" alt + ctrl + space "), Some(Hotkey::DEFAULT));
        let win_k = Hotkey::parse("Win+Shift+K").unwrap();
        assert_eq!(win_k.key, 0x4B);
        assert_eq!(win_k.name(), "Shift+Win+K");
        assert_eq!(Hotkey::parse("Ctrl+F12").unwrap().key, 0x7B);
        assert_eq!(Hotkey::parse("Ctrl+F12").unwrap().name(), "Ctrl+F12");
        assert_eq!(Hotkey::parse("Space"), None);
        assert_eq!(Hotkey::parse("Ctrl+A+B"), None);
        assert_eq!(Hotkey::parse("Ctrl+Banana"), None);
        assert_eq!(Hotkey::parse("Ctrl+F25"), None);
    }

    #[test]
    fn name_round_trips() {
        assert_eq!(Hotkey::DEFAULT.name(), "Ctrl+Alt+Space");
        let key = Hotkey::parse("Ctrl+Shift+7").unwrap();
        assert_eq!(Hotkey::parse(&key.name()), Some(key));
    }
}
