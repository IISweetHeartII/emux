//! Project layout templates loaded from `.emux.toml` files.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A project layout template loaded from `.emux.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutTemplate {
    /// Template name.
    pub name: String,
    /// Pane definitions.
    pub panes: Vec<PaneTemplate>,
}

/// A pane definition in a layout template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneTemplate {
    /// Startup command to run in this pane.
    #[serde(default)]
    pub command: Option<String>,
    /// Working directory (relative to project root).
    #[serde(default)]
    pub cwd: Option<String>,
    /// Split direction from previous pane.
    #[serde(default = "default_split")]
    pub split: SplitDir,
    /// Size as percentage (0-100) or fixed columns/rows.
    #[serde(default)]
    pub size: Option<u16>,
    /// Pane title.
    #[serde(default)]
    pub title: Option<String>,
}

/// Split direction for a pane in a layout template.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SplitDir {
    /// Split horizontally: panes stacked top/bottom.
    #[default]
    Horizontal,
    /// Split vertically: panes side by side.
    Vertical,
}

fn default_split() -> SplitDir {
    SplitDir::Horizontal
}

/// The filename used for project layout templates.
const TEMPLATE_FILENAME: &str = ".emux.toml";

/// Load and parse a layout template from the given path.
pub fn load_template(path: &Path) -> Result<LayoutTemplate, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    parse_template(&content)
}

/// Parse a layout template from a TOML string.
pub fn parse_template(toml_str: &str) -> Result<LayoutTemplate, String> {
    toml::from_str(toml_str).map_err(|e| format!("TOML parse error: {e}"))
}

/// Walk up from `start_dir` looking for a `.emux.toml` file.
/// Returns the path and parsed template if found.
pub fn find_project_template(start_dir: &Path) -> Option<(PathBuf, LayoutTemplate)> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let candidate = dir.join(TEMPLATE_FILENAME);
        if candidate.is_file() {
            if let Ok(template) = load_template(&candidate) {
                return Some((candidate, template));
            }
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Save a layout template to the given path as TOML.
pub fn save_template(template: &LayoutTemplate, path: &Path) -> Result<(), String> {
    let content =
        toml::to_string_pretty(template).map_err(|e| format!("serialization error: {e}"))?;
    std::fs::write(path, content).map_err(|e| format!("failed to write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_basic_template() {
        let toml = r#"
name = "web-dev"

[[panes]]
command = "nvim ."
title = "editor"

[[panes]]
command = "cargo watch -x test"
split = "horizontal"
size = 30
title = "tests"

[[panes]]
command = "git log --oneline -20"
split = "vertical"
title = "git"
"#;
        let t = parse_template(toml).unwrap();
        assert_eq!(t.name, "web-dev");
        assert_eq!(t.panes.len(), 3);
        assert_eq!(t.panes[0].command.as_deref(), Some("nvim ."));
        assert_eq!(t.panes[0].title.as_deref(), Some("editor"));
        assert_eq!(t.panes[1].split, SplitDir::Horizontal);
        assert_eq!(t.panes[1].size, Some(30));
        assert_eq!(t.panes[2].split, SplitDir::Vertical);
    }

    #[test]
    fn parse_template_with_defaults() {
        let toml = r#"
name = "minimal"

[[panes]]
"#;
        let t = parse_template(toml).unwrap();
        assert_eq!(t.panes.len(), 1);
        let p = &t.panes[0];
        assert!(p.command.is_none());
        assert!(p.cwd.is_none());
        assert_eq!(p.split, SplitDir::Horizontal);
        assert!(p.size.is_none());
        assert!(p.title.is_none());
    }

    #[test]
    fn parse_template_minimal() {
        let toml = r#"
name = "bare"

[[panes]]
command = "bash"
"#;
        let t = parse_template(toml).unwrap();
        assert_eq!(t.name, "bare");
        assert_eq!(t.panes.len(), 1);
        assert_eq!(t.panes[0].command.as_deref(), Some("bash"));
    }

    #[test]
    fn parse_template_all_fields() {
        let toml = r#"
name = "full"

[[panes]]
command = "vim"
cwd = "src"
split = "vertical"
size = 60
title = "code"
"#;
        let t = parse_template(toml).unwrap();
        let p = &t.panes[0];
        assert_eq!(p.command.as_deref(), Some("vim"));
        assert_eq!(p.cwd.as_deref(), Some("src"));
        assert_eq!(p.split, SplitDir::Vertical);
        assert_eq!(p.size, Some(60));
        assert_eq!(p.title.as_deref(), Some("code"));
    }

    #[test]
    fn serialize_roundtrip() {
        let template = LayoutTemplate {
            name: "roundtrip".into(),
            panes: vec![
                PaneTemplate {
                    command: Some("echo hi".into()),
                    cwd: Some("subdir".into()),
                    split: SplitDir::Vertical,
                    size: Some(50),
                    title: Some("hello".into()),
                },
                PaneTemplate {
                    command: None,
                    cwd: None,
                    split: SplitDir::Horizontal,
                    size: None,
                    title: None,
                },
            ],
        };
        let serialized = toml::to_string_pretty(&template).unwrap();
        let deserialized: LayoutTemplate = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.name, template.name);
        assert_eq!(deserialized.panes.len(), template.panes.len());
        assert_eq!(deserialized.panes[0].command, template.panes[0].command);
        assert_eq!(deserialized.panes[0].split, template.panes[0].split);
        assert_eq!(deserialized.panes[1].command, template.panes[1].command);
    }

    #[test]
    fn find_template_in_current_dir() {
        let dir = tempfile::tempdir().unwrap();
        let toml_path = dir.path().join(".emux.toml");
        fs::write(
            &toml_path,
            "name = \"found\"\n\n[[panes]]\ncommand = \"ls\"\n",
        )
        .unwrap();

        let result = find_project_template(dir.path());
        assert!(result.is_some());
        let (path, t) = result.unwrap();
        assert_eq!(path, toml_path);
        assert_eq!(t.name, "found");
    }

    #[test]
    fn find_template_walks_up() {
        let parent = tempfile::tempdir().unwrap();
        let child = parent.path().join("sub").join("deep");
        fs::create_dir_all(&child).unwrap();
        fs::write(
            parent.path().join(".emux.toml"),
            "name = \"parent\"\n\n[[panes]]\ncommand = \"pwd\"\n",
        )
        .unwrap();

        let result = find_project_template(&child);
        assert!(result.is_some());
        let (_, t) = result.unwrap();
        assert_eq!(t.name, "parent");
    }

    #[test]
    fn find_template_not_found() {
        let dir = tempfile::tempdir().unwrap();
        // No .emux.toml anywhere in this temp directory tree
        let result = find_project_template(dir.path());
        assert!(result.is_none());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".emux.toml");

        let template = LayoutTemplate {
            name: "saved".into(),
            panes: vec![PaneTemplate {
                command: Some("make build".into()),
                cwd: Some("project".into()),
                split: SplitDir::Horizontal,
                size: Some(40),
                title: Some("build".into()),
            }],
        };

        save_template(&template, &path).unwrap();
        let loaded = load_template(&path).unwrap();

        assert_eq!(loaded.name, "saved");
        assert_eq!(loaded.panes.len(), 1);
        assert_eq!(loaded.panes[0].command.as_deref(), Some("make build"));
        assert_eq!(loaded.panes[0].cwd.as_deref(), Some("project"));
        assert_eq!(loaded.panes[0].size, Some(40));
    }

    #[test]
    fn split_dir_deserialization() {
        #[derive(Deserialize)]
        struct Wrapper {
            dir: SplitDir,
        }

        let h: Wrapper = toml::from_str("dir = \"horizontal\"").unwrap();
        assert_eq!(h.dir, SplitDir::Horizontal);

        let v: Wrapper = toml::from_str("dir = \"vertical\"").unwrap();
        assert_eq!(v.dir, SplitDir::Vertical);
    }
}
