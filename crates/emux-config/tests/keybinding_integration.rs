//! Integration tests: Config loading + keybinding parsing.
//!
//! These tests verify that configuration loading produces valid, parseable
//! keybinding strings, and that the keybinding format is consistent.

use emux_config::{Config, KeyBindings, merge_with_defaults};

// ---------------------------------------------------------------------------
// 1. Default config keybinding format validation
// ---------------------------------------------------------------------------

#[test]
fn default_keybindings_have_valid_format() {
    let keys = KeyBindings::default();
    let all_bindings = collect_bindings(&keys);

    for (name, binding) in &all_bindings {
        // Every keybinding should have at least one component
        assert!(
            !binding.is_empty(),
            "keybinding '{name}' should not be empty"
        );

        // Parse the binding into components (split by '+')
        let parts: Vec<&str> = binding.split('+').collect();
        assert!(
            !parts.is_empty(),
            "keybinding '{name}' = '{binding}' should have at least one part"
        );

        // The last part should be a key name (not a modifier)
        let key_part = parts.last().unwrap();
        assert!(
            !key_part.is_empty(),
            "keybinding '{name}' = '{binding}' has empty key part"
        );

        // Verify modifier parts are recognized
        for part in &parts[..parts.len() - 1] {
            assert!(
                is_known_modifier(part),
                "keybinding '{name}' = '{binding}' has unknown modifier '{part}'"
            );
        }
    }
}

#[test]
fn default_keybindings_all_use_leader_prefix() {
    let keys = KeyBindings::default();
    let all_bindings = collect_bindings(&keys);

    for (name, binding) in &all_bindings {
        assert!(
            binding.starts_with("Leader+"),
            "default keybinding '{name}' = '{binding}' should start with 'Leader+'"
        );
    }
}

#[test]
fn default_keybindings_have_unique_key_parts() {
    let keys = KeyBindings::default();
    let all_bindings = collect_bindings(&keys);

    // Verify no two bindings map to the same key combination
    let mut seen = std::collections::HashSet::new();
    for (name, binding) in &all_bindings {
        assert!(
            seen.insert(*binding),
            "keybinding '{name}' = '{binding}' is a duplicate"
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Config loading roundtrip with keybindings
// ---------------------------------------------------------------------------

#[test]
fn config_roundtrip_preserves_keybindings() {
    let original = Config::default();
    let serialized = toml::to_string(&original).expect("config should serialize");
    let deserialized: Config = toml::from_str(&serialized).expect("config should deserialize");

    let orig_bindings = collect_bindings(&original.keys);
    let deser_bindings = collect_bindings(&deserialized.keys);

    assert_eq!(orig_bindings.len(), deser_bindings.len());
    for ((name_a, val_a), (name_b, val_b)) in orig_bindings.iter().zip(deser_bindings.iter()) {
        assert_eq!(name_a, name_b);
        assert_eq!(
            val_a, val_b,
            "keybinding '{name_a}' changed after roundtrip: '{val_a}' != '{val_b}'"
        );
    }
}

#[test]
fn custom_keybindings_parse_correctly() {
    let partial = r##"
[keys]
split_down = "Ctrl+Shift+D"
split_right = "Ctrl+Shift+R"
close_pane = "Alt+X"
focus_up = "Ctrl+Up"
focus_down = "Ctrl+Down"
focus_left = "Ctrl+Left"
focus_right = "Ctrl+Right"
new_tab = "Ctrl+T"
close_tab = "Ctrl+W"
next_tab = "Ctrl+Tab"
prev_tab = "Ctrl+Shift+Tab"
detach = "Ctrl+Shift+Q"
search = "Ctrl+F"
toggle_fullscreen = "F11"
toggle_float = "Ctrl+Shift+G"
"##;
    let value: toml::Value = toml::from_str(partial.trim()).unwrap();
    let cfg = merge_with_defaults(value);

    // Verify custom bindings
    assert_eq!(cfg.keys.split_down, "Ctrl+Shift+D");
    assert_eq!(cfg.keys.close_pane, "Alt+X");
    assert_eq!(cfg.keys.toggle_fullscreen, "F11");

    // Verify the custom bindings still have valid format
    let all_bindings = collect_bindings(&cfg.keys);
    for (name, binding) in &all_bindings {
        let parts: Vec<&str> = binding.split('+').collect();
        assert!(
            !parts.is_empty(),
            "custom keybinding '{name}' = '{binding}' should have parts"
        );
    }
}

#[test]
fn partial_keybinding_override_preserves_other_defaults() {
    let partial = r##"
[keys]
split_down = "Ctrl+D"
"##;
    let value: toml::Value = toml::from_str(partial.trim()).unwrap();
    let cfg = merge_with_defaults(value);

    // Overridden
    assert_eq!(cfg.keys.split_down, "Ctrl+D");

    // All others should be defaults
    let default_keys = KeyBindings::default();
    assert_eq!(cfg.keys.split_right, default_keys.split_right);
    assert_eq!(cfg.keys.close_pane, default_keys.close_pane);
    assert_eq!(cfg.keys.focus_up, default_keys.focus_up);
    assert_eq!(cfg.keys.focus_down, default_keys.focus_down);
    assert_eq!(cfg.keys.focus_left, default_keys.focus_left);
    assert_eq!(cfg.keys.focus_right, default_keys.focus_right);
    assert_eq!(cfg.keys.new_tab, default_keys.new_tab);
    assert_eq!(cfg.keys.close_tab, default_keys.close_tab);
    assert_eq!(cfg.keys.next_tab, default_keys.next_tab);
    assert_eq!(cfg.keys.prev_tab, default_keys.prev_tab);
    assert_eq!(cfg.keys.detach, default_keys.detach);
    assert_eq!(cfg.keys.search, default_keys.search);
    assert_eq!(cfg.keys.toggle_fullscreen, default_keys.toggle_fullscreen);
    assert_eq!(cfg.keys.toggle_float, default_keys.toggle_float);
}

#[test]
fn config_default_loads_without_file() {
    // load_config without a file should return defaults.
    // We can test merge_with_defaults with an empty table to simulate this.
    let empty = toml::Value::Table(Default::default());
    let cfg = merge_with_defaults(empty);

    let default_cfg = Config::default();
    assert_eq!(cfg.font_size, default_cfg.font_size);
    assert_eq!(cfg.keys.split_down, default_cfg.keys.split_down);
    assert_eq!(cfg.keys.new_tab, default_cfg.keys.new_tab);
    assert_eq!(cfg.theme.background, default_cfg.theme.background);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn collect_bindings(keys: &KeyBindings) -> Vec<(&'static str, &str)> {
    vec![
        ("split_down", keys.split_down.as_str()),
        ("split_right", keys.split_right.as_str()),
        ("close_pane", keys.close_pane.as_str()),
        ("focus_up", keys.focus_up.as_str()),
        ("focus_down", keys.focus_down.as_str()),
        ("focus_left", keys.focus_left.as_str()),
        ("focus_right", keys.focus_right.as_str()),
        ("new_tab", keys.new_tab.as_str()),
        ("close_tab", keys.close_tab.as_str()),
        ("next_tab", keys.next_tab.as_str()),
        ("prev_tab", keys.prev_tab.as_str()),
        ("detach", keys.detach.as_str()),
        ("search", keys.search.as_str()),
        ("toggle_fullscreen", keys.toggle_fullscreen.as_str()),
        ("toggle_float", keys.toggle_float.as_str()),
    ]
}

fn is_known_modifier(s: &str) -> bool {
    matches!(
        s,
        "Leader" | "Ctrl" | "Alt" | "Shift" | "Super" | "Meta" | "Cmd"
    )
}
