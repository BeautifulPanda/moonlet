// Interface language: Russian or English, following Windows unless
// config.toml says otherwise.

use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Globalization::GetUserDefaultUILanguage;

static ENGLISH: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Auto,
    Russian,
    English,
}

impl Language {
    pub const ALL: [(Language, &'static str); 3] = [
        (Language::Auto, "auto"),
        (Language::Russian, "ru"),
        (Language::English, "en"),
    ];

    pub fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(lang, _)| *lang == self)
            .map_or("auto", |(_, name)| name)
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .find(|(_, n)| *n == name)
            .map(|(lang, _)| *lang)
    }
}

pub fn set(language: Language) {
    let english = match language {
        Language::Auto => !system_is_russian(),
        Language::Russian => false,
        Language::English => true,
    };
    ENGLISH.store(english, Ordering::Relaxed);
}

pub fn is_english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

pub fn tr(ru: &'static str, en: &'static str) -> &'static str {
    if is_english() { en } else { ru }
}

fn system_is_russian() -> bool {
    let primary = unsafe { GetUserDefaultUILanguage() } & 0x3FF;
    matches!(primary, 0x19 | 0x23 | 0x3F | 0x40)
}
