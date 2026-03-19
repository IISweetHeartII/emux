//! TOML-based swap layout configuration parsing.

use std::fmt;

use serde::Deserialize;

use crate::layout::{LayoutNode, SplitDirection};
use crate::tab::SwapLayout;

/// Errors that can occur when parsing swap layout TOML.
#[derive(Debug)]
pub enum LayoutParseError {
    Toml(toml::de::Error),
    InvalidPaneCount { value: usize },
    UnknownTemplate(String),
    InvalidRatio(String),
}

impl fmt::Display for LayoutParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayoutParseError::Toml(e) => write!(f, "TOML parse error: {e}"),
            LayoutParseError::InvalidPaneCount { value } => {
                write!(f, "invalid pane_count: {value}")
            }
            LayoutParseError::UnknownTemplate(t) => write!(f, "unknown template: {t}"),
            LayoutParseError::InvalidRatio(r) => write!(f, "invalid ratio: {r}"),
        }
    }
}

impl std::error::Error for LayoutParseError {}

impl From<toml::de::Error> for LayoutParseError {
    fn from(e: toml::de::Error) -> Self {
        LayoutParseError::Toml(e)
    }
}

/// Raw TOML representation of a swap layout entry.
#[derive(Debug, Deserialize)]
struct RawSwapLayout {
    name: Option<String>,
    pane_count: Option<usize>,
    min_panes: Option<usize>,
    max_panes: Option<usize>,
    template: Option<String>,
    direction: Option<String>,
    ratio: Option<f64>,
    splits: Option<Vec<String>>,
}

/// Top-level TOML document containing swap layouts.
#[derive(Debug, Deserialize)]
struct RawSwapLayoutDoc {
    swap_layout: Vec<RawSwapLayout>,
}

/// Parse a TOML string containing swap layout definitions.
///
/// Supported format:
/// ```toml
/// [[swap_layout]]
/// name = "side-by-side"
/// pane_count = 2
/// direction = "horizontal"
/// ratio = 0.5
///
/// [[swap_layout]]
/// name = "weighted"
/// pane_count = 2
/// direction = "vertical"
/// splits = ["30%", "70%"]
/// ```
pub fn parse_swap_layout_toml(toml_str: &str) -> Result<Vec<SwapLayout>, LayoutParseError> {
    let doc: RawSwapLayoutDoc = toml::from_str(toml_str)?;
    let mut layouts = Vec::new();

    for (i, raw) in doc.swap_layout.into_iter().enumerate() {
        // Determine pane count range
        let min_panes = raw.min_panes.or(raw.pane_count);
        let max_panes = raw.max_panes.or(raw.pane_count);

        // Validate pane count
        if let Some(count) = raw.pane_count
            && count == 0
        {
            return Err(LayoutParseError::InvalidPaneCount { value: 0 });
        }
        if let Some(min) = min_panes
            && min == 0
        {
            return Err(LayoutParseError::InvalidPaneCount { value: 0 });
        }

        let name = raw.name.unwrap_or_else(|| format!("layout-{i}"));

        // Determine the direction
        let direction = match raw.direction.as_deref() {
            Some("horizontal") => SplitDirection::Horizontal,
            Some("vertical") => SplitDirection::Vertical,
            None => SplitDirection::Vertical, // default
            Some(other) => {
                return Err(LayoutParseError::UnknownTemplate(other.to_string()));
            }
        };

        // Determine ratio from explicit ratio or from splits
        let ratio = if let Some(ref splits) = raw.splits {
            parse_ratio_from_splits(splits)?
        } else if let Some(r) = raw.ratio {
            r as f32
        } else if let Some(ref tmpl) = raw.template {
            // Named template shortcuts
            match tmpl.as_str() {
                "vsplit" | "hsplit" => 0.5,
                _ => return Err(LayoutParseError::UnknownTemplate(tmpl.clone())),
            }
        } else {
            0.5
        };

        // Determine actual pane count for building the template tree
        let pane_count = raw.pane_count.or(min_panes).unwrap_or(2);

        // Build the layout node
        let layout = build_layout_node(pane_count, direction, ratio);

        layouts.push(SwapLayout {
            name,
            min_panes,
            max_panes,
            layout,
        });
    }

    Ok(layouts)
}

/// Parse a ratio from a splits array like `["30%", "70%"]`.
fn parse_ratio_from_splits(splits: &[String]) -> Result<f32, LayoutParseError> {
    if splits.len() != 2 {
        return Err(LayoutParseError::InvalidRatio(format!(
            "expected exactly 2 splits, got {}",
            splits.len()
        )));
    }
    let first = parse_percentage(&splits[0])?;
    let _second = parse_percentage(&splits[1])?;
    Ok(first)
}

/// Parse a percentage string like "30%" into a float (0.30).
fn parse_percentage(s: &str) -> Result<f32, LayoutParseError> {
    let s = s.trim();
    if let Some(num_str) = s.strip_suffix('%') {
        num_str
            .trim()
            .parse::<f32>()
            .map(|v| v / 100.0)
            .map_err(|_| LayoutParseError::InvalidRatio(s.to_string()))
    } else {
        Err(LayoutParseError::InvalidRatio(s.to_string()))
    }
}

/// Build a layout node tree for the given pane count, using a binary split
/// with the given direction and ratio for the first split.
fn build_layout_node(pane_count: usize, direction: SplitDirection, ratio: f32) -> LayoutNode {
    if pane_count <= 1 {
        LayoutNode::Leaf(100)
    } else if pane_count == 2 {
        LayoutNode::Split {
            direction,
            ratio,
            first: Box::new(LayoutNode::Leaf(100)),
            second: Box::new(LayoutNode::Leaf(101)),
        }
    } else {
        // Build a chain: first leaf, then recursively split the rest
        let mut node = LayoutNode::Leaf(100);
        for i in 1..pane_count {
            let leaf_id = 100 + i as u32;
            node = LayoutNode::Split {
                direction,
                ratio: i as f32 / (i as f32 + 1.0),
                first: Box::new(node),
                second: Box::new(LayoutNode::Leaf(leaf_id)),
            };
        }
        node
    }
}
