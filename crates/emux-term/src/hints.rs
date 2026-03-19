//! Pattern detection for smart selection (hint mode).
//!
//! Scans terminal buffer rows for common patterns — URLs, file paths, git SHAs,
//! IP addresses, email addresses, and bare numbers — and returns structured
//! [`HintMatch`] values that higher layers can use to offer quick-select /
//! hint-mode interaction.

use regex::Regex;
use std::sync::LazyLock;

use crate::grid::Row;

/// A detected pattern match in the terminal buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HintMatch {
    /// The matched text.
    pub text: String,
    /// What kind of pattern was matched.
    pub kind: HintKind,
    /// Row where the match starts (viewport-relative).
    pub start_row: usize,
    /// Column where the match starts.
    pub start_col: usize,
    /// Row where the match ends (viewport-relative).
    pub end_row: usize,
    /// Column where the match ends (inclusive).
    pub end_col: usize,
}

/// The kind of pattern detected by the hint engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintKind {
    /// A URL (http, https, ftp).
    Url,
    /// A filesystem path (absolute, relative, or home-relative).
    FilePath,
    /// A git SHA (7–40 hex characters).
    GitSha,
    /// An IPv4 address.
    IpAddress,
    /// An email address.
    Email,
    /// A bare number (integer).
    Number,
}

// ---------------------------------------------------------------------------
// Compiled regex patterns (compiled once via LazyLock)
// ---------------------------------------------------------------------------

static RE_URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https?://[^\s<>"'`\)\]]+|ftp://[^\s<>"'`\)\]]+"#).unwrap()
});

static RE_FILE_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:(?:[A-Za-z]:[/\\]|\\\\|\.[\\/]|~[\\/]|/)[A-Za-z0-9_.][A-Za-z0-9_.\\/\-]*)").unwrap()
});

static RE_GIT_SHA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[0-9a-f]{7,40}\b").unwrap()
});

static RE_IP_ADDRESS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b").unwrap()
});

static RE_EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b").unwrap()
});

// ---------------------------------------------------------------------------
// Single-line detection helpers
// ---------------------------------------------------------------------------

/// Find URLs in a single line of text.
///
/// Returns `(byte_start, byte_end, matched_text)` tuples.
pub fn detect_urls(text: &str) -> Vec<(usize, usize, String)> {
    RE_URL
        .find_iter(text)
        .map(|m| (m.start(), m.end(), m.as_str().to_string()))
        .collect()
}

/// Find file paths in a single line of text.
///
/// Recognises absolute (`/foo`), relative (`./foo`), and home-relative (`~/foo`)
/// paths.
pub fn detect_file_paths(text: &str) -> Vec<(usize, usize, String)> {
    RE_FILE_PATH
        .find_iter(text)
        .filter(|m| {
            let s = m.as_str();
            // Reject things that look like protocol prefixes (already caught by URL)
            !s.starts_with("//")
        })
        .map(|m| (m.start(), m.end(), m.as_str().to_string()))
        .collect()
}

/// Find git SHA-like hex strings (7–40 lowercase hex characters bounded by
/// word boundaries).
pub fn detect_git_shas(text: &str) -> Vec<(usize, usize, String)> {
    RE_GIT_SHA
        .find_iter(text)
        .filter(|m| {
            let s = m.as_str();
            // Must contain at least one digit AND at least one letter to
            // look like a real SHA (avoids matching pure numbers or pure
            // letter words).
            s.chars().any(|c| c.is_ascii_digit())
                && s.chars().any(|c| c.is_ascii_alphabetic())
        })
        .map(|m| (m.start(), m.end(), m.as_str().to_string()))
        .collect()
}

/// Find IPv4 addresses.
pub fn detect_ip_addresses(text: &str) -> Vec<(usize, usize, String)> {
    RE_IP_ADDRESS
        .find_iter(text)
        .map(|m| (m.start(), m.end(), m.as_str().to_string()))
        .collect()
}

/// Find email addresses.
pub fn detect_emails(text: &str) -> Vec<(usize, usize, String)> {
    RE_EMAIL
        .find_iter(text)
        .map(|m| (m.start(), m.end(), m.as_str().to_string()))
        .collect()
}

// ---------------------------------------------------------------------------
// Grid-level detection
// ---------------------------------------------------------------------------

/// Scan visible rows for all recognised patterns.
///
/// `rows` is a slice of [`Row`] (typically the viewport rows), and `cols` is
/// the grid width.  Returns matches sorted by position (row, then column).
pub fn detect_hints(rows: &[Row], cols: usize) -> Vec<HintMatch> {
    let mut matches: Vec<HintMatch> = Vec::new();

    for (row_idx, row) in rows.iter().enumerate() {
        // Build the text for this row (skipping continuation/wide-char spacer cells).
        let text = row_to_text(row, cols);

        // Helper: convert a byte offset in `text` to a column index.
        // Because we build `text` from the cell characters (one char per
        // non-zero-width cell), the char index *is* the column index.
        let byte_to_col = |byte_off: usize| -> usize {
            text[..byte_off].chars().count()
        };

        // URL (highest priority — detect before file paths so we don't
        // duplicate the path portion of a URL).
        let url_ranges: Vec<(usize, usize)> = detect_urls(&text)
            .iter()
            .map(|(s, e, _)| (*s, *e))
            .collect();

        for (start, end, t) in detect_urls(&text) {
            let sc = byte_to_col(start);
            let ec = byte_to_col(end).saturating_sub(1);
            matches.push(HintMatch {
                text: t,
                kind: HintKind::Url,
                start_row: row_idx,
                start_col: sc,
                end_row: row_idx,
                end_col: ec,
            });
        }

        // Email (detect before file paths to avoid partial overlap).
        let email_ranges: Vec<(usize, usize)> = detect_emails(&text)
            .iter()
            .map(|(s, e, _)| (*s, *e))
            .collect();

        for (start, end, t) in detect_emails(&text) {
            let sc = byte_to_col(start);
            let ec = byte_to_col(end).saturating_sub(1);
            matches.push(HintMatch {
                text: t,
                kind: HintKind::Email,
                start_row: row_idx,
                start_col: sc,
                end_row: row_idx,
                end_col: ec,
            });
        }

        // IP addresses.
        for (start, end, t) in detect_ip_addresses(&text) {
            if overlaps_any(start, end, &url_ranges) {
                continue;
            }
            let sc = byte_to_col(start);
            let ec = byte_to_col(end).saturating_sub(1);
            matches.push(HintMatch {
                text: t,
                kind: HintKind::IpAddress,
                start_row: row_idx,
                start_col: sc,
                end_row: row_idx,
                end_col: ec,
            });
        }

        // File paths (skip if inside a URL or email).
        for (start, end, t) in detect_file_paths(&text) {
            if overlaps_any(start, end, &url_ranges)
                || overlaps_any(start, end, &email_ranges)
            {
                continue;
            }
            let sc = byte_to_col(start);
            let ec = byte_to_col(end).saturating_sub(1);
            matches.push(HintMatch {
                text: t,
                kind: HintKind::FilePath,
                start_row: row_idx,
                start_col: sc,
                end_row: row_idx,
                end_col: ec,
            });
        }

        // Git SHAs (skip if inside a URL, email, or file path).
        let fp_ranges: Vec<(usize, usize)> = detect_file_paths(&text)
            .iter()
            .map(|(s, e, _)| (*s, *e))
            .collect();

        for (start, end, t) in detect_git_shas(&text) {
            if overlaps_any(start, end, &url_ranges)
                || overlaps_any(start, end, &email_ranges)
                || overlaps_any(start, end, &fp_ranges)
            {
                continue;
            }
            let sc = byte_to_col(start);
            let ec = byte_to_col(end).saturating_sub(1);
            matches.push(HintMatch {
                text: t,
                kind: HintKind::GitSha,
                start_row: row_idx,
                start_col: sc,
                end_row: row_idx,
                end_col: ec,
            });
        }
    }

    // Sort by (row, col).
    matches.sort_by(|a, b| {
        a.start_row
            .cmp(&b.start_row)
            .then(a.start_col.cmp(&b.start_col))
    });
    // Suppress duplicates at the same position (can happen if path regex
    // overlaps with something else at the exact same span).
    matches.dedup_by(|a, b| {
        a.start_row == b.start_row
            && a.start_col == b.start_col
            && a.end_row == b.end_row
            && a.end_col == b.end_col
    });
    matches
}

// ---------------------------------------------------------------------------
// Hint-label assignment
// ---------------------------------------------------------------------------

/// Assign single-character labels (a–z, then A–Z) to each match for hint mode.
///
/// Returns at most 52 labelled matches.  If there are more matches than labels,
/// the excess matches are unlabelled (not included in the result).
pub fn assign_labels(matches: &[HintMatch]) -> Vec<(char, &HintMatch)> {
    let labels = ('a'..='z').chain('A'..='Z');
    labels.zip(matches.iter()).collect()
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Build a plain-text string from a `Row`, skipping wide-char continuation
/// cells (width == 0).
fn row_to_text(row: &Row, cols: usize) -> String {
    let mut s = String::with_capacity(cols);
    for cell in &row.cells {
        if cell.width == 0 {
            continue;
        }
        s.push(cell.c);
    }
    // Trim trailing spaces.
    let trimmed = s.trim_end().len();
    s.truncate(trimmed);
    s
}

/// Check whether the range `[start, end)` overlaps with any range in `ranges`.
fn overlaps_any(start: usize, end: usize, ranges: &[(usize, usize)]) -> bool {
    ranges.iter().any(|&(rs, re)| start < re && end > rs)
}

// ─── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Row;

    /// Helper: build a `Row` from a plain-text string.
    fn row_from_str(text: &str, cols: usize) -> Row {
        let mut row = Row::new(cols);
        for (i, ch) in text.chars().enumerate() {
            if i < cols {
                row.cells[i].c = ch;
            }
        }
        row
    }

    // ── URL detection ─────────────────────────────────────────────────

    #[test]
    fn detect_url_http() {
        let hits = detect_urls("visit https://example.com for info");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].2, "https://example.com");
    }

    #[test]
    fn detect_url_https_with_path() {
        let hits = detect_urls("https://github.com/user/repo/issues/123");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].2, "https://github.com/user/repo/issues/123");
    }

    #[test]
    fn detect_url_with_query() {
        let hits = detect_urls("https://example.com/search?q=test&page=2");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].2, "https://example.com/search?q=test&page=2");
    }

    // ── File-path detection ───────────────────────────────────────────

    #[test]
    fn detect_file_path_absolute() {
        let hits = detect_file_paths("/usr/local/bin/emux");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].2, "/usr/local/bin/emux");
    }

    #[test]
    fn detect_file_path_relative() {
        let hits = detect_file_paths("./src/main.rs");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].2, "./src/main.rs");
    }

    #[test]
    fn detect_file_path_home() {
        let hits = detect_file_paths("~/projects/emux");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].2, "~/projects/emux");
    }

    // ── Git SHA detection ─────────────────────────────────────────────

    #[test]
    fn detect_git_sha_short() {
        let hits = detect_git_shas("commit abc1234 is good");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].2, "abc1234");
    }

    #[test]
    fn detect_git_sha_full() {
        let hits =
            detect_git_shas("sha abc1234567890abcdef1234567890abcdef1234 done");
        assert_eq!(hits.len(), 1);
        assert_eq!(
            hits[0].2,
            "abc1234567890abcdef1234567890abcdef1234"
        );
    }

    // ── IP address detection ──────────────────────────────────────────

    #[test]
    fn detect_ip_v4() {
        let hits = detect_ip_addresses("server at 192.168.1.100 ready");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].2, "192.168.1.100");
    }

    // ── Email detection ───────────────────────────────────────────────

    #[test]
    fn detect_email() {
        let hits = detect_emails("contact user@example.com for help");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].2, "user@example.com");
    }

    // ── Mixed / multiple-pattern detection ────────────────────────────

    #[test]
    fn detect_multiple_patterns() {
        // Use detect_hints (which handles overlap suppression) to verify
        // that a line with mixed patterns produces the right results.
        let cols = 80;
        let text =
            "see https://example.com and /tmp/log.txt commit abc1234 done";
        let rows = vec![row_from_str(text, cols)];
        let hints = detect_hints(&rows, cols);

        let kinds: Vec<HintKind> = hints.iter().map(|h| h.kind).collect();
        assert!(kinds.contains(&HintKind::Url));
        assert!(kinds.contains(&HintKind::FilePath));
        assert!(kinds.contains(&HintKind::GitSha));
        // URL, file path, and SHA — exactly one of each.
        assert_eq!(
            hints.iter().filter(|h| h.kind == HintKind::Url).count(),
            1
        );
        assert_eq!(
            hints.iter().filter(|h| h.kind == HintKind::FilePath).count(),
            1
        );
        assert_eq!(
            hints.iter().filter(|h| h.kind == HintKind::GitSha).count(),
            1
        );
    }

    // ── Label assignment ──────────────────────────────────────────────

    #[test]
    fn assign_labels_sequential() {
        let m1 = HintMatch {
            text: "https://a.com".into(),
            kind: HintKind::Url,
            start_row: 0,
            start_col: 0,
            end_row: 0,
            end_col: 12,
        };
        let m2 = HintMatch {
            text: "/tmp/foo".into(),
            kind: HintKind::FilePath,
            start_row: 1,
            start_col: 0,
            end_row: 1,
            end_col: 7,
        };
        let m3 = HintMatch {
            text: "abc1234".into(),
            kind: HintKind::GitSha,
            start_row: 2,
            start_col: 0,
            end_row: 2,
            end_col: 6,
        };
        let all = vec![m1, m2, m3];
        let labelled = assign_labels(&all);
        assert_eq!(labelled.len(), 3);
        assert_eq!(labelled[0].0, 'a');
        assert_eq!(labelled[1].0, 'b');
        assert_eq!(labelled[2].0, 'c');
    }

    // ── False-positive resistance ─────────────────────────────────────

    #[test]
    fn no_false_positives() {
        let text = "just some normal text with no special patterns here";
        assert!(detect_urls(text).is_empty());
        assert!(detect_file_paths(text).is_empty());
        assert!(detect_git_shas(text).is_empty());
        assert!(detect_ip_addresses(text).is_empty());
        assert!(detect_emails(text).is_empty());
    }

    // ── Grid-level integration ────────────────────────────────────────

    #[test]
    fn detect_hints_from_grid() {
        let cols = 80;
        let rows = vec![
            row_from_str("visit https://example.com for info", cols),
            row_from_str("edit /usr/local/bin/emux please", cols),
            row_from_str("commit abc1234 merged", cols),
            row_from_str("server 192.168.1.1 online", cols),
            row_from_str("mail user@example.com now", cols),
        ];
        let hints = detect_hints(&rows, cols);

        let kinds: Vec<HintKind> = hints.iter().map(|h| h.kind).collect();
        assert!(kinds.contains(&HintKind::Url));
        assert!(kinds.contains(&HintKind::FilePath));
        assert!(kinds.contains(&HintKind::GitSha));
        assert!(kinds.contains(&HintKind::IpAddress));
        assert!(kinds.contains(&HintKind::Email));

        // Verify each match has sensible row/col values.
        for h in &hints {
            assert!(h.start_col <= h.end_col || h.start_row < h.end_row);
            assert!(h.end_col < cols);
        }
    }
}
