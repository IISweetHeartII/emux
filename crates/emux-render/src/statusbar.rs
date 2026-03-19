//! Status bar renderer inspired by gpakosz/.tmux.
//!
//! Layout: `[session]▸[tab1 tab2 tab3]         [notifications][time][host]`
//! Uses Powerline-style separators when available.

use crossterm::style::{Attribute, Color as CtColor, ContentStyle};

/// Powerline separator characters (require patched font / Nerd Font).
pub const POWERLINE_RIGHT: char = '\u{E0B0}'; //
pub const POWERLINE_RIGHT_THIN: char = '\u{E0B1}'; //
pub const POWERLINE_LEFT: char = '\u{E0B2}'; //
pub const POWERLINE_LEFT_THIN: char = '\u{E0B3}'; //

/// Fallback separators when Powerline fonts are not available.
pub const FALLBACK_SEP: char = '│';

/// A styled segment of the status bar.
#[derive(Debug, Clone)]
pub struct Segment {
    pub text: String,
    pub fg: CtColor,
    pub bg: CtColor,
    pub bold: bool,
}

impl Segment {
    pub fn new(text: impl Into<String>, fg: CtColor, bg: CtColor) -> Self {
        Self {
            text: text.into(),
            fg,
            bg,
            bold: false,
        }
    }

    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }
}

/// Information needed to render the status bar.
#[derive(Debug, Clone)]
pub struct StatusBarInfo {
    /// Current session name.
    pub session_name: String,
    /// Tab names with active index.
    pub tabs: Vec<TabInfo>,
    /// Index of the currently active tab.
    pub active_tab: usize,
    /// Number of unread notifications.
    pub notification_count: usize,
    /// Hostname.
    pub hostname: String,
    /// Whether to use Powerline separators.
    pub powerline: bool,
}

/// Information about a single tab for status bar display.
#[derive(Debug, Clone)]
pub struct TabInfo {
    pub name: String,
    pub index: usize,
    pub has_notification: bool,
    pub pane_count: usize,
}

/// Color palette for the status bar (gpakosz/.tmux inspired).
#[derive(Debug, Clone)]
pub struct StatusBarTheme {
    /// Status bar background.
    pub bar_bg: CtColor,
    /// Session section: fg, bg.
    pub session_fg: CtColor,
    pub session_bg: CtColor,
    /// Active tab: fg, bg.
    pub active_tab_fg: CtColor,
    pub active_tab_bg: CtColor,
    /// Inactive tab: fg, bg.
    pub inactive_tab_fg: CtColor,
    pub inactive_tab_bg: CtColor,
    /// Tab with notification: fg.
    pub notify_tab_fg: CtColor,
    /// Right section 1 (notifications/info): fg, bg.
    pub right1_fg: CtColor,
    pub right1_bg: CtColor,
    /// Right section 2 (time): fg, bg.
    pub right2_fg: CtColor,
    pub right2_bg: CtColor,
    /// Right section 3 (host): fg, bg.
    pub right3_fg: CtColor,
    pub right3_bg: CtColor,
    /// Pane border: active, inactive.
    pub border_active: CtColor,
    pub border_inactive: CtColor,
}

impl Default for StatusBarTheme {
    fn default() -> Self {
        Self {
            bar_bg: CtColor::Rgb { r: 0x08, g: 0x08, b: 0x08 },
            // Session: dark on yellow (bold)
            session_fg: CtColor::Rgb { r: 0x08, g: 0x08, b: 0x08 },
            session_bg: CtColor::Rgb { r: 0xFF, g: 0xFF, b: 0x00 },
            // Active tab: dark on blue (bold)
            active_tab_fg: CtColor::Rgb { r: 0x08, g: 0x08, b: 0x08 },
            active_tab_bg: CtColor::Rgb { r: 0x00, g: 0xAF, b: 0xFF },
            // Inactive tab: gray on dark
            inactive_tab_fg: CtColor::Rgb { r: 0x8A, g: 0x8A, b: 0x8A },
            inactive_tab_bg: CtColor::Rgb { r: 0x08, g: 0x08, b: 0x08 },
            // Notification tab: yellow blink
            notify_tab_fg: CtColor::Rgb { r: 0xFF, g: 0xFF, b: 0x00 },
            // Right 1: gray on dark (info)
            right1_fg: CtColor::Rgb { r: 0x8A, g: 0x8A, b: 0x8A },
            right1_bg: CtColor::Rgb { r: 0x08, g: 0x08, b: 0x08 },
            // Right 2: white on red (time)
            right2_fg: CtColor::Rgb { r: 0xE4, g: 0xE4, b: 0xE4 },
            right2_bg: CtColor::Rgb { r: 0xD7, g: 0x00, b: 0x00 },
            // Right 3: dark on white (host)
            right3_fg: CtColor::Rgb { r: 0x08, g: 0x08, b: 0x08 },
            right3_bg: CtColor::Rgb { r: 0xE4, g: 0xE4, b: 0xE4 },
            // Borders
            border_active: CtColor::Rgb { r: 0x00, g: 0xAF, b: 0xFF },
            border_inactive: CtColor::Rgb { r: 0x30, g: 0x30, b: 0x30 },
        }
    }
}

/// Render the status bar into a list of (ContentStyle, String) spans.
///
/// The total width of all spans equals exactly `width` columns.
pub fn render_statusbar(info: &StatusBarInfo, theme: &StatusBarTheme, width: usize) -> Vec<(ContentStyle, String)> {
    let mut left_segments = Vec::new();
    let mut right_segments = Vec::new();

    // ── Left: session name ──
    left_segments.push(Segment::new(
        format!(" ❐ {} ", info.session_name),
        theme.session_fg,
        theme.session_bg,
    ).bold());

    // ── Left: tabs ──
    for tab in &info.tabs {
        let is_active = tab.index == info.active_tab;
        let (fg, bg) = if is_active {
            (theme.active_tab_fg, theme.active_tab_bg)
        } else if tab.has_notification {
            (theme.notify_tab_fg, theme.inactive_tab_bg)
        } else {
            (theme.inactive_tab_fg, theme.inactive_tab_bg)
        };

        let marker = if tab.has_notification { "●" } else { "" };
        let text = format!(" {}{} {} ", tab.index + 1, marker, tab.name);
        let seg = Segment::new(text, fg, bg);
        left_segments.push(if is_active { seg.bold() } else { seg });
    }

    // ── Right: notifications ──
    if info.notification_count > 0 {
        right_segments.push(Segment::new(
            format!(" !{} ", info.notification_count),
            theme.notify_tab_fg,
            theme.right1_bg,
        ));
    }

    // ── Right: time ──
    let now = chrono_free_time();
    right_segments.push(Segment::new(
        format!(" {} ", now),
        theme.right2_fg,
        theme.right2_bg,
    ));

    // ── Right: hostname ──
    right_segments.push(Segment::new(
        format!(" {} ", info.hostname),
        theme.right3_fg,
        theme.right3_bg,
    ).bold());

    // ── Assemble into spans ──
    assemble_bar(&left_segments, &right_segments, theme, width, info.powerline)
}

/// Assemble left and right segments into a full-width status bar.
fn assemble_bar(
    left: &[Segment],
    right: &[Segment],
    theme: &StatusBarTheme,
    width: usize,
    powerline: bool,
) -> Vec<(ContentStyle, String)> {
    let mut spans = Vec::new();

    // Render left segments with separators
    for (i, seg) in left.iter().enumerate() {
        let mut style = ContentStyle::new();
        style.foreground_color = Some(seg.fg);
        style.background_color = Some(seg.bg);
        if seg.bold {
            style.attributes.set(Attribute::Bold);
        }
        spans.push((style, seg.text.clone()));

        // Separator between segments
        if powerline && i + 1 < left.len() {
            let next_bg = left[i + 1].bg;
            let mut sep_style = ContentStyle::new();
            sep_style.foreground_color = Some(seg.bg);
            sep_style.background_color = Some(next_bg);
            spans.push((sep_style, POWERLINE_RIGHT.to_string()));
        }
    }

    // Separator after last left segment to bar background
    if powerline && !left.is_empty() {
        let last_bg = left.last().unwrap().bg;
        let mut sep_style = ContentStyle::new();
        sep_style.foreground_color = Some(last_bg);
        sep_style.background_color = Some(theme.bar_bg);
        spans.push((sep_style, POWERLINE_RIGHT.to_string()));
    }

    // Calculate widths
    let left_width: usize = left.iter().map(|s| display_width(&s.text)).sum::<usize>()
        + if powerline { left.len() } else { 0 }; // separators
    let right_width: usize = right.iter().map(|s| display_width(&s.text)).sum::<usize>()
        + if powerline { right.len() } else { 0 };

    let fill = width.saturating_sub(left_width + right_width);

    // Fill middle with bar background
    if fill > 0 {
        let mut fill_style = ContentStyle::new();
        fill_style.background_color = Some(theme.bar_bg);
        fill_style.foreground_color = Some(theme.bar_bg);
        spans.push((fill_style, " ".repeat(fill)));
    }

    // Render right segments with separators
    for (i, seg) in right.iter().enumerate() {
        // Separator before segment
        if powerline {
            let prev_bg = if i == 0 { theme.bar_bg } else { right[i - 1].bg };
            let mut sep_style = ContentStyle::new();
            sep_style.foreground_color = Some(seg.bg);
            sep_style.background_color = Some(prev_bg);
            spans.push((sep_style, POWERLINE_LEFT.to_string()));
        }

        let mut style = ContentStyle::new();
        style.foreground_color = Some(seg.fg);
        style.background_color = Some(seg.bg);
        if seg.bold {
            style.attributes.set(Attribute::Bold);
        }
        spans.push((style, seg.text.clone()));
    }

    spans
}

/// Simple time formatting without chrono dependency.
fn chrono_free_time() -> String {
    // Use a fixed format via libc or fallback
    #[cfg(unix)]
    {
        use std::ffi::CStr;
        unsafe {
            let mut t: libc::time_t = 0;
            libc::time(&mut t);
            let tm = libc::localtime(&t);
            if tm.is_null() {
                return "??:??".into();
            }
            let mut buf = [0u8; 32];
            let fmt = b"%H:%M\0";
            let len = libc::strftime(
                buf.as_mut_ptr() as *mut libc::c_char,
                buf.len(),
                fmt.as_ptr() as *const libc::c_char,
                tm,
            );
            if len == 0 {
                return "??:??".into();
            }
            CStr::from_ptr(buf.as_ptr() as *const libc::c_char)
                .to_string_lossy()
                .into_owned()
        }
    }
    #[cfg(not(unix))]
    {
        use std::time::SystemTime;
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let hours = (now % 86400) / 3600;
        let minutes = (now % 3600) / 60;
        format!("{:02}:{:02}", hours, minutes)
    }
}

/// Approximate display width of a string, accounting for wide characters.
fn display_width(s: &str) -> usize {
    s.chars()
        .map(|c| {
            if c.is_ascii() {
                1
            } else if ('\u{1100}'..='\u{115F}').contains(&c) // Hangul Jamo
                || ('\u{2E80}'..='\u{303E}').contains(&c)    // CJK Radicals / Kangxi
                || ('\u{3040}'..='\u{33BF}').contains(&c)    // Japanese Hiragana/Katakana
                || ('\u{3400}'..='\u{4DBF}').contains(&c)    // CJK Unified Ext A
                || ('\u{4E00}'..='\u{9FFF}').contains(&c)    // CJK Unified Ideographs
                || ('\u{AC00}'..='\u{D7AF}').contains(&c)    // Hangul Syllables
                || ('\u{F900}'..='\u{FAFF}').contains(&c)    // CJK Compatibility Ideographs
                || ('\u{FE30}'..='\u{FE6F}').contains(&c)    // CJK Compatibility Forms
                || ('\u{FF01}'..='\u{FF60}').contains(&c)    // Fullwidth Forms
                || ('\u{1F000}'..='\u{1FFFF}').contains(&c)  // Emoji and symbols
                || ('\u{20000}'..='\u{2FFFF}').contains(&c)  // CJK Unified Ext B+
            {
                2
            } else {
                1
            }
        })
        .sum()
}

/// Render a horizontal pane border line.
pub fn render_border(width: usize, active: bool, theme: &StatusBarTheme) -> Vec<(ContentStyle, String)> {
    let color = if active { theme.border_active } else { theme.border_inactive };
    let mut style = ContentStyle::new();
    style.foreground_color = Some(color);
    vec![(style, "─".repeat(width))]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_info() -> StatusBarInfo {
        StatusBarInfo {
            session_name: "dev".into(),
            tabs: vec![
                TabInfo { name: "bash".into(), index: 0, has_notification: false, pane_count: 1 },
                TabInfo { name: "vim".into(), index: 1, has_notification: false, pane_count: 1 },
                TabInfo { name: "htop".into(), index: 2, has_notification: true, pane_count: 2 },
            ],
            active_tab: 1,
            notification_count: 1,
            hostname: "myhost".into(),
            powerline: false,
        }
    }

    #[test]
    fn statusbar_renders_to_exact_width() {
        let info = sample_info();
        let theme = StatusBarTheme::default();
        let spans = render_statusbar(&info, &theme, 120);
        let total: usize = spans.iter().map(|(_, t)| display_width(t)).sum();
        assert_eq!(total, 120);
    }

    #[test]
    fn statusbar_contains_session_name() {
        let info = sample_info();
        let theme = StatusBarTheme::default();
        let spans = render_statusbar(&info, &theme, 120);
        let text: String = spans.iter().map(|(_, t)| t.as_str()).collect();
        assert!(text.contains("dev"));
    }

    #[test]
    fn statusbar_contains_active_tab() {
        let info = sample_info();
        let theme = StatusBarTheme::default();
        let spans = render_statusbar(&info, &theme, 120);
        let text: String = spans.iter().map(|(_, t)| t.as_str()).collect();
        assert!(text.contains("vim"));
    }

    #[test]
    fn statusbar_contains_hostname() {
        let info = sample_info();
        let theme = StatusBarTheme::default();
        let spans = render_statusbar(&info, &theme, 120);
        let text: String = spans.iter().map(|(_, t)| t.as_str()).collect();
        assert!(text.contains("myhost"));
    }

    #[test]
    fn statusbar_shows_notification_indicator() {
        let info = sample_info();
        let theme = StatusBarTheme::default();
        let spans = render_statusbar(&info, &theme, 120);
        let text: String = spans.iter().map(|(_, t)| t.as_str()).collect();
        // Notification tab should have the dot marker
        assert!(text.contains("●"));
        // Notification count
        assert!(text.contains("!1"));
    }

    #[test]
    fn statusbar_no_notification_when_zero() {
        let mut info = sample_info();
        info.notification_count = 0;
        info.tabs[2].has_notification = false;
        let theme = StatusBarTheme::default();
        let spans = render_statusbar(&info, &theme, 120);
        let text: String = spans.iter().map(|(_, t)| t.as_str()).collect();
        // The notification segment " !N " should not be present
        assert!(!text.contains(" !"));
    }

    #[test]
    fn statusbar_powerline_separators() {
        let mut info = sample_info();
        info.powerline = true;
        let theme = StatusBarTheme::default();
        let spans = render_statusbar(&info, &theme, 120);
        let text: String = spans.iter().map(|(_, t)| t.as_str()).collect();
        assert!(text.contains(POWERLINE_RIGHT) || text.contains(POWERLINE_LEFT));
    }

    #[test]
    fn statusbar_narrow_width_does_not_panic() {
        let info = sample_info();
        let theme = StatusBarTheme::default();
        // Very narrow — should not panic
        let spans = render_statusbar(&info, &theme, 20);
        assert!(!spans.is_empty());
    }

    #[test]
    fn border_renders_correct_width() {
        let theme = StatusBarTheme::default();
        let spans = render_border(80, true, &theme);
        let total: usize = spans.iter().map(|(_, t)| t.chars().count()).sum();
        assert_eq!(total, 80);
    }

    #[test]
    fn border_active_uses_active_color() {
        let theme = StatusBarTheme::default();
        let spans = render_border(10, true, &theme);
        assert_eq!(spans[0].0.foreground_color, Some(theme.border_active));
    }

    #[test]
    fn border_inactive_uses_inactive_color() {
        let theme = StatusBarTheme::default();
        let spans = render_border(10, false, &theme);
        assert_eq!(spans[0].0.foreground_color, Some(theme.border_inactive));
    }

    #[test]
    fn segment_bold_flag() {
        let seg = Segment::new("test", CtColor::White, CtColor::Black).bold();
        assert!(seg.bold);
    }

    #[test]
    fn display_width_basic() {
        assert_eq!(display_width("hello"), 5);
        assert_eq!(display_width(""), 0);
    }

    #[test]
    fn display_width_wide_chars() {
        // Emoji (U+1F514 BELL) should be 2 columns wide
        assert_eq!(display_width("\u{1F514}"), 2);
        // CJK ideograph should be 2 columns wide
        assert_eq!(display_width("\u{4E2D}"), 2);
        // Mixed ASCII and wide
        assert_eq!(display_width("A\u{4E2D}B"), 4);
    }

    #[test]
    fn default_theme_has_distinct_colors() {
        let theme = StatusBarTheme::default();
        assert_ne!(theme.session_bg, theme.active_tab_bg);
        assert_ne!(theme.active_tab_bg, theme.inactive_tab_bg);
        assert_ne!(theme.right2_bg, theme.right3_bg);
    }
}
