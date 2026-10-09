//! How many terminal columns a character takes: two for the East Asian wide
//! and fullwidth blocks and the emoji planes, one for everything else.
//!
//! A table of the ranges that matter rather than a dependency: the question a
//! host asks is "does this need two cells to be shown", and the answer for the
//! text of real pages is these blocks.

const WIDE: &[(u32, u32)] = &[
    (0x1100, 0x115F),   // Hangul Jamo
    (0x231A, 0x231B),   // watch, hourglass
    (0x2E80, 0x303E),   // CJK radicals, symbols and punctuation
    (0x3041, 0x33FF),   // Hiragana, Katakana, Bopomofo, CJK compatibility
    (0x3400, 0x4DBF),   // CJK extension A
    (0x4E00, 0x9FFF),   // CJK unified ideographs
    (0xA000, 0xA4CF),   // Yi
    (0xAC00, 0xD7A3),   // Hangul syllables
    (0xF900, 0xFAFF),   // CJK compatibility ideographs
    (0xFE30, 0xFE6F),   // CJK compatibility forms
    (0xFF00, 0xFF60),   // fullwidth forms
    (0xFFE0, 0xFFE6),   // fullwidth signs
    (0x1F300, 0x1F64F), // pictographs, emoticons
    (0x1F680, 0x1F6FF), // transport symbols
    (0x1F900, 0x1F9FF), // supplemental symbols
    (0x20000, 0x3FFFD), // CJK extensions B and on
];

/// Whether `ch` takes two columns.
pub fn is_wide(ch: char) -> bool {
    let code = ch as u32;
    WIDE.iter().any(|&(from, to)| (from..=to).contains(&code))
}

#[cfg(test)]
mod tests {
    use super::is_wide;

    #[test]
    fn east_asian_text_and_emoji_are_wide_and_latin_is_not() {
        assert!(is_wide('日') && is_wide('語') && is_wide('한') && is_wide('😀'));
        assert!(!is_wide('a') && !is_wide('é') && !is_wide(' ') && !is_wide('•'));
    }
}
