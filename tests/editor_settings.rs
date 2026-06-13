use fragile_notepad::core::{
    AppearanceMode, EditorSettings, HardwareAccelerationMode, IndentationMode, KeyBinding,
    ShortcutCommand, ShortcutKey,
};
use fragile_notepad::editor::DecorationSettings;

#[test]
fn editor_settings_defaults_enable_core_editor_decorations() {
    let settings = EditorSettings::default();

    assert!(settings.word_wrap);
    assert_eq!(settings.zoom, EditorSettings::DEFAULT_ZOOM);
    assert_eq!(settings.scroll_speed, EditorSettings::DEFAULT_SCROLL_SPEED);
    assert_eq!(
        settings.indentation,
        IndentationMode::Spaces(IndentationMode::DEFAULT_SPACE_WIDTH)
    );
    assert_eq!(settings.appearance, AppearanceMode::System);
    assert_eq!(
        settings.hardware_acceleration,
        HardwareAccelerationMode::Lazy
    );
    assert_eq!(settings.decorations, DecorationSettings::default());
    assert!(settings.decorations.show_line_numbers);
    assert!(!settings.decorations.show_spaces);
    assert!(!settings.decorations.show_tabs);
    assert!(!settings.decorations.show_end_of_line_markers);
    assert!(settings.decorations.show_indentation_guides);
    assert!(settings.decorations.show_folding_controls);
    assert!(
        settings
            .shortcuts
            .binding(ShortcutCommand::AddCaretAbove)
            .is_some()
    );
    assert!(
        settings
            .shortcuts
            .binding(ShortcutCommand::AddCaretBelow)
            .is_some()
    );
    assert!(
        settings
            .shortcuts
            .binding(ShortcutCommand::SplitSelectionIntoLines)
            .is_some()
    );
    assert!(
        settings
            .shortcuts
            .binding(ShortcutCommand::ConvertSelectionToRectangle)
            .is_some()
    );
}

#[test]
fn editor_settings_parse_xml_decoration_toggles_indentation_and_shortcuts() {
    let settings = EditorSettings::from_xml_str(
        "\
<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<fragile-notepad-settings version=\"1\">
  <general appearance=\"dark\" hardware-acceleration=\"diagnostic\" syntax-theme=\"Solarized Dark\" />
  <editor word-wrap=\"false\" indentation=\"tabs\" scroll-speed=\"2.750\" />
  <appearance zoom=\"2.250\" />
  <decorations line-numbers=\"false\" spaces=\"true\" tabs=\"true\" eol-markers=\"true\" indentation-guides=\"false\" folding-controls=\"false\" />
  <shortcuts>
    <shortcut command=\"save_file\" binding=\"primary+shift+s\" />
    <shortcut command=\"fold_all\" />
  </shortcuts>
</fragile-notepad-settings>
",
    );

    assert!(!settings.word_wrap);
    assert_eq!(settings.zoom, 2.25);
    assert_eq!(settings.scroll_speed, 2.75);
    assert_eq!(settings.indentation, IndentationMode::Tabs);
    assert_eq!(settings.decoration_settings().indent_width, 4);
    assert_eq!(settings.appearance, AppearanceMode::Dark);
    assert_eq!(
        settings.hardware_acceleration,
        HardwareAccelerationMode::Diagnostic
    );
    assert!(!settings.decorations.show_line_numbers);
    assert!(settings.decorations.show_spaces);
    assert!(settings.decorations.show_tabs);
    assert!(settings.decorations.show_end_of_line_markers);
    assert!(!settings.decorations.show_indentation_guides);
    assert!(!settings.decorations.show_folding_controls);
    assert_eq!(
        settings
            .shortcuts
            .binding_display(ShortcutCommand::SaveFile),
        format!("{}+Shift+S", platform_primary_label())
    );
    assert_eq!(
        settings.shortcuts.binding_display(ShortcutCommand::FoldAll),
        "Unassigned"
    );
}

#[test]
fn editor_settings_persist_all_decoration_keys_as_xml() {
    let mut settings = EditorSettings::default();
    settings.set_word_wrap(false);
    settings.set_zoom(1.5);
    settings.set_scroll_speed(2.25);
    settings.set_indentation(IndentationMode::spaces(2));
    settings.set_appearance(AppearanceMode::Light);
    settings.set_hardware_acceleration(HardwareAccelerationMode::Lazy);
    settings.set_show_line_numbers(false);
    settings.set_show_spaces(true);
    settings.set_show_tabs(true);
    settings.set_show_end_of_line_markers(true);
    settings.set_show_indentation_guides(false);
    settings.set_show_folding_controls(false);

    let persisted = settings.to_xml_string();

    assert!(persisted.contains("<fragile-notepad-settings version=\"1\">"));
    assert!(persisted.contains("<general appearance=\"light\""));
    assert!(persisted.contains("hardware-acceleration=\"lazy\""));
    assert!(persisted.contains("<editor word-wrap=\"false\" indentation=\"spaces:2\""));
    assert!(persisted.contains("<appearance zoom=\"1.500\""));
    assert!(persisted.contains("line-numbers=\"false\""));
    assert!(persisted.contains("spaces=\"true\""));
    assert!(persisted.contains("tabs=\"true\""));
    assert!(persisted.contains("eol-markers=\"true\""));
    assert!(persisted.contains("indentation-guides=\"false\""));
    assert!(persisted.contains("folding-controls=\"false\""));
    assert!(persisted.contains("command=\"save_file\" binding=\"primary+s\""));
    assert_eq!(settings.decoration_settings().indent_width, 2);
}

#[test]
fn editor_settings_round_trip_preserves_decoration_controls() {
    let mut settings = EditorSettings::default();
    settings.set_show_line_numbers(false);
    settings.set_show_spaces(true);
    settings.set_show_tabs(true);
    settings.set_show_end_of_line_markers(true);
    settings.set_show_indentation_guides(false);
    settings.set_show_folding_controls(false);
    settings.set_hardware_acceleration(HardwareAccelerationMode::Diagnostic);

    let parsed = EditorSettings::from_xml_str(&settings.to_xml_string());

    assert_eq!(parsed.word_wrap, settings.word_wrap);
    assert_eq!(parsed.zoom, settings.zoom);
    assert_eq!(parsed.scroll_speed, settings.scroll_speed);
    assert_eq!(parsed.indentation, settings.indentation);
    assert_eq!(parsed.appearance, settings.appearance);
    assert_eq!(parsed.hardware_acceleration, settings.hardware_acceleration);
    assert_eq!(parsed.syntax_theme, settings.syntax_theme);
    assert_eq!(parsed.decorations, settings.decorations);
}

#[test]
fn editor_settings_round_trip_preserves_xml_escaped_shortcuts() {
    let mut settings = EditorSettings::default();
    settings
        .shortcuts
        .set_binding(
            ShortcutCommand::SaveFile,
            KeyBinding::primary(ShortcutKey::character('&')),
        )
        .expect("custom shortcut should not conflict");

    let persisted = settings.to_xml_string();
    assert!(persisted.contains("binding=\"primary+&amp;\""));

    let parsed = EditorSettings::from_xml_str(&persisted);
    assert_eq!(
        parsed.shortcuts.binding_display(ShortcutCommand::SaveFile),
        format!("{}+&", platform_primary_label())
    );
}

fn platform_primary_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "Cmd"
    } else {
        "Ctrl"
    }
}

#[test]
fn editor_settings_invalid_values_keep_defaults_and_clamp_zoom() {
    let settings = EditorSettings::from_xml_str(
        "\
<fragile-notepad-settings version=\"1\">
  <general appearance=\"sepia\" />
  <editor word-wrap=\"maybe\" indentation=\"spaces:0\" scroll-speed=\"99\" />
  <appearance zoom=\"99\" />
  <decorations spaces=\"maybe\" tabs=\"true\" />
</fragile-notepad-settings>
",
    );

    assert!(settings.word_wrap);
    assert_eq!(settings.zoom, EditorSettings::MAX_ZOOM);
    assert_eq!(settings.scroll_speed, EditorSettings::MAX_SCROLL_SPEED);
    assert_eq!(
        settings.indentation,
        IndentationMode::Spaces(IndentationMode::DEFAULT_SPACE_WIDTH)
    );
    assert_eq!(settings.appearance, AppearanceMode::System);
    assert_eq!(
        settings.hardware_acceleration,
        HardwareAccelerationMode::Lazy
    );
    assert!(!settings.decorations.show_spaces);
    assert!(settings.decorations.show_tabs);
}
