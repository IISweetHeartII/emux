//! Scrollback search types and algorithms.
//!
//! The search engine supports plain-text and regex queries over the combined
//! scrollback + viewport buffer.  Results are represented as [`SearchMatch`]
//! values carrying an absolute row index (0 = oldest scrollback line) and
//! column offset.

use std::fmt;

/// A single search match in the combined scrollback + viewport buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    /// Absolute row (0 = oldest scrollback line).
    pub row: usize,
    /// Column offset (character position in the row text).
    pub col: usize,
    /// Length of the match in characters.
    pub len: usize,
}

/// Persistent search state attached to a screen.
#[derive(Debug, Clone, Default)]
pub struct SearchState {
    /// The query string (plain text or regex pattern).
    pub query: String,
    /// All matches found.
    pub matches: Vec<SearchMatch>,
    /// Index into `matches` of the "current" (active) match.
    pub current: Option<usize>,
    /// Whether the search is case-sensitive.
    pub case_sensitive: bool,
    /// Whether the query is a regex pattern.
    pub regex: bool,
}

/// Error type for search operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchError {
    /// The provided regex pattern was invalid.
    InvalidRegex(String),
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SearchError::InvalidRegex(msg) => write!(f, "invalid regex: {msg}"),
        }
    }
}

impl std::error::Error for SearchError {}

/// Find all plain-text matches of `query` across the given row texts.
///
/// Each entry in `texts` corresponds to one row (index = absolute row number).
/// Returns matches sorted by row then column.
pub fn find_all_matches(texts: &[String], query: &str, case_sensitive: bool) -> Vec<SearchMatch> {
    let mut matches = Vec::new();
    if query.is_empty() {
        return matches;
    }

    let query_cmp = if case_sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };

    for (row_idx, text) in texts.iter().enumerate() {
        let haystack = if case_sensitive {
            text.clone()
        } else {
            text.to_lowercase()
        };

        let mut start = 0;
        while let Some(pos) = haystack[start..].find(&query_cmp) {
            let byte_offset = start + pos;
            let col = haystack[..byte_offset].chars().count();
            let len = query_cmp.chars().count();
            matches.push(SearchMatch {
                row: row_idx,
                col,
                len,
            });
            start = byte_offset + query_cmp.len().max(1);
        }
    }
    matches
}

/// Find all regex matches of `pattern` across the given row texts.
pub fn find_all_matches_regex(
    texts: &[String],
    pattern: &str,
    case_sensitive: bool,
) -> Result<Vec<SearchMatch>, SearchError> {
    let full_pattern = if case_sensitive {
        pattern.to_string()
    } else {
        format!("(?i){pattern}")
    };
    let re =
        regex::Regex::new(&full_pattern).map_err(|e| SearchError::InvalidRegex(e.to_string()))?;

    let mut matches = Vec::new();
    for (row_idx, text) in texts.iter().enumerate() {
        for m in re.find_iter(text) {
            let col = text[..m.start()].chars().count();
            let len = m.as_str().chars().count();
            matches.push(SearchMatch {
                row: row_idx,
                col,
                len,
            });
        }
    }
    Ok(matches)
}

/// Advance to the next match index (wrapping).
pub fn next_match_index(current: Option<usize>, total: usize) -> Option<usize> {
    if total == 0 {
        return None;
    }
    Some(match current {
        Some(idx) => (idx + 1) % total,
        None => 0,
    })
}

/// Move to the previous match index (wrapping).
pub fn prev_match_index(current: Option<usize>, total: usize) -> Option<usize> {
    if total == 0 {
        return None;
    }
    Some(match current {
        Some(0) => total - 1,
        Some(idx) => idx - 1,
        None => total - 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_texts() -> Vec<String> {
        vec![
            "Hello World".into(),
            "hello again".into(),
            "nothing here".into(),
            "Hello Hello".into(),
            "".into(),
        ]
    }

    // ── Plain-text search ───────────────────────────────────────────

    #[test]
    fn find_case_sensitive() {
        let texts = sample_texts();
        let matches = find_all_matches(&texts, "Hello", true);
        // "Hello" appears in row 0 col 0, row 3 col 0, row 3 col 6
        assert_eq!(matches.len(), 3);
        assert_eq!(
            matches[0],
            SearchMatch {
                row: 0,
                col: 0,
                len: 5
            }
        );
        assert_eq!(
            matches[1],
            SearchMatch {
                row: 3,
                col: 0,
                len: 5
            }
        );
        assert_eq!(
            matches[2],
            SearchMatch {
                row: 3,
                col: 6,
                len: 5
            }
        );
    }

    #[test]
    fn find_case_insensitive() {
        let texts = sample_texts();
        let matches = find_all_matches(&texts, "hello", false);
        // Should find in rows 0, 1, 3 (twice)
        assert_eq!(matches.len(), 4);
        assert_eq!(matches[0].row, 0);
        assert_eq!(matches[1].row, 1);
        assert_eq!(matches[2].row, 3);
        assert_eq!(matches[3].row, 3);
    }

    #[test]
    fn find_empty_query_returns_nothing() {
        let texts = sample_texts();
        let matches = find_all_matches(&texts, "", true);
        assert!(matches.is_empty());
    }

    #[test]
    fn find_no_matches() {
        let texts = sample_texts();
        let matches = find_all_matches(&texts, "ZZZZZ", true);
        assert!(matches.is_empty());
    }

    #[test]
    fn find_in_empty_texts() {
        let texts: Vec<String> = vec![];
        let matches = find_all_matches(&texts, "hello", false);
        assert!(matches.is_empty());
    }

    #[test]
    fn find_overlapping_positions() {
        let texts = vec!["aaaa".into()];
        let matches = find_all_matches(&texts, "aa", true);
        // Non-overlapping: positions 0 and 2
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].col, 0);
        assert_eq!(matches[1].col, 2);
    }

    // ── Regex search ────────────────────────────────────────────────

    #[test]
    fn regex_basic() {
        let texts = sample_texts();
        let matches = find_all_matches_regex(&texts, r"[Hh]ello", true).unwrap();
        assert_eq!(matches.len(), 4); // row 0, 1, 3 (twice)
    }

    #[test]
    fn regex_case_insensitive() {
        let texts = vec!["ABC def".into(), "abc DEF".into()];
        let matches = find_all_matches_regex(&texts, "abc", false).unwrap();
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn regex_invalid_returns_error() {
        let texts = sample_texts();
        let result = find_all_matches_regex(&texts, "[invalid", true);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, SearchError::InvalidRegex(_)));
        assert!(err.to_string().contains("invalid regex"));
    }

    #[test]
    fn regex_digit_pattern() {
        let texts = vec![
            "line 42 here".into(),
            "no digits".into(),
            "port 8080 open".into(),
        ];
        let matches = find_all_matches_regex(&texts, r"\d+", true).unwrap();
        assert_eq!(matches.len(), 2);
        assert_eq!(
            matches[0],
            SearchMatch {
                row: 0,
                col: 5,
                len: 2
            }
        );
        assert_eq!(
            matches[1],
            SearchMatch {
                row: 2,
                col: 5,
                len: 4
            }
        );
    }

    // ── Navigation ──────────────────────────────────────────────────

    #[test]
    fn next_match_wraps() {
        assert_eq!(next_match_index(Some(2), 3), Some(0));
        assert_eq!(next_match_index(Some(0), 3), Some(1));
        assert_eq!(next_match_index(None, 3), Some(0));
        assert_eq!(next_match_index(None, 0), None);
    }

    #[test]
    fn prev_match_wraps() {
        assert_eq!(prev_match_index(Some(0), 3), Some(2));
        assert_eq!(prev_match_index(Some(2), 3), Some(1));
        assert_eq!(prev_match_index(None, 3), Some(2));
        assert_eq!(prev_match_index(None, 0), None);
    }

    // ── SearchState ─────────────────────────────────────────────────

    #[test]
    fn search_state_default() {
        let s = SearchState::default();
        assert!(s.query.is_empty());
        assert!(s.matches.is_empty());
        assert_eq!(s.current, None);
        assert!(!s.case_sensitive);
        assert!(!s.regex);
    }

    // ── SearchMatch ─────────────────────────────────────────────────

    #[test]
    fn search_match_equality() {
        let a = SearchMatch {
            row: 1,
            col: 2,
            len: 3,
        };
        let b = SearchMatch {
            row: 1,
            col: 2,
            len: 3,
        };
        let c = SearchMatch {
            row: 1,
            col: 2,
            len: 4,
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
