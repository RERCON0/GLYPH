use std::fmt;

use base64::{
    Engine,
    engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
};
use zeroize::{Zeroize, Zeroizing};

pub const MAX_PASSWORD_LENGTH: usize = 256;
pub const MAX_TOKEN_BYTES: usize = 1024;
const LETTERS: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &str = "0123456789";
const SPECIAL: &str = "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~";
const SIMILAR: &str = "0OoIl1";
const SHELL_UNSAFE: &str = "\"'\\$`!";
const ZERO_WIDTH: [char; 3] = ['\u{200b}', '\u{200c}', '\u{200d}'];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Password,
    Pin,
    Hex,
    Url,
    Fernet,
}

impl Mode {
    pub const ALL: [Self; 5] = [
        Self::Password,
        Self::Pin,
        Self::Hex,
        Self::Url,
        Self::Fernet,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Password => "Пароль",
            Self::Pin => "PIN",
            Self::Hex => "HEX",
            Self::Url => "URL-safe",
            Self::Fernet => "Fernet",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasswordOptions {
    pub length: usize,
    pub min_digits: usize,
    pub special: bool,
    pub exclude_similar: bool,
    pub hidden_count: usize,
    pub argon2_safe: bool,
}

impl Default for PasswordOptions {
    fn default() -> Self {
        Self {
            length: 32,
            min_digits: 5,
            special: true,
            exclude_similar: false,
            hidden_count: 0,
            argon2_safe: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerateError {
    InvalidLength,
    InvalidDigits,
    InvalidHiddenCount,
    InvalidTokenSize,
    RandomUnavailable,
}

impl fmt::Display for GenerateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidLength => "Длина пароля должна быть от 1 до 256 символов.",
            Self::InvalidDigits => "Минимум цифр не может превышать длину пароля.",
            Self::InvalidHiddenCount => "Допустимо от 0 до 256 скрытых символов.",
            Self::InvalidTokenSize => "Размер токена должен быть от 1 до 1024 байт.",
            Self::RandomUnavailable => {
                "Системный источник случайности недоступен. Секрет не создан."
            }
        })
    }
}
impl std::error::Error for GenerateError {}

/// Fallible even during shuffle/insertion. No PRNG fallback on OS errors.
pub trait RandomSource {
    fn fill(&mut self, bytes: &mut [u8]) -> Result<(), GenerateError>;
}
pub struct SystemRandom;
impl RandomSource for SystemRandom {
    fn fill(&mut self, bytes: &mut [u8]) -> Result<(), GenerateError> {
        getrandom::fill(bytes).map_err(|_| GenerateError::RandomUnavailable)
    }
}

fn index(random: &mut impl RandomSource, upper: usize) -> Result<usize, GenerateError> {
    debug_assert!(upper > 0 && upper <= u32::MAX as usize);
    if upper == 1 {
        return Ok(0);
    }
    // The accepted range is an exact multiple of upper, including when upper
    // divides 2^32. A bounded rejection loop fails closed on a broken RNG.
    let bound = upper as u64;
    let limit = (1_u64 << 32) / bound * bound;
    let mut bytes = Zeroizing::new([0_u8; 4]);
    for _ in 0..128 {
        random.fill(bytes.as_mut())?;
        let value = u32::from_le_bytes(*bytes) as u64;
        if value < limit {
            return Ok((value % bound) as usize);
        }
    }
    Err(GenerateError::RandomUnavailable)
}

fn shuffle<T>(values: &mut [T], random: &mut impl RandomSource) -> Result<(), GenerateError> {
    for i in (1..values.len()).rev() {
        let j = index(random, i + 1)?;
        values.swap(i, j);
    }
    Ok(())
}

/// Non-secret generation parameters. Volatile session values use separate,
/// zeroizing secret buffers and must never be formatted as debug metadata.
#[derive(Clone, Debug)]
pub struct Metadata {
    pub mode: Mode,
    pub visible_length: usize,
    pub hidden_count: usize,
    pub source_bytes: Option<usize>,
    pub entropy_bits: f64,
}

pub struct Generated {
    pub secret: Zeroizing<String>,
    pub metadata: Metadata,
}

impl Generated {
    pub fn representation(&self) -> Zeroizing<String> {
        // Reserve before copying secrets: reallocation would otherwise leave
        // unwiped prefixes in allocations no longer owned by Zeroizing.
        let bytes = self
            .secret
            .chars()
            .map(|ch| {
                if ZERO_WIDTH.contains(&ch) {
                    6
                } else {
                    ch.len_utf8()
                }
            })
            .sum();
        let mut result = Zeroizing::new(String::with_capacity(bytes));
        for ch in self.secret.chars() {
            match ch {
                '\u{200b}' => result.push_str("\\u200B"),
                '\u{200c}' => result.push_str("\\u200C"),
                '\u{200d}' => result.push_str("\\u200D"),
                _ => result.push(ch),
            }
        }
        result
    }
}

pub fn password(options: &PasswordOptions) -> Result<Generated, GenerateError> {
    password_with(options, false, &mut SystemRandom)
}

pub fn pin(length: usize, exclude_similar: bool) -> Result<Generated, GenerateError> {
    password_with(
        &PasswordOptions {
            length,
            min_digits: length,
            special: false,
            exclude_similar,
            hidden_count: 0,
            argon2_safe: false,
        },
        true,
        &mut SystemRandom,
    )
}

fn password_with(
    options: &PasswordOptions,
    pin: bool,
    random: &mut impl RandomSource,
) -> Result<Generated, GenerateError> {
    if !(1..=MAX_PASSWORD_LENGTH).contains(&options.length) {
        return Err(GenerateError::InvalidLength);
    }
    if options.min_digits > options.length {
        return Err(GenerateError::InvalidDigits);
    }
    if options.hidden_count > MAX_PASSWORD_LENGTH {
        return Err(GenerateError::InvalidHiddenCount);
    }
    let allowed = |ch: &char| {
        (!options.exclude_similar || !SIMILAR.contains(*ch))
            && (!options.argon2_safe || !SHELL_UNSAFE.contains(*ch))
    };
    let digits: Vec<char> = DIGITS.chars().filter(allowed).collect();
    let specials: Vec<char> = SPECIAL.chars().filter(allowed).collect();
    let charset: Vec<char> = LETTERS
        .chars()
        .chain(DIGITS.chars())
        .chain(if options.special { SPECIAL } else { "" }.chars())
        .filter(allowed)
        .collect();
    let mut visible = Zeroizing::new(Vec::<char>::with_capacity(options.length));
    let required_digits = if pin {
        options.length
    } else {
        options.min_digits
    };
    for _ in 0..required_digits {
        visible.push(digits[index(random, digits.len())?]);
    }
    let has_forced_special = !pin && options.special && visible.len() < options.length;
    if has_forced_special {
        visible.push(specials[index(random, specials.len())?]);
    }
    let remaining = options.length - visible.len();
    for _ in 0..remaining {
        visible.push(charset[index(random, charset.len())?]);
    }
    // A lower bound on min-entropy of the ordered independent draws. Shuffling
    // is a mixture of bijections, so it cannot lower this bound. Do not count
    // zero-width chars or pretend constrained draws are full-alphabet draws.
    let entropy_bits = required_digits as f64 * (digits.len() as f64).log2()
        + if has_forced_special {
            (specials.len() as f64).log2()
        } else {
            0.0
        }
        + remaining as f64 * (charset.len() as f64).log2();
    shuffle(&mut visible, random)?;

    let mut positions = Vec::new();
    if !pin && !options.argon2_safe && options.hidden_count > 0 {
        let start = visible
            .iter()
            .position(|ch| ch.is_ascii_alphanumeric())
            .unwrap_or(0);
        let end = visible
            .iter()
            .rposition(|ch| ch.is_ascii_alphanumeric())
            .unwrap_or(0);
        if start + 1 < end {
            positions.extend(start + 1..end);
        }
        shuffle(&mut positions, random)?;
        positions.truncate(options.hidden_count);
        positions.sort_unstable();
    }
    let mut secret = Zeroizing::new(String::with_capacity(options.length + positions.len() * 3));
    let mut selected = positions.iter().peekable();
    for (i, ch) in visible.iter().enumerate() {
        secret.push(*ch);
        if selected.peek() == Some(&&i) {
            secret.push(ZERO_WIDTH[index(random, ZERO_WIDTH.len())?]);
            selected.next();
        }
    }
    Ok(Generated {
        secret,
        metadata: Metadata {
            mode: if pin { Mode::Pin } else { Mode::Password },
            visible_length: options.length,
            hidden_count: positions.len(),
            source_bytes: None,
            entropy_bits,
        },
    })
}

pub fn token(mode: Mode, bytes: usize) -> Result<Generated, GenerateError> {
    token_with(mode, bytes, &mut SystemRandom)
}

fn token_with(
    mode: Mode,
    bytes: usize,
    random: &mut impl RandomSource,
) -> Result<Generated, GenerateError> {
    let bytes = if mode == Mode::Fernet { 32 } else { bytes };
    if !(1..=MAX_TOKEN_BYTES).contains(&bytes) {
        return Err(GenerateError::InvalidTokenSize);
    }
    let mut raw = Zeroizing::new(vec![0_u8; bytes]);
    random.fill(&mut raw)?;
    let mut secret = Zeroizing::new(String::with_capacity(bytes * 2));
    match mode {
        Mode::Hex => {
            const HEX: &[u8] = b"0123456789abcdef";
            for byte in raw.iter() {
                secret.push(HEX[(byte >> 4) as usize] as char);
                secret.push(HEX[(byte & 15) as usize] as char);
            }
        }
        Mode::Url => URL_SAFE_NO_PAD.encode_string(&raw, &mut secret),
        Mode::Fernet => URL_SAFE.encode_string(&raw, &mut secret),
        _ => return Err(GenerateError::InvalidTokenSize),
    }
    // Wipe the source before handing the encoded secret to the UI.
    raw.zeroize();
    let visible_length = secret.len();
    Ok(Generated {
        secret,
        metadata: Metadata {
            mode,
            visible_length,
            hidden_count: 0,
            source_bytes: Some(bytes),
            entropy_bits: bytes as f64 * 8.0,
        },
    })
}

pub fn strength(bits: f64) -> &'static str {
    match bits {
        x if x < 40.0 => "Очень слабый",
        x if x < 60.0 => "Слабый",
        x if x < 80.0 => "Средний",
        x if x < 100.0 => "Хороший",
        x if x < 120.0 => "Сильный",
        _ => "Очень сильный",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Counter {
        state: u64,
        calls: usize,
        fail_at: usize,
    }
    impl Counter {
        fn new() -> Self {
            Self {
                state: 42,
                calls: 0,
                fail_at: usize::MAX,
            }
        }
    }
    impl RandomSource for Counter {
        fn fill(&mut self, bytes: &mut [u8]) -> Result<(), GenerateError> {
            if self.calls == self.fail_at {
                return Err(GenerateError::RandomUnavailable);
            }
            self.calls += 1;
            for byte in bytes {
                self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
                *byte = (self.state >> 32) as u8;
            }
            Ok(())
        }
    }

    #[test]
    fn all_password_options_preserve_invariants() {
        for length in [1, 2, 3, 4, 16, 32, 64, 256] {
            for min_digits in [0, length / 2, length] {
                for special in [false, true] {
                    for exclude_similar in [false, true] {
                        for argon2_safe in [false, true] {
                            let options = PasswordOptions {
                                length,
                                min_digits,
                                special,
                                exclude_similar,
                                hidden_count: 6,
                                argon2_safe,
                            };
                            let output =
                                password_with(&options, false, &mut Counter::new()).unwrap();
                            let visible: String = output
                                .secret
                                .chars()
                                .filter(|ch| !ZERO_WIDTH.contains(ch))
                                .collect();
                            assert_eq!(visible.len(), length);
                            assert!(
                                visible.chars().filter(char::is_ascii_digit).count() >= min_digits
                            );
                            if special && min_digits < length {
                                assert!(visible.chars().any(|ch| SPECIAL.contains(ch)));
                            }
                            if !special {
                                assert!(visible.chars().all(|ch| ch.is_ascii_alphanumeric()));
                            }
                            if exclude_similar {
                                assert!(!visible.chars().any(|ch| SIMILAR.contains(ch)));
                            }
                            if argon2_safe {
                                assert!(!output.secret.chars().any(
                                    |ch| SHELL_UNSAFE.contains(ch) || ZERO_WIDTH.contains(&ch)
                                ));
                            }
                            assert_eq!(
                                output.secret.chars().count(),
                                length + output.metadata.hidden_count
                            );
                            assert!(output.metadata.entropy_bits.is_finite());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn random_failures_at_every_generation_stage_fail_closed() {
        let options = PasswordOptions {
            hidden_count: 6,
            ..Default::default()
        };
        let mut random = Counter::new();
        password_with(&options, false, &mut random).unwrap();
        for fail_at in 0..random.calls {
            let mut random = Counter {
                fail_at,
                ..Counter::new()
            };
            assert!(matches!(
                password_with(&options, false, &mut random),
                Err(GenerateError::RandomUnavailable)
            ));
        }
        for mode in [Mode::Hex, Mode::Url, Mode::Fernet] {
            let mut random = Counter {
                fail_at: 0,
                ..Counter::new()
            };
            assert!(matches!(
                token_with(mode, 32, &mut random),
                Err(GenerateError::RandomUnavailable)
            ));
        }
    }

    #[test]
    fn pin_is_only_digits_even_if_password_options_have_hidden_chars() {
        let options = PasswordOptions {
            length: 4,
            min_digits: 0,
            hidden_count: 256,
            ..Default::default()
        };
        let output = password_with(&options, true, &mut Counter::new()).unwrap();
        assert_eq!(output.secret.len(), 4);
        assert!(output.secret.chars().all(|ch| ch.is_ascii_digit()));
        assert_eq!(output.metadata.hidden_count, 0);
        assert!((output.metadata.entropy_bits - 4.0 * 10_f64.log2()).abs() < 1e-9);
    }

    #[test]
    fn invalid_settings_are_rejected_not_silently_truncated() {
        for length in [0, 257, usize::MAX] {
            assert!(matches!(
                password(&PasswordOptions {
                    length,
                    ..Default::default()
                }),
                Err(GenerateError::InvalidLength)
            ));
        }
        assert!(matches!(
            password(&PasswordOptions {
                length: 4,
                min_digits: 5,
                ..Default::default()
            }),
            Err(GenerateError::InvalidDigits)
        ));
        assert!(matches!(
            password(&PasswordOptions {
                hidden_count: 257,
                ..Default::default()
            }),
            Err(GenerateError::InvalidHiddenCount)
        ));
        for bytes in [0, 1025, usize::MAX] {
            assert!(token(Mode::Hex, bytes).is_err());
        }
    }

    #[test]
    fn hidden_positions_and_representation_are_consistent() {
        let output = password_with(
            &PasswordOptions {
                length: 256,
                hidden_count: 256,
                ..Default::default()
            },
            false,
            &mut Counter::new(),
        )
        .unwrap();
        assert!(output.metadata.hidden_count > 0 && output.metadata.hidden_count <= 254);
        assert!(!ZERO_WIDTH.contains(&output.secret.chars().next().unwrap()));
        assert!(!ZERO_WIDTH.contains(&output.secret.chars().last().unwrap()));
        let representation = output.representation();
        assert!(!representation.chars().any(|ch| ZERO_WIDTH.contains(&ch)));
        assert_eq!(
            representation.matches("\\u200").count(),
            output.metadata.hidden_count
        );
        assert_eq!(
            representation
                .replace("\\u200B", "\u{200b}")
                .replace("\\u200C", "\u{200c}")
                .replace("\\u200D", "\u{200d}"),
            *output.secret
        );
    }

    #[test]
    fn token_encodings_round_trip_for_every_supported_size() {
        for size in 1..=1024 {
            for mode in [Mode::Hex, Mode::Url] {
                let output = token_with(mode, size, &mut Counter::new()).unwrap();
                assert_eq!(output.metadata.entropy_bits, (size * 8) as f64);
                if mode == Mode::Hex {
                    assert_eq!(output.secret.len(), size * 2);
                    assert!(
                        output
                            .secret
                            .bytes()
                            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
                    );
                } else {
                    assert_eq!(output.secret.len(), (size * 8).div_ceil(6));
                    assert_eq!(
                        URL_SAFE_NO_PAD
                            .decode(output.secret.as_bytes())
                            .unwrap()
                            .len(),
                        size
                    );
                    assert!(!output.secret.contains('='));
                }
            }
        }
    }

    #[test]
    fn known_bytes_encode_exactly() {
        struct Fixed;
        impl RandomSource for Fixed {
            fn fill(&mut self, bytes: &mut [u8]) -> Result<(), GenerateError> {
                for (i, byte) in bytes.iter_mut().enumerate() {
                    *byte = i as u8;
                }
                Ok(())
            }
        }
        assert_eq!(
            &*token_with(Mode::Hex, 4, &mut Fixed).unwrap().secret,
            "00010203"
        );
        assert_eq!(
            &*token_with(Mode::Url, 4, &mut Fixed).unwrap().secret,
            "AAECAw"
        );
        let fernet = token_with(Mode::Fernet, 0, &mut Fixed).unwrap();
        assert_eq!(
            &*fernet.secret,
            "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8="
        );
        assert_eq!(URL_SAFE.decode(fernet.secret.as_bytes()).unwrap().len(), 32);
    }

    #[test]
    fn os_rng_smoke_test() {
        assert_eq!(
            password(&PasswordOptions::default())
                .unwrap()
                .metadata
                .visible_length,
            32
        );
        assert_eq!(pin(6, true).unwrap().secret.len(), 6);
        assert_eq!(token(Mode::Fernet, 32).unwrap().secret.len(), 44);
    }

    #[test]
    fn rejection_sampling_handles_top_of_range_without_bias() {
        struct Top;
        impl RandomSource for Top {
            fn fill(&mut self, bytes: &mut [u8]) -> Result<(), GenerateError> {
                bytes.fill(255);
                Ok(())
            }
        }
        assert_eq!(index(&mut Top, 256), Ok(255));
        assert_eq!(index(&mut Top, 3), Err(GenerateError::RandomUnavailable));
    }
}
