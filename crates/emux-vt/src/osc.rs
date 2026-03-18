//! OSC (Operating System Command) types.

/// Actions from parsed OSC sequences.
#[derive(Debug, Clone, PartialEq)]
pub enum OscAction {
    /// Set window title (OSC 0 or OSC 2).
    SetTitle(String),
    /// Set icon name (OSC 1).
    SetIconName(String),
    /// Color query/set (OSC 4, 10, 11, etc.).
    Color { index: u16, value: String },
    /// Clipboard operation (OSC 52).
    Clipboard { clipboard: u8, data: String },
    /// Hyperlink (OSC 8).
    Hyperlink { uri: String, id: Option<String> },
    /// Current directory (OSC 7).
    CurrentDirectory(String),
    /// Semantic prompt (OSC 133).
    SemanticPrompt { kind: char },
    /// Unknown/unhandled OSC.
    Unknown(Vec<Vec<u8>>),
}
