use std::io::Write;
use std::thread;
use std::time::Duration;

use emux_config::{Config, ConfigWatcher, KeyBindings, Theme, load_from_path, merge_with_defaults};

#[test]
fn default_theme_has_correct_colors() {
    let theme = Theme::default();
    assert_eq!(theme.background, "#282C34");
    assert_eq!(theme.foreground, "#ABB2BF");
    assert_eq!(theme.cursor, "#528BFF");
    assert_eq!(theme.selection_bg, "#3E4451");
    assert_eq!(theme.colors[0], "#1D1F21");
    assert_eq!(theme.colors[1], "#CC6666");
    assert_eq!(theme.colors[2], "#B5BD68");
    assert_eq!(theme.colors[3], "#F0C674");
    assert_eq!(theme.colors[4], "#81A2BE");
    assert_eq!(theme.colors[5], "#B294BB");
    assert_eq!(theme.colors[6], "#8ABEB7");
    assert_eq!(theme.colors[7], "#C5C8C6");
    assert_eq!(theme.colors[8], "#666666");
    assert_eq!(theme.colors[9], "#D54E53");
    assert_eq!(theme.colors[10], "#B9CA4A");
    assert_eq!(theme.colors[11], "#E7C547");
    assert_eq!(theme.colors[12], "#7AA6DA");
    assert_eq!(theme.colors[13], "#C397D8");
    assert_eq!(theme.colors[14], "#70C0B1");
    assert_eq!(theme.colors[15], "#EAEAEA");
    assert_eq!(theme.colors.len(), 16);
}

#[test]
fn default_keybindings_match_spec() {
    let keys = KeyBindings::default();
    assert_eq!(keys.split_down, "Leader+D");
    assert_eq!(keys.split_right, "Leader+R");
    assert_eq!(keys.close_pane, "Leader+X");
    assert_eq!(keys.focus_up, "Leader+Up");
    assert_eq!(keys.focus_down, "Leader+Down");
    assert_eq!(keys.focus_left, "Leader+Left");
    assert_eq!(keys.focus_right, "Leader+Right");
    assert_eq!(keys.new_tab, "Leader+T");
    assert_eq!(keys.close_tab, "Leader+W");
    assert_eq!(keys.next_tab, "Leader+N");
    assert_eq!(keys.prev_tab, "Leader+P");
    assert_eq!(keys.detach, "Leader+Q");
    assert_eq!(keys.search, "Leader+/");
    assert_eq!(keys.toggle_fullscreen, "Leader+F");
    assert_eq!(keys.toggle_float, "Leader+G");
    assert_eq!(keys.copy_mode, "Leader+[");
}

#[test]
fn default_config_has_sane_values() {
    let cfg = Config::default();
    assert_eq!(cfg.font_size, 14.0);
    assert!(cfg.font_family.is_none());
    assert_eq!(cfg.scrollback_limit, 10_000);
    assert_eq!(cfg.tab_width, 8);
    assert_eq!(cfg.cursor_shape, "block");
    assert!(cfg.cursor_blink);
    assert!(!cfg.bold_is_bright);
}

#[test]
fn toml_parsing_works() {
    let toml_content = r##"
font_size = 18.0
cursor_shape = "underline"
cursor_blink = false
bold_is_bright = true
scrollback_limit = 50000
tab_width = 4

[theme]
background = "#000000"
foreground = "#FFFFFF"
cursor = "#FF0000"
selection_bg = "#333333"
colors = [
    "#000000", "#FF0000", "#00FF00", "#FFFF00",
    "#0000FF", "#FF00FF", "#00FFFF", "#FFFFFF",
    "#808080", "#FF8080", "#80FF80", "#FFFF80",
    "#8080FF", "#FF80FF", "#80FFFF", "#F0F0F0"
]

[keys]
split_down = "Ctrl+Shift+D"
split_right = "Ctrl+Shift+R"
close_pane = "Ctrl+Shift+X"
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

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, toml_content).unwrap();

    let cfg = load_from_path(&path).unwrap();
    assert_eq!(cfg.font_size, 18.0);
    assert_eq!(cfg.cursor_shape, "underline");
    assert!(!cfg.cursor_blink);
    assert!(cfg.bold_is_bright);
    assert_eq!(cfg.scrollback_limit, 50_000);
    assert_eq!(cfg.theme.background, "#000000");
    assert_eq!(cfg.keys.split_down, "Ctrl+Shift+D");
}

#[test]
fn partial_toml_merges_with_defaults() {
    let partial = r##"
font_size = 20.0
[theme]
background = "#111111"
"##;
    let value: toml::Value = toml::from_str(partial.trim()).unwrap();
    let cfg = merge_with_defaults(value);

    // Overridden values
    assert_eq!(cfg.font_size, 20.0);
    assert_eq!(cfg.theme.background, "#111111");

    // Default values preserved
    assert_eq!(cfg.theme.foreground, "#ABB2BF");
    assert_eq!(cfg.theme.cursor, "#528BFF");
    assert_eq!(cfg.keys.split_down, "Leader+D");
    assert_eq!(cfg.cursor_shape, "block");
    assert!(cfg.cursor_blink);
    assert_eq!(cfg.scrollback_limit, 10_000);
}

#[test]
fn missing_file_returns_error() {
    let result = load_from_path(std::path::Path::new("/tmp/nonexistent_emux_config.toml"));
    assert!(result.is_err());
}

#[test]
fn invalid_toml_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.toml");
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(b"this is not valid { toml [[[").unwrap();

    let result = load_from_path(&path);
    assert!(result.is_err());
}

// ── Theme color validation tests ────────────────────────────────────

#[test]
fn all_default_theme_colors_are_valid_hex() {
    let theme = Theme::default();
    let all_colors = [
        &theme.background,
        &theme.foreground,
        &theme.cursor,
        &theme.selection_bg,
    ];
    for color in &all_colors {
        assert!(
            color.starts_with('#') && color.len() == 7,
            "color '{}' is not valid #RRGGBB hex",
            color
        );
        assert!(
            u32::from_str_radix(&color[1..], 16).is_ok(),
            "color '{}' contains invalid hex digits",
            color
        );
    }
    for (i, color) in theme.colors.iter().enumerate() {
        assert!(
            color.starts_with('#') && color.len() == 7,
            "palette color {} ('{}') is not valid #RRGGBB hex",
            i,
            color
        );
        assert!(
            u32::from_str_radix(&color[1..], 16).is_ok(),
            "palette color {} ('{}') contains invalid hex digits",
            i,
            color
        );
    }
}

// ── Keybinding non-empty validation ─────────────────────────────────

#[test]
fn all_default_keybindings_are_non_empty() {
    let keys = KeyBindings::default();
    let bindings = [
        ("split_down", &keys.split_down),
        ("split_right", &keys.split_right),
        ("close_pane", &keys.close_pane),
        ("focus_up", &keys.focus_up),
        ("focus_down", &keys.focus_down),
        ("focus_left", &keys.focus_left),
        ("focus_right", &keys.focus_right),
        ("new_tab", &keys.new_tab),
        ("close_tab", &keys.close_tab),
        ("next_tab", &keys.next_tab),
        ("prev_tab", &keys.prev_tab),
        ("detach", &keys.detach),
        ("search", &keys.search),
        ("toggle_fullscreen", &keys.toggle_fullscreen),
        ("toggle_float", &keys.toggle_float),
    ];
    for (name, value) in &bindings {
        assert!(!value.is_empty(), "keybinding '{}' must not be empty", name);
    }
}

// ── Config serialization roundtrip ──────────────────────────────────

#[test]
fn config_serialization_roundtrip() {
    let original = Config::default();
    let serialized = toml::to_string(&original).expect("config should serialize to TOML");
    let deserialized: Config =
        toml::from_str(&serialized).expect("config should deserialize from TOML");

    assert_eq!(deserialized.font_size, original.font_size);
    assert_eq!(deserialized.cursor_shape, original.cursor_shape);
    assert_eq!(deserialized.cursor_blink, original.cursor_blink);
    assert_eq!(deserialized.bold_is_bright, original.bold_is_bright);
    assert_eq!(deserialized.scrollback_limit, original.scrollback_limit);
    assert_eq!(deserialized.tab_width, original.tab_width);
    assert_eq!(deserialized.font_family, original.font_family);
    assert_eq!(deserialized.theme.background, original.theme.background);
    assert_eq!(deserialized.theme.foreground, original.theme.foreground);
    assert_eq!(deserialized.theme.cursor, original.theme.cursor);
    assert_eq!(deserialized.theme.selection_bg, original.theme.selection_bg);
    assert_eq!(deserialized.theme.colors, original.theme.colors);
    assert_eq!(deserialized.keys.split_down, original.keys.split_down);
    assert_eq!(deserialized.keys.new_tab, original.keys.new_tab);
}

#[test]
fn config_roundtrip_with_custom_values() {
    let mut cfg = Config::default();
    cfg.font_size = 22.0;
    cfg.cursor_shape = "bar".into();
    cfg.cursor_blink = false;
    cfg.bold_is_bright = true;
    cfg.scrollback_limit = 50_000;
    cfg.font_family = Some("Fira Code".into());

    let serialized = toml::to_string(&cfg).expect("should serialize");
    let deserialized: Config = toml::from_str(&serialized).expect("should deserialize");

    assert_eq!(deserialized.font_size, 22.0);
    assert_eq!(deserialized.cursor_shape, "bar");
    assert!(!deserialized.cursor_blink);
    assert!(deserialized.bold_is_bright);
    assert_eq!(deserialized.scrollback_limit, 50_000);
    assert_eq!(deserialized.font_family, Some("Fira Code".into()));
}

// ── Deep merge edge cases ───────────────────────────────────────────

#[test]
fn merge_empty_partial_preserves_all_defaults() {
    let empty: toml::Value = ""
        .parse::<toml::Value>()
        .unwrap_or(toml::Value::Table(Default::default()));
    let cfg = merge_with_defaults(empty);
    let default_cfg = Config::default();

    assert_eq!(cfg.font_size, default_cfg.font_size);
    assert_eq!(cfg.cursor_shape, default_cfg.cursor_shape);
    assert_eq!(cfg.theme.background, default_cfg.theme.background);
    assert_eq!(cfg.keys.split_down, default_cfg.keys.split_down);
}

#[test]
fn merge_only_theme_preserves_other_sections() {
    let partial = r##"
[theme]
background = "#FF0000"
foreground = "#00FF00"
"##;
    let value: toml::Value = toml::from_str(partial.trim()).unwrap();
    let cfg = merge_with_defaults(value);

    // Theme overridden
    assert_eq!(cfg.theme.background, "#FF0000");
    assert_eq!(cfg.theme.foreground, "#00FF00");
    // Other theme fields keep defaults
    assert_eq!(cfg.theme.cursor, "#528BFF");
    assert_eq!(cfg.theme.selection_bg, "#3E4451");
    // Keys and top-level options unchanged
    assert_eq!(cfg.keys.split_down, "Leader+D");
    assert_eq!(cfg.font_size, 14.0);
    assert!(cfg.cursor_blink);
}

#[test]
fn merge_only_keys_preserves_theme_and_options() {
    let partial = r##"
[keys]
split_down = "Alt+D"
new_tab = "Alt+T"
"##;
    let value: toml::Value = toml::from_str(partial.trim()).unwrap();
    let cfg = merge_with_defaults(value);

    // Keys overridden
    assert_eq!(cfg.keys.split_down, "Alt+D");
    assert_eq!(cfg.keys.new_tab, "Alt+T");
    // Other keys keep defaults
    assert_eq!(cfg.keys.split_right, "Leader+R");
    assert_eq!(cfg.keys.close_pane, "Leader+X");
    // Theme and options unchanged
    assert_eq!(cfg.theme.background, "#282C34");
    assert_eq!(cfg.font_size, 14.0);
}

#[test]
fn merge_scalar_override_does_not_affect_tables() {
    let partial = r##"
font_size = 30.0
scrollback_limit = 100
"##;
    let value: toml::Value = toml::from_str(partial.trim()).unwrap();
    let cfg = merge_with_defaults(value);

    assert_eq!(cfg.font_size, 30.0);
    assert_eq!(cfg.scrollback_limit, 100);
    // Tables fully preserved
    assert_eq!(cfg.theme.background, "#282C34");
    assert_eq!(cfg.keys.split_down, "Leader+D");
}

#[test]
fn merge_all_sections_simultaneously() {
    let partial = r##"
font_size = 16.0
cursor_shape = "underline"

[theme]
background = "#000000"

[keys]
detach = "Ctrl+Q"
"##;
    let value: toml::Value = toml::from_str(partial.trim()).unwrap();
    let cfg = merge_with_defaults(value);

    assert_eq!(cfg.font_size, 16.0);
    assert_eq!(cfg.cursor_shape, "underline");
    assert_eq!(cfg.theme.background, "#000000");
    assert_eq!(cfg.theme.foreground, "#ABB2BF"); // default preserved
    assert_eq!(cfg.keys.detach, "Ctrl+Q");
    assert_eq!(cfg.keys.split_down, "Leader+D"); // default preserved
}

// ── Config hot-reload (ConfigWatcher) ───────────────────────────────

#[test]
fn config_watcher_detects_change() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");

    // Write initial config.
    std::fs::write(&path, "font_size = 14.0\n").unwrap();

    let mut watcher = ConfigWatcher::new(path.clone());

    // First check: no change since we just constructed the watcher.
    assert!(
        watcher.check().is_none(),
        "no change expected on first check"
    );

    // Wait a tiny bit so the mtime differs, then write new config.
    thread::sleep(Duration::from_millis(50));
    std::fs::write(&path, "font_size = 20.0\n").unwrap();

    // Second check: should detect the change.
    let new_cfg = watcher.check();
    assert!(new_cfg.is_some(), "expected config change to be detected");
    assert_eq!(new_cfg.unwrap().font_size, 20.0);

    // Third check: no change.
    assert!(watcher.check().is_none(), "no further change expected");
}

#[test]
fn config_watcher_detects_new_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");

    // File does not exist yet.
    let mut watcher = ConfigWatcher::new(path.clone());
    assert!(watcher.check().is_none());

    // Create the file.
    std::fs::write(&path, "font_size = 30.0\ncursor_blink = false\n").unwrap();

    let cfg = watcher.check();
    assert!(cfg.is_some());
    let cfg = cfg.unwrap();
    assert_eq!(cfg.font_size, 30.0);
    assert!(!cfg.cursor_blink);
}

#[test]
fn config_reload_updates_keybindings() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");

    // Write config with custom keybindings.
    let toml1 = r#"
[keys]
split_down = "Ctrl+D"
new_tab = "Ctrl+T"
"#;
    std::fs::write(&path, toml1).unwrap();

    let cfg1 = load_from_path(&path).unwrap();
    assert_eq!(cfg1.keys.split_down, "Ctrl+D");
    assert_eq!(cfg1.keys.new_tab, "Ctrl+T");

    // Update keybindings.
    thread::sleep(Duration::from_millis(50));
    let toml2 = r#"
[keys]
split_down = "Alt+D"
new_tab = "Alt+T"
"#;
    std::fs::write(&path, toml2).unwrap();

    let mut watcher = ConfigWatcher::new(path.clone());
    // Force the watcher's mtime to an old value so the next check detects change.
    // We can do this by recreating with the old content's time already captured,
    // so we need to just write again to change mtime.
    thread::sleep(Duration::from_millis(50));
    std::fs::write(&path, toml2).unwrap();

    let cfg2 = watcher.check();
    assert!(cfg2.is_some());
    let cfg2 = cfg2.unwrap();
    assert_eq!(cfg2.keys.split_down, "Alt+D");
    assert_eq!(cfg2.keys.new_tab, "Alt+T");
    // Non-overridden keys should still have defaults.
    assert_eq!(cfg2.keys.close_pane, "Leader+X");
}

#[test]
fn config_watcher_survives_invalid_toml() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");

    std::fs::write(&path, "font_size = 14.0\n").unwrap();
    let mut watcher = ConfigWatcher::new(path.clone());

    // Write invalid TOML.
    thread::sleep(Duration::from_millis(50));
    std::fs::write(&path, "this is not valid { toml [[[").unwrap();

    // check() should return None (parse failure) rather than panicking.
    assert!(watcher.check().is_none());

    // Fix the file.
    thread::sleep(Duration::from_millis(50));
    std::fs::write(&path, "font_size = 25.0\n").unwrap();

    let cfg = watcher.check();
    assert!(cfg.is_some());
    assert_eq!(cfg.unwrap().font_size, 25.0);
}

#[test]
fn config_path_returns_expected_location() {
    if let Some(path) = emux_config::config_path() {
        assert!(path.ends_with("config.toml"));
        assert!(path.to_string_lossy().contains("emux"));
    }
}
