//! Cross-pane search across all panes in a session.
//!
//! Provides [`search_text`] and [`search_lines`] for querying text content,
//! and [`search_session`] for searching across every pane in a [`Session`].

use crate::pane::PaneId;
use crate::session::Session;

/// A single match found within a set of lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchLineResult {
    /// Row index within the searched lines.
    pub row: usize,
    /// Column (character offset) where the match starts.
    pub col: usize,
    /// The matched text.
    pub matched_text: String,
    /// The full content of the line containing the match.
    pub line_content: String,
}

/// A search result from cross-pane search.
#[derive(Debug, Clone)]
pub struct GlobalSearchResult {
    /// The pane that contains this match.
    pub pane_id: PaneId,
    /// Title of the pane at the time of the search.
    pub pane_title: String,
    /// Index of the tab that contains this pane.
    pub tab_index: usize,
    /// Row index within the pane's scrollback.
    pub row: usize,
    /// Column (character offset) where the match starts.
    pub col: usize,
    /// The matched text.
    pub matched_text: String,
    /// The full content of the line containing the match.
    pub line_content: String,
}

/// Search a single text string for all occurrences of `query`.
///
/// Returns a vec of `(row, col, matched_text)` tuples. The text is treated as
/// a single row (row index 0). When `regex` is true, `query` is compiled as a
/// regular expression; otherwise a plain substring search is performed.
///
/// Returns an empty vec on no match or on an invalid regex pattern.
pub fn search_text(text: &str, query: &str, regex: bool) -> Vec<(usize, usize, String)> {
    if query.is_empty() || text.is_empty() {
        return Vec::new();
    }

    let lines: Vec<String> = text.lines().map(String::from).collect();
    search_lines(&lines, query, regex)
        .into_iter()
        .map(|r| (r.row, r.col, r.matched_text))
        .collect()
}

/// Search across multiple lines for all occurrences of `query`.
///
/// Each entry in `lines` corresponds to one row. When `regex` is true, `query`
/// is compiled as a regular expression; otherwise a plain substring search is
/// performed. Results include the row index, column offset, matched text, and
/// the full line content.
///
/// Returns an empty vec on no match or on an invalid regex pattern.
pub fn search_lines(lines: &[String], query: &str, regex: bool) -> Vec<SearchLineResult> {
    if query.is_empty() || lines.is_empty() {
        return Vec::new();
    }

    if regex {
        search_lines_regex(lines, query)
    } else {
        search_lines_plain(lines, query)
    }
}

/// Plain-text (case-sensitive) substring search across lines.
fn search_lines_plain(lines: &[String], query: &str) -> Vec<SearchLineResult> {
    let mut results = Vec::new();

    for (row_idx, line) in lines.iter().enumerate() {
        let mut start = 0;
        while let Some(byte_pos) = line[start..].find(query) {
            let abs_byte = start + byte_pos;
            let col = line[..abs_byte].chars().count();
            results.push(SearchLineResult {
                row: row_idx,
                col,
                matched_text: query.to_string(),
                line_content: line.clone(),
            });
            start = abs_byte + query.len().max(1);
        }
    }

    results
}

/// Regex search across lines. Returns an empty vec if the pattern is invalid.
fn search_lines_regex(lines: &[String], pattern: &str) -> Vec<SearchLineResult> {
    let re = match regex::Regex::new(pattern) {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };

    let mut results = Vec::new();

    for (row_idx, line) in lines.iter().enumerate() {
        for m in re.find_iter(line) {
            let col = line[..m.start()].chars().count();
            results.push(SearchLineResult {
                row: row_idx,
                col,
                matched_text: m.as_str().to_string(),
                line_content: line.clone(),
            });
        }
    }

    results
}

/// Search across all panes in a session.
///
/// Iterates every tab, and within each tab iterates all tiled and floating
/// panes. For each pane the scrollback buffer is searched for `query`. When
/// `regex` is true the query is treated as a regular expression pattern.
pub fn search_session(session: &Session, query: &str, regex: bool) -> Vec<GlobalSearchResult> {
    let mut results = Vec::new();

    if query.is_empty() {
        return results;
    }

    for tab_index in 0..session.tab_count() {
        let tab = match session.tab(tab_index) {
            Some(t) => t,
            None => continue,
        };

        // Search tiled panes
        for pane_id in tab.pane_ids() {
            if let Some(pane) = tab.pane(pane_id) {
                let lines: Vec<String> = pane.scrollback().to_vec();
                let matches = search_lines(&lines, query, regex);
                for m in matches {
                    results.push(GlobalSearchResult {
                        pane_id,
                        pane_title: pane.title().to_string(),
                        tab_index,
                        row: m.row,
                        col: m.col,
                        matched_text: m.matched_text,
                        line_content: m.line_content,
                    });
                }
            }
        }

        // Search floating panes
        for fp_id in tab.floating_pane_ids() {
            if let Some(fp) = tab.floating_pane(fp_id) {
                let pane = &fp.pane;
                let lines: Vec<String> = pane.scrollback().to_vec();
                let matches = search_lines(&lines, query, regex);
                for m in matches {
                    results.push(GlobalSearchResult {
                        pane_id: fp_id,
                        pane_title: pane.title().to_string(),
                        tab_index,
                        row: m.row,
                        col: m.col,
                        matched_text: m.matched_text,
                        line_content: m.line_content,
                    });
                }
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::SplitDirection;

    #[test]
    fn search_text_plain_match() {
        let results = search_text("hello world", "hello", false);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], (0, 0, "hello".to_string()));
    }

    #[test]
    fn search_text_regex_match() {
        let results = search_text("error 404 not found", r"\d+", true);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], (0, 6, "404".to_string()));
    }

    #[test]
    fn search_text_no_match() {
        let results = search_text("hello world", "foobar", false);
        assert!(results.is_empty());
    }

    #[test]
    fn search_text_multiple_matches() {
        let results = search_text("abcabc", "abc", false);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], (0, 0, "abc".to_string()));
        assert_eq!(results[1], (0, 3, "abc".to_string()));
    }

    #[test]
    fn search_text_case_sensitive() {
        let results = search_text("Hello hello HELLO", "hello", false);
        // Plain search is case-sensitive
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], (0, 6, "hello".to_string()));
    }

    #[test]
    fn search_lines_multiple_rows() {
        let lines = vec![
            "first line with target".to_string(),
            "no match here".to_string(),
            "another target found".to_string(),
        ];
        let results = search_lines(&lines, "target", false);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].row, 0);
        assert_eq!(results[0].col, 16);
        assert_eq!(results[0].matched_text, "target");
        assert_eq!(results[0].line_content, "first line with target");
        assert_eq!(results[1].row, 2);
        assert_eq!(results[1].col, 8);
    }

    #[test]
    fn search_lines_empty_input() {
        let lines: Vec<String> = vec![];
        let results = search_lines(&lines, "query", false);
        assert!(results.is_empty());
    }

    #[test]
    fn search_session_across_panes() {
        let mut session = Session::new("test", 80, 25);

        // Add content to the first pane in the first tab
        {
            let tab = session.active_tab_mut();
            let pane_id = tab.active_pane_id().unwrap();
            let pane = tab.pane_mut(pane_id).unwrap();
            pane.push_scrollback("hello from pane 0");
            pane.push_scrollback("world");
        }

        // Split to create a second pane
        {
            let tab = session.active_tab_mut();
            tab.split_pane(SplitDirection::Vertical);
            let pane_id = tab.active_pane_id().unwrap();
            let pane = tab.pane_mut(pane_id).unwrap();
            pane.push_scrollback("hello from pane 1");
        }

        let results = search_session(&session, "hello", false);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].tab_index, 0);
        assert_eq!(results[1].tab_index, 0);
        // Both should match "hello"
        assert!(results.iter().all(|r| r.matched_text == "hello"));
        // They should come from different panes
        assert_ne!(results[0].pane_id, results[1].pane_id);
    }

    #[test]
    fn search_session_empty_query() {
        let session = Session::new("test", 80, 25);
        let results = search_session(&session, "", false);
        assert!(results.is_empty());
    }

    #[test]
    fn search_session_no_match() {
        let mut session = Session::new("test", 80, 25);
        {
            let tab = session.active_tab_mut();
            let pane_id = tab.active_pane_id().unwrap();
            let pane = tab.pane_mut(pane_id).unwrap();
            pane.push_scrollback("some content");
        }
        let results = search_session(&session, "zzz_not_found", false);
        assert!(results.is_empty());
    }

    #[test]
    fn search_session_includes_floating_panes() {
        let mut session = Session::new("test", 80, 25);
        {
            let tab = session.active_tab_mut();
            let fp_id = tab.new_floating_pane();
            tab.floating_pane_mut(fp_id)
                .unwrap()
                .pane
                .push_scrollback("floating match");
        }
        let results = search_session(&session, "floating", false);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].matched_text, "floating");
    }

    #[test]
    fn search_session_multiple_tabs() {
        let mut session = Session::new("test", 80, 25);
        {
            let tab = session.active_tab_mut();
            let pid = tab.active_pane_id().unwrap();
            tab.pane_mut(pid)
                .unwrap()
                .push_scrollback("needle in tab 0");
        }
        session.new_tab("Tab 2");
        {
            let tab = session.active_tab_mut();
            let pid = tab.active_pane_id().unwrap();
            tab.pane_mut(pid)
                .unwrap()
                .push_scrollback("needle in tab 1");
        }
        let results = search_session(&session, "needle", false);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].tab_index, 0);
        assert_eq!(results[1].tab_index, 1);
    }

    #[test]
    fn search_session_regex() {
        let mut session = Session::new("test", 80, 25);
        {
            let tab = session.active_tab_mut();
            let pid = tab.active_pane_id().unwrap();
            tab.pane_mut(pid).unwrap().push_scrollback("error code 42");
            tab.pane_mut(pid).unwrap().push_scrollback("no numbers");
        }
        let results = search_session(&session, r"\d+", true);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].matched_text, "42");
    }
}
