pub(crate) struct Encoded {
    pub(crate) bytes: Vec<u8>,

    pub(crate) unrepresentable: usize,

    pub(crate) first_unrepresentable: Option<char>,
}

const REPLACEMENT: u8 = b'?';

const HIGH_BLOCK: [char; 32] = [
    '\u{20AC}', '\u{FFFD}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{FFFD}', '\u{017D}', '\u{FFFD}',
    '\u{FFFD}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{FFFD}', '\u{017E}', '\u{0178}',
];

use crate::contracts::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Encoding {
    Utf8,

    Latin1,

    WindowsAnsi,
}

impl Encoding {
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "utf-8" | "utf8" => Some(Self::Utf8),
            "iso-8859-1" | "iso8859-1" | "latin-1" | "latin1" | "l1" => Some(Self::Latin1),
            "windows-1252" | "cp1252" | "cp-1252" => Some(Self::WindowsAnsi),
            _ => None,
        }
    }

    pub fn decode(self, bytes: Vec<u8>, origin: &str) -> Result<String, Error> {
        match self {
            Self::Utf8 => String::from_utf8(bytes).map_err(|error| Error::NotUtf8 {
                origin: origin.to_owned(),
                position: error.utf8_error().valid_up_to(),
            }),
            Self::Latin1 => Ok(bytes.into_iter().map(char::from).collect()),
            Self::WindowsAnsi => Ok(bytes
                .into_iter()
                .map(|byte| match byte.checked_sub(0x80) {
                    Some(index) if usize::from(index) < HIGH_BLOCK.len() => HIGH_BLOCK
                        .get(usize::from(index))
                        .copied()
                        .unwrap_or(char::from(byte)),
                    _ => char::from(byte),
                })
                .collect()),
        }
    }
}

pub(crate) fn to_winansi(text: &str) -> Encoded {
    let mut bytes = Vec::with_capacity(text.len());
    let mut unrepresentable = 0_usize;
    let mut first_unrepresentable = None;

    for character in text.chars() {
        if let Some(byte) = encode_char(character) {
            bytes.push(byte);
        } else {
            bytes.push(REPLACEMENT);
            unrepresentable = unrepresentable.saturating_add(1);
            first_unrepresentable.get_or_insert(character);
        }
    }

    Encoded {
        bytes,
        unrepresentable,
        first_unrepresentable,
    }
}

fn encode_char(character: char) -> Option<u8> {
    if matches!(character, '\n' | '\r' | '\t') {
        return Some(b' ');
    }

    let code = u32::from(character);
    if (0x20..=0x7E).contains(&code) || (0xA0..=0xFF).contains(&code) {
        return u8::try_from(code).ok();
    }

    HIGH_BLOCK
        .iter()
        .position(|&candidate| candidate == character && candidate != '\u{FFFD}')
        .and_then(|index| u8::try_from(index + 0x80).ok())
}
