/// Determine the display width of a character.
/// Returns 2 for CJK wide characters, 1 for most others, 0 for combining.
pub fn char_width(c: char) -> u8 {
    let cp = c as u32;
    // Fast path: ASCII printable (most common case)
    if cp < 0x7F {
        return if cp >= 0x20 { 1 } else { 0 };
    }

    // Zero-width characters
    if cp == 0x200B  // Zero-Width Space
        || cp == 0x200C  // Zero-Width Non-Joiner
        || cp == 0x200D  // Zero-Width Joiner
        || cp == 0xFEFF  // BOM / Zero-Width No-Break Space
        || cp == 0x2060  // Word Joiner
        || cp == 0x2061  // Function Application
        || cp == 0x2062  // Invisible Times
        || cp == 0x2063  // Invisible Separator
        || cp == 0x2064  // Invisible Plus
        || cp == 0x180E
    // Mongolian Vowel Separator
    {
        return 0;
    }

    // Combining characters (partial list of common ranges)
    if (0x0300..=0x036F).contains(&cp)   // Combining Diacritical Marks
        || (0x1AB0..=0x1AFF).contains(&cp) // Combining Diacritical Marks Extended
        || (0x1DC0..=0x1DFF).contains(&cp) // Combining Diacritical Marks Supplement
        || (0x20D0..=0x20FF).contains(&cp) // Combining Diacritical Marks for Symbols
        || (0xFE20..=0xFE2F).contains(&cp)
    // Combining Half Marks
    {
        return 0;
    }

    // Wide characters
    if (0x1100..=0x115F).contains(&cp)    // Hangul Jamo
        || (0x2329..=0x232A).contains(&cp) // CJK angle brackets
        || (0x2E80..=0x303E).contains(&cp) // CJK misc
        || (0x3040..=0x33BF).contains(&cp) // Hiragana, Katakana, Bopomofo, etc.
        || (0x3400..=0x4DBF).contains(&cp) // CJK Unified Ideographs Extension A
        || (0x4E00..=0x9FFF).contains(&cp) // CJK Unified Ideographs
        || (0xA000..=0xA4CF).contains(&cp) // Yi
        || (0xAC00..=0xD7A3).contains(&cp) // Hangul Syllables
        || (0xF900..=0xFAFF).contains(&cp) // CJK Compatibility Ideographs
        || (0xFE10..=0xFE19).contains(&cp) // Vertical forms
        || (0xFE30..=0xFE6F).contains(&cp) // CJK Compatibility Forms
        || (0xFF01..=0xFF60).contains(&cp) // Fullwidth Forms
        || (0xFFE0..=0xFFE6).contains(&cp) // Fullwidth Signs
        || (0x1F000..=0x1F9FF).contains(&cp) // Various emoji/symbols
        || (0x20000..=0x2FA1F).contains(&cp) // CJK Extension B and beyond
        || (0x30000..=0x3134F).contains(&cp)
    // CJK Extension G
    {
        return 2;
    }

    // Control characters
    if cp < 0x20 || (0x7F..=0x9F).contains(&cp) {
        return 0;
    }

    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_printable() {
        assert_eq!(char_width('A'), 1);
        assert_eq!(char_width(' '), 1);
        assert_eq!(char_width('~'), 1);
    }

    #[test]
    fn control_chars() {
        assert_eq!(char_width('\0'), 0);
        assert_eq!(char_width('\x1b'), 0);
        assert_eq!(char_width('\x7f'), 0);
    }

    #[test]
    fn cjk_wide() {
        assert_eq!(char_width('\u{FF10}'), 2); // fullwidth digit zero
        assert_eq!(char_width('\u{4E00}'), 2); // CJK unified ideograph
        assert_eq!(char_width('\u{AC00}'), 2); // Hangul syllable
    }

    #[test]
    fn combining_zero_width() {
        assert_eq!(char_width('\u{0300}'), 0); // combining grave accent
        assert_eq!(char_width('\u{200B}'), 0); // zero-width space
        assert_eq!(char_width('\u{200D}'), 0); // zero-width joiner
    }
}
