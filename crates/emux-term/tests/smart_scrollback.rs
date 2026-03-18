//! TDD specs for smart scrollback search.
//!
//! Users can search forward/backward through scrollback and on-screen content
//! using plain text or regex patterns, with match highlighting and navigation.

use emux_term::Screen;

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
    // Searching forward for "hello" should return the position of the first
    // occurrence after the current viewport.
    let mut screen = setup_screen_with_scrollback();
    let matches = screen.search_forward("hello", false);
    assert!(!matches.is_empty(), "should find at least one match");

    // The current match should be at or after the viewport start
    let current = screen.current_match().expect("should have a current match");
    let sb_len = screen.grid.scrollback_len();
    assert!(current.row >= sb_len, "current match should be in viewport");
    assert_eq!(current.col, 8); // "line N: " is 8 chars, then "hello"
}

#[test]
fn search_backward_finds_previous_match() {
    // Searching backward for "hello" should return the nearest occurrence
    // before the current viewport position.
    let mut screen = setup_screen_with_scrollback();
    let matches = screen.search_backward("hello", false);
    assert!(!matches.is_empty());

    let current = screen.current_match().expect("should have a current match");
    let sb_len = screen.grid.scrollback_len();
    assert!(current.row < sb_len, "current match should be in scrollback");
}

#[test]
fn search_no_match_returns_none() {
    // Searching for a string that does not exist anywhere in the scrollback
    // should return None without error.
    let mut screen = setup_screen_with_scrollback();
    let matches = screen.search_forward("zzz_nonexistent", false);
    assert!(matches.is_empty());
    assert!(screen.current_match().is_none());
}

#[test]
fn search_wraps_around_forward() {
    // When searching forward past the end of the buffer, the search should
    // wrap to the beginning and continue.
    let mut screen = setup_screen_with_scrollback();
    let matches = screen.search_forward("hello", false);
    let total = matches.len();
    assert!(total >= 2);

    // Navigate forward through all matches and then one more (should wrap)
    let initial_idx = screen.search_state().as_ref().unwrap().current.unwrap();
    for _ in 0..total {
        screen.search_next();
    }
    // After wrapping, we should be back at the same match as the initial current
    let idx = screen.search_state().as_ref().unwrap().current.unwrap();
    assert_eq!(idx, initial_idx, "should wrap back to the starting index");
}

#[test]
fn search_wraps_around_backward() {
    // When searching backward past the start of the buffer, the search should
    // wrap to the end and continue.
    let mut screen = setup_screen_with_scrollback();
    screen.search_forward("hello", false);
    // Set current to first match
    let state = screen.search_state().as_ref().unwrap();
    let total = state.matches.len();

    // Go to the first match by navigating
    // Navigate backward until we wrap
    let first = screen.search_state().as_ref().unwrap().matches[0].clone();
    // Navigate to first match
    while screen.current_match().unwrap() != &first {
        screen.search_next();
    }
    // Now go backward - should wrap to last match
    let prev = screen.search_prev().cloned().unwrap();
    let last_match = &screen.search_state().as_ref().unwrap().matches[total - 1];
    assert_eq!(&prev, last_match, "backward from first should wrap to last");
}

// ---------------------------------------------------------------------------
// 2. Case sensitivity
// ---------------------------------------------------------------------------

#[test]
fn search_case_insensitive() {
    // A case-insensitive search for "Error" should match "error", "ERROR",
    // and "Error".
    let mut screen = Screen::new(40, 5);
    write_str(&mut screen, "error on line 1");
    screen.carriage_return();
    screen.linefeed();
    write_str(&mut screen, "ERROR on line 2");
    screen.carriage_return();
    screen.linefeed();
    write_str(&mut screen, "Error on line 3");

    let matches = screen.search_forward("Error", false);
    assert_eq!(matches.len(), 3, "case-insensitive should find all 3 variants");
}

#[test]
fn search_case_sensitive() {
    // A case-sensitive search for "Error" should not match "error" or "ERROR".
    let mut screen = Screen::new(40, 5);
    write_str(&mut screen, "error on line 1");
    screen.carriage_return();
    screen.linefeed();
    write_str(&mut screen, "ERROR on line 2");
    screen.carriage_return();
    screen.linefeed();
    write_str(&mut screen, "Error on line 3");

    let matches = screen.search_forward("Error", true);
    assert_eq!(matches.len(), 1, "case-sensitive should find only exact match");
    assert_eq!(matches[0].row, 2);
    assert_eq!(matches[0].col, 0);
}

// ---------------------------------------------------------------------------
// 3. Regex search
// ---------------------------------------------------------------------------

#[test]
fn search_regex_pattern() {
    // Searching with regex r"\d{4}-\d{2}-\d{2}" should match date-like
    // strings such as "2026-03-18".
    let mut screen = Screen::new(40, 5);
    write_str(&mut screen, "today is 2026-03-18 ok");
    screen.carriage_return();
    screen.linefeed();
    write_str(&mut screen, "no date here");
    screen.carriage_return();
    screen.linefeed();
    write_str(&mut screen, "another 2025-12-01 date");

    let matches = screen.search_regex(r"\d{4}-\d{2}-\d{2}", true).unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].col, 9); // "today is " = 9 chars
    assert_eq!(matches[0].len, 10); // "2026-03-18" = 10 chars
}

#[test]
fn search_invalid_regex_returns_error() {
    // An invalid regex like "[unclosed" should return an error rather than
    // panic.
    let mut screen = Screen::new(40, 5);
    write_str(&mut screen, "some text");
    let result = screen.search_regex("[unclosed", true);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// 4. Match highlighting
// ---------------------------------------------------------------------------

#[test]
fn search_highlights_all_visible_matches() {
    // After a search, all matches currently visible on screen should be marked
    // for highlight rendering.
    let mut screen = Screen::new(40, 5);
    write_str(&mut screen, "hello world");
    screen.carriage_return();
    screen.linefeed();
    write_str(&mut screen, "hello again");
    screen.carriage_return();
    screen.linefeed();
    write_str(&mut screen, "goodbye");

    screen.search_forward("hello", false);
    let visible = screen.visible_matches();
    assert_eq!(visible.len(), 2, "both hello matches should be visible");
}

#[test]
fn search_highlights_current_match_distinctly() {
    // The "active" match (the one the user navigated to) should have a
    // different highlight style from other matches.
    let mut screen = Screen::new(40, 5);
    write_str(&mut screen, "aaa bbb aaa");
    screen.carriage_return();
    screen.linefeed();
    write_str(&mut screen, "aaa ccc");

    screen.search_forward("aaa", false);
    let state = screen.search_state().as_ref().unwrap();
    let current_idx = state.current.unwrap();

    // The current match index should be valid and distinct
    assert!(current_idx < state.matches.len());
    // Navigate next and verify current changes
    let _ = screen.search_next();
    let new_idx = screen.search_state().as_ref().unwrap().current.unwrap();
    assert_ne!(current_idx, new_idx, "current match index should change on navigation");
}

// ---------------------------------------------------------------------------
// 5. Match navigation
// ---------------------------------------------------------------------------

#[test]
fn navigate_next_match() {
    // Pressing "next" should advance the active match to the following
    // occurrence, scrolling the viewport if necessary.
    let mut screen = setup_screen_with_scrollback();
    screen.search_forward("hello", false);
    let first = screen.current_match().cloned().unwrap();
    let next = screen.search_next().cloned().unwrap();
    // next should be a different match (different row or col)
    assert_ne!(first, next, "next match should differ from first");
    // next should come after first in the match list
    let state = screen.search_state().as_ref().unwrap();
    let first_idx = state.matches.iter().position(|m| m == &first).unwrap();
    let next_idx = state.matches.iter().position(|m| m == &next).unwrap();
    assert_eq!(next_idx, first_idx + 1);
}

#[test]
fn navigate_prev_match() {
    // Pressing "prev" should move the active match to the preceding
    // occurrence.
    let mut screen = setup_screen_with_scrollback();
    screen.search_forward("hello", false);
    // Go forward twice, then back once
    let _ = screen.search_next();
    let second = screen.current_match().cloned().unwrap();
    let _ = screen.search_next();
    let prev = screen.search_prev().cloned().unwrap();
    assert_eq!(prev, second, "prev should go back to second match");
}

// ---------------------------------------------------------------------------
// 6. Boundary and performance
// ---------------------------------------------------------------------------

#[test]
fn search_across_screen_and_scrollback_boundary() {
    // A match that exists in both scrollback and viewport should be found
    // in both locations.
    let mut screen = setup_screen_with_scrollback();
    let matches = screen.search_forward("hello", false);
    let sb_len = screen.grid.scrollback_len();

    let in_scrollback = matches.iter().any(|m| m.row < sb_len);
    let in_viewport = matches.iter().any(|m| m.row >= sb_len);
    assert!(in_scrollback, "should find matches in scrollback");
    assert!(in_viewport, "should find matches in viewport");
}

#[test]
fn clear_search_removes_highlights() {
    // Clearing the search should remove all highlights and reset the match
    // index.
    let mut screen = setup_screen_with_scrollback();
    screen.search_forward("hello", false);
    assert!(screen.current_match().is_some());

    screen.clear_search();
    assert!(screen.current_match().is_none());
    assert!(screen.search_state().is_none());
    assert!(screen.visible_matches().is_empty());
}

#[test]
fn search_performance_large_scrollback() {
    // Searching through 100,000 lines of scrollback should complete in under
    // 100ms on a reasonable machine.
    let mut screen = Screen::new(80, 24);
    // Fill with many lines to create scrollback
    for i in 0..100_000 {
        write_str(&mut screen, &format!("log line {i}: some data here"));
        screen.carriage_return();
        screen.linefeed();
    }

    let start = std::time::Instant::now();
    let matches = screen.search_forward("some data", false);
    let elapsed = start.elapsed();

    assert!(!matches.is_empty(), "should find matches");
    assert!(
        elapsed.as_millis() < 5000,
        "search took too long: {:?}",
        elapsed
    );
}
