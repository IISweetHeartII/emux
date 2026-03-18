//! Character set definitions (G0, G1, DEC special graphics, etc.)

/// Supported character sets.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Charset {
    /// ASCII (default).
    #[default]
    Ascii,
    /// DEC Special Graphics (line drawing).
    DecSpecialGraphics,
    /// UK character set.
    Uk,
}

impl Charset {
    /// Map a byte through this character set.
    pub fn map(self, byte: u8) -> char {
        match self {
            Charset::Ascii | Charset::Uk => byte as char,
            Charset::DecSpecialGraphics => dec_special_graphics(byte),
        }
    }
}

fn dec_special_graphics(byte: u8) -> char {
    match byte {
        0x6a => '\u{2518}', // ┘
        0x6b => '\u{2510}', // ┐
        0x6c => '\u{250c}', // ┌
        0x6d => '\u{2514}', // └
        0x6e => '\u{253c}', // ┼
        0x71 => '\u{2500}', // ─
        0x74 => '\u{251c}', // ├
        0x75 => '\u{2524}', // ┤
        0x76 => '\u{2534}', // ┴
        0x77 => '\u{252c}', // ┬
        0x78 => '\u{2502}', // │
        0x61 => '\u{2592}', // ▒
        _ => byte as char,
    }
}

