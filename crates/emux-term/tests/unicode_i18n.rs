mod common;
use common::TestTerminal;

// ── Korean (Hangul) ──────────────────────────────────────────────────────────

#[test]
fn korean_hangul_width() {
    // 한글은 각 글자가 2칸
    let mut t = TestTerminal::new(20, 5);
    t.push_str("안녕하세요");
    assert_eq!(t.cursor(), (0, 10)); // 5 chars x 2 width = 10
    assert_eq!(t.cell(0, 0).c, '안');
    assert_eq!(t.cell(0, 0).width, 2);
    assert_eq!(t.cell(0, 2).c, '녕');
    assert_eq!(t.cell(0, 2).width, 2);
    assert_eq!(t.cell(0, 4).c, '하');
    assert_eq!(t.cell(0, 6).c, '세');
    assert_eq!(t.cell(0, 8).c, '요');
    // Continuation cells should have width 0
    assert_eq!(t.cell(0, 1).width, 0);
    assert_eq!(t.cell(0, 3).width, 0);
}

#[test]
fn korean_mixed_with_ascii() {
    // "Hello 세계" = 6 ASCII(6칸) + 2 한글(4칸) = 10칸
    let mut t = TestTerminal::new(20, 5);
    t.push_str("Hello 세계");
    assert_eq!(t.cursor(), (0, 10));
    assert_eq!(t.cell(0, 0).c, 'H');
    assert_eq!(t.cell(0, 5).c, ' ');
    assert_eq!(t.cell(0, 6).c, '세');
    assert_eq!(t.cell(0, 6).width, 2);
    assert_eq!(t.cell(0, 8).c, '계');
}

#[test]
fn korean_at_line_end_wraps() {
    // 79열에서 한글(2칸) 쓰면 다음 줄로 wrap
    let mut t = TestTerminal::new(80, 5);
    t.push_str(&" ".repeat(79));
    t.push_str("가"); // needs 2 cols, only 1 left -> should wrap
    // Wide char should wrap to next line
    assert_eq!(t.cell(1, 0).c, '가');
    assert_eq!(t.cell(1, 0).width, 2);
    // Cursor should be at row 1, col 2 (past the wide char)
    assert_eq!(t.cursor(), (1, 2));
}

#[test]
fn korean_jamo() {
    // 자모 (ㄱ, ㅏ 등) -- 호환 자모는 wide (falls in 0x3131-0x318E, within 0x3040-0x33BF)
    let mut t = TestTerminal::new(20, 5);
    t.push_str("ㄱㅏ");
    assert_eq!(t.cursor(), (0, 4)); // 2 chars x 2 width = 4
    assert_eq!(t.cell(0, 0).c, 'ㄱ');
    assert_eq!(t.cell(0, 0).width, 2);
    assert_eq!(t.cell(0, 2).c, 'ㅏ');
    assert_eq!(t.cell(0, 2).width, 2);
}

#[test]
fn korean_long_sentence_wraps() {
    // 10칸에서 6글자(12칸) -> 첫째 줄 5글자(10칸), 둘째 줄 1글자
    let mut t = TestTerminal::new(10, 5);
    t.push_str("가나다라마바");
    assert_eq!(t.cell(0, 0).c, '가');
    assert_eq!(t.cell(0, 8).c, '마');
    assert_eq!(t.cell(1, 0).c, '바');
}

// ── Japanese ─────────────────────────────────────────────────────────────────

#[test]
fn japanese_katakana() {
    let mut t = TestTerminal::new(20, 5);
    t.push_str("カタカナ");
    assert_eq!(t.cursor(), (0, 8)); // 4 x 2
    assert_eq!(t.cell(0, 0).c, 'カ');
    assert_eq!(t.cell(0, 2).c, 'タ');
    assert_eq!(t.cell(0, 4).c, 'カ');
    assert_eq!(t.cell(0, 6).c, 'ナ');
}

#[test]
fn japanese_hiragana() {
    let mut t = TestTerminal::new(20, 5);
    t.push_str("ひらがな");
    assert_eq!(t.cursor(), (0, 8)); // 4 x 2
    assert_eq!(t.cell(0, 0).c, 'ひ');
    assert_eq!(t.cell(0, 6).c, 'な');
}

#[test]
fn japanese_half_width_katakana() {
    // ｶﾀｶﾅ -- half-width katakana, 1칸씩
    // Half-width katakana is in 0xFF61-0xFF9F, outside the fullwidth range (0xFF01-0xFF60)
    let mut t = TestTerminal::new(20, 5);
    t.push_str("ｶﾀｶﾅ");
    assert_eq!(t.cursor(), (0, 4)); // 4 x 1
    assert_eq!(t.cell(0, 0).c, 'ｶ');
    assert_eq!(t.cell(0, 0).width, 1);
}

#[test]
fn japanese_kanji() {
    let mut t = TestTerminal::new(20, 5);
    t.push_str("漢字");
    assert_eq!(t.cursor(), (0, 4)); // 2 x 2
    assert_eq!(t.cell(0, 0).c, '漢');
    assert_eq!(t.cell(0, 0).width, 2);
}

// ── Chinese ──────────────────────────────────────────────────────────────────

#[test]
fn chinese_simplified() {
    let mut t = TestTerminal::new(20, 5);
    t.push_str("你好世界");
    assert_eq!(t.cursor(), (0, 8)); // 4 x 2
    assert_eq!(t.cell(0, 0).c, '你');
    assert_eq!(t.cell(0, 2).c, '好');
    assert_eq!(t.cell(0, 4).c, '世');
    assert_eq!(t.cell(0, 6).c, '界');
}

#[test]
fn chinese_traditional() {
    let mut t = TestTerminal::new(20, 5);
    t.push_str("開發者");
    assert_eq!(t.cursor(), (0, 6)); // 3 x 2
}

// ── Emoji ────────────────────────────────────────────────────────────────────

#[test]
fn emoji_basic() {
    let mut t = TestTerminal::new(20, 5);
    t.push_str("😀🎉🚀");
    assert_eq!(t.cursor(), (0, 6)); // 3 x 2
    assert_eq!(t.cell(0, 0).c, '😀');
    assert_eq!(t.cell(0, 0).width, 2);
    assert_eq!(t.cell(0, 2).c, '🎉');
    assert_eq!(t.cell(0, 4).c, '🚀');
}

#[test]
fn emoji_flags() {
    // 🇰🇷 = U+1F1F0 U+1F1F7 (regional indicator pair)
    // Each regional indicator symbol is U+1F1E6..U+1F1FF, in the emoji range
    let mut t = TestTerminal::new(20, 5);
    t.push_str("🇰🇷");
    // Should not panic; implementation writes each codepoint separately
    let (row, _col) = t.cursor();
    assert_eq!(row, 0);
}

#[test]
fn emoji_skin_tone() {
    // 👋🏻 = U+1F44B U+1F3FB
    let mut t = TestTerminal::new(20, 5);
    t.push_str("👋🏻");
    // Should not panic
    let (row, _col) = t.cursor();
    assert_eq!(row, 0);
}

#[test]
fn emoji_zwj_family() {
    // 👨‍👩‍👧‍👦 = complex ZWJ sequence (U+200D between each person)
    let mut t = TestTerminal::new(40, 5);
    t.push_str("👨\u{200D}👩\u{200D}👧\u{200D}👦");
    // Should not panic, exact width depends on implementation
    // ZWJ chars are zero-width, so only the emoji code points take space
    let (row, _col) = t.cursor();
    assert_eq!(row, 0);
}

#[test]
fn emoji_mixed_with_text() {
    let mut t = TestTerminal::new(30, 5);
    t.push_str("Hi 😀 Bye");
    // H(1) i(1) (1) 😀(2) (1) B(1) y(1) e(1) = 9
    assert_eq!(t.cursor(), (0, 9));
}

// ── Arabic / Hebrew (RTL scripts, narrow chars) ─────────────────────────────

#[test]
fn arabic_text() {
    let mut t = TestTerminal::new(20, 5);
    t.push_str("مرحبا");
    // Arabic letters are narrow (1 cell each), RTL handling is display-level
    assert_eq!(t.cursor(), (0, 5));
}

#[test]
fn hebrew_text() {
    let mut t = TestTerminal::new(20, 5);
    t.push_str("שלום");
    assert_eq!(t.cursor(), (0, 4));
    assert_eq!(t.cell(0, 0).width, 1);
}

// ── Thai / Devanagari (combining marks) ─────────────────────────────────────

#[test]
fn thai_with_combining() {
    let mut t = TestTerminal::new(20, 5);
    t.push_str("สวัสดี"); // Thai with combining marks
    // Should not panic; combining marks are zero-width
    let (row, _col) = t.cursor();
    assert_eq!(row, 0);
}

#[test]
fn devanagari() {
    let mut t = TestTerminal::new(20, 5);
    t.push_str("नमस्ते"); // Hindi "Namaste"
    // Should not panic
    let (row, _col) = t.cursor();
    assert_eq!(row, 0);
}

// ── Edge cases ───────────────────────────────────────────────────────────────

#[test]
fn mixed_cjk_ascii_emoji_line() {
    let mut t = TestTerminal::new(40, 5);
    t.push_str("Hello 세계! 🌍 你好");
    // H(1) e(1) l(1) l(1) o(1) (1) 세(2) 계(2) !(1) (1) 🌍(2) (1) 你(2) 好(2)
    // = 6 + 4 + 2 + 2 + 1 + 4 = 19
    assert_eq!(t.cursor(), (0, 19));
}

#[test]
fn fullwidth_ascii() {
    // Ｈｅｌｌｏ -- fullwidth ASCII, 2칸씩
    let mut t = TestTerminal::new(20, 5);
    t.push_str("Ｈｅｌｌｏ");
    assert_eq!(t.cursor(), (0, 10)); // 5 x 2
    assert_eq!(t.cell(0, 0).c, 'Ｈ');
    assert_eq!(t.cell(0, 0).width, 2);
    assert_eq!(t.cell(0, 1).width, 0); // continuation
}

#[test]
fn wide_char_overwrite_with_narrow() {
    // Write wide char, then overwrite first half with narrow
    let mut t = TestTerminal::new(10, 3);
    t.push_str("가");
    assert_eq!(t.cell(0, 0).c, '가');
    assert_eq!(t.cell(0, 0).width, 2);
    t.push_str("\x1b[1G"); // CUP: move to col 1 (0-based col 0)
    t.push_str("A"); // overwrite first half of '가'
    assert_eq!(t.cell(0, 0).c, 'A');
    assert_eq!(t.cell(0, 0).width, 1);
    // The continuation cell should also be cleaned up
    assert_eq!(t.cell(0, 1).width, 1);
}

#[test]
fn wide_char_overwrite_second_half() {
    // Write wide char at col 0, then overwrite the continuation cell at col 1
    let mut t = TestTerminal::new(10, 3);
    t.push_str("가나");
    // '가' at col 0-1, '나' at col 2-3
    t.push_str("\x1b[1;2H"); // move to row 1, col 2 (0-based: row 0, col 1)
    t.push_str("X");
    // Overwriting col 1 (continuation of '가') should clean up the wide char head
    assert_eq!(t.cell(0, 0).c, ' '); // head cleaned to space
    assert_eq!(t.cell(0, 0).width, 1);
    assert_eq!(t.cell(0, 1).c, 'X');
    assert_eq!(t.cell(0, 1).width, 1);
}

#[test]
fn scroll_with_wide_chars() {
    let mut t = TestTerminal::new(10, 3);
    t.push_str("가나다라마\n");
    t.push_str("바사아자차\n");
    t.push_str("카타파하\n");
    // Verify scrolling preserves wide char integrity -- screen should not panic
    // After scrolling, check that some wide chars are still intact
    // Row 0 should have been scrolled away, row 1 content is now row 0
    let c = t.cell(0, 0);
    assert!(c.width == 1 || c.width == 2); // valid width, not corrupt
}

#[test]
fn erase_display_with_korean() {
    let mut t = TestTerminal::new(20, 3);
    t.push_str("안녕하세요");
    assert_eq!(t.cell(0, 0).c, '안');
    t.push_str("\x1b[2J"); // ED: clear entire screen
    assert_eq!(t.row_text(0).trim(), "");
    // All cells should be blank
    assert_eq!(t.cell(0, 0).c, ' ');
    assert_eq!(t.cell(0, 0).width, 1);
}

#[test]
fn erase_line_with_korean() {
    let mut t = TestTerminal::new(20, 3);
    t.push_str("안녕하세요");
    t.push_str("\x1b[1G"); // move to col 0
    t.push_str("\x1b[2K"); // EL: clear entire line
    assert_eq!(t.row_text(0).trim(), "");
}

#[test]
fn search_korean_text() {
    let mut t = TestTerminal::new(40, 5);
    t.push_str("Hello 세계 World 안녕");
    let matches = t.screen.search_forward("세계", true);
    assert_eq!(matches.len(), 1);
    // In row_text, the text is "Hello 세계 World 안녕"
    // "세" is at char position 6 (after H,e,l,l,o,space)
    assert_eq!(matches[0].col, 6);
    assert_eq!(matches[0].len, 2);
}

#[test]
fn search_multiple_korean_matches() {
    let mut t = TestTerminal::new(40, 5);
    t.push_str("안녕 세상 안녕 친구");
    let matches = t.screen.search_forward("안녕", true);
    assert_eq!(matches.len(), 2);
}

#[test]
fn insert_mode_with_wide_chars() {
    let mut t = TestTerminal::new(20, 3);
    t.push_str("ABCDEF");
    t.push_str("\x1b[1G");    // move to col 0
    t.push_str("\x1b[4h");    // enable insert mode
    t.push_str("가");          // insert wide char, shifting existing content right
    assert_eq!(t.cell(0, 0).c, '가');
    assert_eq!(t.cell(0, 0).width, 2);
    assert_eq!(t.cell(0, 2).c, 'A');
}

#[test]
fn delete_chars_with_wide_chars() {
    let mut t = TestTerminal::new(20, 3);
    t.push_str("가나다ABC");
    t.push_str("\x1b[1G");    // move to col 0
    t.push_str("\x1b[2P");    // DCH: delete 2 cells at cursor
    // After deleting 2 cells from the start, content shifts left
    // Should not corrupt wide char state
    let c = t.cell(0, 0);
    assert!(c.width == 1 || c.width == 2);
}

#[test]
fn cursor_movement_across_wide_chars() {
    let mut t = TestTerminal::new(20, 3);
    t.push_str("가나다");
    // cursor is at (0, 6)
    assert_eq!(t.cursor(), (0, 6));
    t.push_str("\x1b[1G");   // move to col 0
    assert_eq!(t.cursor(), (0, 0));
    t.push_str("\x1b[4G");   // move to col 4 (0-based col 3)
    assert_eq!(t.cursor(), (0, 3));
}

#[test]
fn multiple_lines_of_cjk() {
    let mut t = TestTerminal::new(20, 5);
    t.push_str("第一行\r\n");
    t.push_str("第二行\r\n");
    t.push_str("第三行");
    assert_eq!(t.cell(0, 0).c, '第');
    assert_eq!(t.cell(1, 0).c, '第');
    assert_eq!(t.cell(2, 0).c, '第');
    assert_eq!(t.cursor(), (2, 6));
}

#[test]
fn cjk_with_ansi_colors() {
    // Ensure ANSI color codes don't affect wide char handling
    let mut t = TestTerminal::new(20, 5);
    t.push_str("\x1b[31m한글\x1b[0m");
    assert_eq!(t.cursor(), (0, 4));
    assert_eq!(t.cell(0, 0).c, '한');
    assert_eq!(t.cell(0, 0).width, 2);
    assert_eq!(t.cell(0, 2).c, '글');
}

#[test]
fn zero_width_space_does_not_advance() {
    let mut t = TestTerminal::new(20, 5);
    t.push_str("A\u{200B}B"); // A + ZWSP + B
    // ZWSP is zero-width, so cursor at col 2
    assert_eq!(t.cursor(), (0, 2));
    assert_eq!(t.cell(0, 0).c, 'A');
    assert_eq!(t.cell(0, 1).c, 'B');
}

#[test]
fn fullwidth_punctuation() {
    // Fullwidth punctuation: ！ (U+FF01), ？ (U+FF1F) -- in fullwidth range
    let mut t = TestTerminal::new(20, 5);
    t.push_str("！？");
    assert_eq!(t.cursor(), (0, 4)); // 2 x 2
    assert_eq!(t.cell(0, 0).c, '！');
    assert_eq!(t.cell(0, 0).width, 2);
}

#[test]
fn wide_char_at_exact_line_boundary() {
    // Wide char fits exactly at the end of the line (no wrap needed)
    let mut t = TestTerminal::new(10, 3);
    t.push_str(&" ".repeat(8));
    t.push_str("가"); // cols 8-9, fits exactly
    assert_eq!(t.cell(0, 8).c, '가');
    assert_eq!(t.cell(0, 8).width, 2);
    // Cursor should be at end of row with pending wrap
    assert_eq!(t.cursor().0, 0);
}

#[test]
fn rapid_wide_narrow_alternation() {
    let mut t = TestTerminal::new(20, 5);
    t.push_str("A가B나C다D");
    // A(1) 가(2) B(1) 나(2) C(1) 다(2) D(1) = 10
    assert_eq!(t.cursor(), (0, 10));
    assert_eq!(t.cell(0, 0).c, 'A');
    assert_eq!(t.cell(0, 1).c, '가');
    assert_eq!(t.cell(0, 1).width, 2);
    assert_eq!(t.cell(0, 3).c, 'B');
    assert_eq!(t.cell(0, 4).c, '나');
}
