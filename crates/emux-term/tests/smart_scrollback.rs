//! TDD specs for smart scrollback search.
//!
//! Users can search forward/backward through scrollback and on-screen content
//! using plain text or regex patterns, with match highlighting and navigation.

use emux_term::Screen;
use emux_term::search::ScreenSearcher;

/// Helper: write a string to the screen character by character.
fn write_str(screen: &mut Screen, s: &str) {
    for c in s.chars() {
        if c == '\n' {
            screen.carriage_return();
            screen.linefeed();
        } else {
            screen.write_char(c);
        }
    }
}

/// Helper: create a screen and fill it with lines that overflow into scrollback.
/// Returns a screen with some content in scrollback and some on the viewport.
fn setup_screen_with_scrollback() -> Screen {
    let mut screen = Screen::new(40, 5);
    // Write 10 lines so that the first 5 go into scrollback
    for i in 0..10 {
        write_str(&mut screen, &format!("line {i}: hello world"));
        if i < 9 {
            screen.carriage_return();
            screen.linefeed();
        }
    }
    screen
}

// ---------------------------------------------------------------------------
// 1. Basic text search
// ---------------------------------------------------------------------------

#[test]
fn search_forward_finds_first_match() {
    let screen = setup_screen_with_scrollback();
    let mut searcher = ScreenSearcher::new();
    let matches = searcher.search_forward(&screen, "hello", false);
    assert!(!matches.is_empty(), "should find at least one match");

    let current = searcher
        .current_match()
        .expect("should have a current match");
    let sb_len = screen.grid.scrollback_len();
    assert!(current.row >= sb_len, "current match should be in viewport");
    assert_eq!(current.col, 8);
}

#[test]
fn search_backward_finds_previous_match() {
    let screen = setup_screen_with_scrollback();
    let mut searcher = ScreenSearcher::new();
    let matches = searcher.search_backward(&screen, "hello", false);
    assert!(!matches.is_empty());

    let current = searcher
        .current_match()
        .expect("should have a current match");
    let sb_len = screen.grid.scrollback_len();
    assert!(
        current.row < sb_len,
        "current match should be in scrollback"
    );
}

#[test]
fn search_no_match_returns_none() {
    let screen = setup_screen_with_scrollback();
    let mut searcher = ScreenSearcher::new();
    let matches = searcher.search_forward(&screen, "zzz_nonexistent", false);
    assert!(matches.is_empty());
    assert!(searcher.current_match().is_none());
}

#[test]
fn search_wraps_around_forward() {
    let screen = setup_screen_with_scrollback();
    let mut searcher = ScreenSearcher::new();
    let matches = searcher.search_forward(&screen, "hello", false);
    let total = matches.len();
    assert!(total >= 2);

    let initial_idx = searcher.search_state().as_ref().unwrap().current.unwrap();
    for _ in 0..total {
        searcher.search_next();
    }
    let idx = searcher.search_state().as_ref().unwrap().current.unwrap();
    assert_eq!(idx, initial_idx, "should wrap back to the starting index");
}

#[test]
fn search_wraps_around_backward() {
    let screen = setup_screen_with_scrollback();
    let mut searcher = ScreenSearcher::new();
    searcher.search_forward(&screen, "hello", false);
    let state = searcher.search_state().as_ref().unwrap();
    let total = state.matches.len();

    let first = searcher.search_state().as_ref().unwrap().matches[0].clone();
    while searcher.current_match().unwrap() != &first {
        searcher.search_next();
    }
    let prev = searcher.search_prev().cloned().unwrap();
    let last_match = &searcher.search_state().as_ref().unwrap().matches[total - 1];
    assert_eq!(&prev, last_match, "backward from first should wrap to last");
}

// ---------------------------------------------------------------------------
// 2. Case sensitivity
// ---------------------------------------------------------------------------

#[test]
fn search_case_insensitive() {
    let mut screen = Screen::new(40, 5);
    write_str(&mut screen, "error on line 1");
    screen.carriage_return();
    screen.linefeed();
    write_str(&mut screen, "ERROR on line 2");
    screen.carriage_return();
    screen.linefeed();
    write_str(&mut screen, "Error on line 3");

    let mut searcher = ScreenSearcher::new();
    let matches = searcher.search_forward(&screen, "Error", false);
    assert_eq!(
        matches.len(),
        3,
        "case-insensitive should find all 3 variants"
    );
}

#[test]
fn search_case_sensitive() {
    let mut screen = Screen::new(40, 5);
    write_str(&mut screen, "error on line 1");
    screen.carriage_return();
    screen.linefeed();
    write_str(&mut screen, "ERROR on line 2");
    screen.carriage_return();
    screen.linefeed();
    write_str(&mut screen, "Error on line 3");

    let mut searcher = ScreenSearcher::new();
    let matches = searcher.search_forward(&screen, "Error", true);
    assert_eq!(
        matches.len(),
        1,
        "case-sensitive should find only exact match"
    );
    assert_eq!(matches[0].row, 2);
    assert_eq!(matches[0].col, 0);
}

// ---------------------------------------------------------------------------
// 3. Regex search
// ---------------------------------------------------------------------------

#[test]
fn search_regex_pattern() {
    let mut screen = Screen::new(40, 5);
    write_str(&mut screen, "today is 2026-03-18 ok");
    screen.carriage_return();
    screen.linefeed();
    write_str(&mut screen, "no date here");
    screen.carriage_return();
    screen.linefeed();
    write_str(&mut screen, "another 2025-12-01 date");

    let mut searcher = ScreenSearcher::new();
    let matches = searcher
        .search_regex(&screen, r"\d{4}-\d{2}-\d{2}", true)
        .unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].col, 9);
    assert_eq!(matches[0].len, 10);
}

#[test]
fn search_invalid_regex_returns_error() {
    let mut screen = Screen::new(40, 5);
    write_str(&mut screen, "some text");
    let mut searcher = ScreenSearcher::new();
    let result = searcher.search_regex(&screen, "[unclosed", true);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// 4. Match highlighting
// ---------------------------------------------------------------------------

#[test]
fn search_highlights_all_visible_matches() {
    let mut screen = Screen::new(40, 5);
    write_str(&mut screen, "hello world");
    screen.carriage_return();
    screen.linefeed();
    write_str(&mut screen, "hello again");
    screen.carriage_return();
    screen.linefeed();
    write_str(&mut screen, "goodbye");

    let mut searcher = ScreenSearcher::new();
    searcher.search_forward(&screen, "hello", false);
    let visible = searcher.visible_matches(&screen);
    assert_eq!(visible.len(), 2, "both hello matches should be visible");
}

#[test]
fn search_highlights_current_match_distinctly() {
    let mut screen = Screen::new(40, 5);
    write_str(&mut screen, "aaa bbb aaa");
    screen.carriage_return();
    screen.linefeed();
    write_str(&mut screen, "aaa ccc");

    let mut searcher = ScreenSearcher::new();
    searcher.search_forward(&screen, "aaa", false);
    let state = searcher.search_state().as_ref().unwrap();
    let current_idx = state.current.unwrap();

    assert!(current_idx < state.matches.len());
    let _ = searcher.search_next();
    let new_idx = searcher.search_state().as_ref().unwrap().current.unwrap();
    assert_ne!(
        current_idx, new_idx,
        "current match index should change on navigation"
    );
}

// ---------------------------------------------------------------------------
// 5. Match navigation
// ---------------------------------------------------------------------------

#[test]
fn navigate_next_match() {
    let screen = setup_screen_with_scrollback();
    let mut searcher = ScreenSearcher::new();
    searcher.search_forward(&screen, "hello", false);
    let first = searcher.current_match().cloned().unwrap();
    let next = searcher.search_next().cloned().unwrap();
    assert_ne!(first, next, "next match should differ from first");
    let state = searcher.search_state().as_ref().unwrap();
    let first_idx = state.matches.iter().position(|m| m == &first).unwrap();
    let next_idx = state.matches.iter().position(|m| m == &next).unwrap();
    assert_eq!(next_idx, first_idx + 1);
}

#[test]
fn navigate_prev_match() {
    let screen = setup_screen_with_scrollback();
    let mut searcher = ScreenSearcher::new();
    searcher.search_forward(&screen, "hello", false);
    let _ = searcher.search_next();
    let second = searcher.current_match().cloned().unwrap();
    let _ = searcher.search_next();
    let prev = searcher.search_prev().cloned().unwrap();
    assert_eq!(prev, second, "prev should go back to second match");
}

// ---------------------------------------------------------------------------
// 6. Boundary and performance
// ---------------------------------------------------------------------------

#[test]
fn search_across_screen_and_scrollback_boundary() {
    let screen = setup_screen_with_scrollback();
    let mut searcher = ScreenSearcher::new();
    let matches = searcher.search_forward(&screen, "hello", false);
    let sb_len = screen.grid.scrollback_len();

    let in_scrollback = matches.iter().any(|m| m.row < sb_len);
    let in_viewport = matches.iter().any(|m| m.row >= sb_len);
    assert!(in_scrollback, "should find matches in scrollback");
    assert!(in_viewport, "should find matches in viewport");
}

#[test]
fn clear_search_removes_highlights() {
    let screen = setup_screen_with_scrollback();
    let mut searcher = ScreenSearcher::new();
    searcher.search_forward(&screen, "hello", false);
    assert!(searcher.current_match().is_some());

    searcher.clear_search();
    assert!(searcher.current_match().is_none());
    assert!(searcher.search_state().is_none());
    assert!(searcher.visible_matches(&screen).is_empty());
}

#[test]
fn search_performance_large_scrollback() {
    let mut screen = Screen::new(80, 24);
    for i in 0..100_000 {
        write_str(&mut screen, &format!("log line {i}: some data here"));
        screen.carriage_return();
        screen.linefeed();
    }

    let mut searcher = ScreenSearcher::new();
    let start = std::time::Instant::now();
    let matches = searcher.search_forward(&screen, "some data", false);
    let elapsed = start.elapsed();
    assert!(
        matches.len() > 1000,
        "expected many matches across scrollback, got {}",
        matches.len()
    );
    assert!(
        elapsed.as_millis() < 2000,
        "search through 100k lines took too long: {:?}",
        elapsed
    );
}
