//! Built-in English/Russian UI strings. No catalogs, no files with secrets.
//!
//! The interface language is the only preference GLYPH persists. It lives in
//! `HKCU\Software\GLYPH` as `Language = en|ru`; nothing else is stored there.
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    En,
    Ru,
}

impl Language {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "en" => Some(Self::En),
            "ru" => Some(Self::Ru),
            _ => None,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Ru => "ru",
        }
    }

    pub fn toggled(self) -> Self {
        match self {
            Self::En => Self::Ru,
            Self::Ru => Self::En,
        }
    }
}

/// Russian has been the interface language since the first build; English is
/// one click away and the choice survives restarts.
const DEFAULT: Language = Language::Ru;
const DEFAULT_BYTE: u8 = 1; // 0 = English, 1 = Russian

static LANGUAGE: AtomicU8 = AtomicU8::new(DEFAULT_BYTE);

pub fn set_language(language: Language) {
    LANGUAGE.store(u8::from(language == Language::Ru), Ordering::Relaxed);
}

pub fn language() -> Language {
    #[cfg(test)]
    if let Some(language) = TEST_LANGUAGE.with(|slot| slot.get()) {
        return language;
    }
    if LANGUAGE.load(Ordering::Relaxed) == 0 {
        Language::En
    } else {
        Language::Ru
    }
}

pub fn is_russian() -> bool {
    language() == Language::Ru
}

#[cfg(test)]
thread_local! {
    static TEST_LANGUAGE: std::cell::Cell<Option<Language>> = const { std::cell::Cell::new(None) };
}

/// Test-only override: the global language is process-wide, but each test can
/// render in its own language without racing the others.
#[cfg(test)]
pub(crate) fn with_language<T>(language: Language, action: impl FnOnce() -> T) -> T {
    struct Restore(Option<Language>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TEST_LANGUAGE.with(|slot| slot.set(self.0));
        }
    }
    let _restore = Restore(TEST_LANGUAGE.with(|slot| slot.replace(Some(language))));
    action()
}

#[cfg(windows)]
const REG_PATH: &str = r"Software\GLYPH";

#[cfg(windows)]
fn read_saved() -> Option<Language> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
    let key = winreg::RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(REG_PATH, KEY_READ)
        .ok()?;
    let value: String = key.get_value("Language").ok()?;
    Language::parse(&value)
}

#[cfg(not(windows))]
fn read_saved() -> Option<Language> {
    None
}

pub fn load_saved_language() -> Language {
    read_saved().unwrap_or(DEFAULT)
}

/// Saves the language preference. Returns `false` when Windows rejected the
/// write; the caller reports that instead of pretending it was saved.
pub fn save_language(language: Language) -> bool {
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        let Ok((key, _)) = winreg::RegKey::predef(HKEY_CURRENT_USER).create_subkey(REG_PATH) else {
            return false;
        };
        key.set_value("Language", &language.code()).is_ok()
    }
    #[cfg(not(windows))]
    {
        let _ = language;
        false
    }
}

/// Tests never read the developer's real registry preference.
pub fn initial_language() -> Language {
    #[cfg(test)]
    {
        DEFAULT
    }
    #[cfg(not(test))]
    {
        load_saved_language()
    }
}

#[macro_export]
macro_rules! tr {
    ($en:expr, $ru:expr) => {
        if $crate::i18n::is_russian() { $ru } else { $en }
    };
}

#[macro_export]
macro_rules! tr_format {
    ($en:literal, $ru:literal $(, $args:expr)* $(,)?) => {
        if $crate::i18n::is_russian() {
            format!($ru $(, $args)*)
        } else {
            format!($en $(, $args)*)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_codes_round_trip() {
        for language in [Language::En, Language::Ru] {
            assert_eq!(Language::parse(language.code()), Some(language));
            assert_eq!(language.toggled().toggled(), language);
            assert_ne!(language.toggled(), language);
        }
        assert_eq!(Language::parse("de"), None);
    }

    #[test]
    fn both_languages_select_their_own_text() {
        with_language(Language::En, || {
            assert_eq!(crate::tr!("Create", "Создать"), "Create");
            assert_eq!(crate::tr_format!("{} bits", "{} бит", 192), "192 bits");
        });
        with_language(Language::Ru, || {
            assert_eq!(crate::tr!("Create", "Создать"), "Создать");
            assert_eq!(crate::tr_format!("{} bits", "{} бит", 192), "192 бит");
        });
    }

    #[test]
    fn the_default_is_russian() {
        assert_eq!(DEFAULT, Language::Ru);
        assert_eq!(DEFAULT_BYTE, 1);
    }
}
