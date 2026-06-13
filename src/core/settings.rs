use iced::highlighter;

use crate::core::shortcuts::{KeyBinding, ShortcutCommand, ShortcutMap};
use crate::editor::DecorationSettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearanceMode {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareAccelerationMode {
    Off,
    Lazy,
    Diagnostic,
}

impl HardwareAccelerationMode {
    pub const ALL: &'static [Self] = &[Self::Lazy, Self::Off, Self::Diagnostic];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "Software only",
            Self::Lazy => "Hybrid rendering",
            Self::Diagnostic => "Hardware diagnostic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentationMode {
    Tabs,
    Spaces(u8),
}

impl IndentationMode {
    pub const DEFAULT_SPACE_WIDTH: u8 = 4;

    pub const fn spaces(width: u8) -> Self {
        Self::Spaces(width)
    }

    pub const fn width(self) -> u8 {
        match self {
            Self::Tabs => Self::DEFAULT_SPACE_WIDTH,
            Self::Spaces(width) => width,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditorSettings {
    pub word_wrap: bool,
    pub zoom: f32,
    pub scroll_speed: f32,
    pub indentation: IndentationMode,
    pub appearance: AppearanceMode,
    pub hardware_acceleration: HardwareAccelerationMode,
    pub syntax_theme: highlighter::Theme,
    pub decorations: DecorationSettings,
    pub shortcuts: ShortcutMap,
}

impl EditorSettings {
    pub const DEFAULT_ZOOM: f32 = 1.0;
    pub const MIN_ZOOM: f32 = 0.5;
    pub const MAX_ZOOM: f32 = 3.0;
    pub const ZOOM_STEP: f32 = 0.1;
    pub const DEFAULT_SCROLL_SPEED: f32 = 1.5;
    pub const MIN_SCROLL_SPEED: f32 = 0.25;
    pub const MAX_SCROLL_SPEED: f32 = 4.0;
    pub const SCROLL_SPEED_STEP: f32 = 0.25;

    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
    }

    pub fn zoom_in(&mut self) {
        self.set_zoom(self.zoom + Self::ZOOM_STEP);
    }

    pub fn zoom_out(&mut self) {
        self.set_zoom(self.zoom - Self::ZOOM_STEP);
    }

    pub fn reset_zoom(&mut self) {
        self.zoom = Self::DEFAULT_ZOOM;
    }

    pub fn set_scroll_speed(&mut self, scroll_speed: f32) {
        self.scroll_speed = scroll_speed.clamp(Self::MIN_SCROLL_SPEED, Self::MAX_SCROLL_SPEED);
    }

    pub fn increase_scroll_speed(&mut self) {
        self.set_scroll_speed(self.scroll_speed + Self::SCROLL_SPEED_STEP);
    }

    pub fn decrease_scroll_speed(&mut self) {
        self.set_scroll_speed(self.scroll_speed - Self::SCROLL_SPEED_STEP);
    }

    pub fn reset_scroll_speed(&mut self) {
        self.scroll_speed = Self::DEFAULT_SCROLL_SPEED;
    }

    pub fn set_word_wrap(&mut self, word_wrap: bool) {
        self.word_wrap = word_wrap;
    }

    pub fn set_indentation(&mut self, indentation: IndentationMode) {
        self.indentation = indentation;
        self.decorations.indent_width = indentation.width() as usize;
    }

    pub fn decoration_settings(&self) -> DecorationSettings {
        DecorationSettings {
            indent_width: self.indentation.width() as usize,
            ..self.decorations
        }
    }

    pub fn set_appearance(&mut self, appearance: AppearanceMode) {
        self.appearance = appearance;
    }

    pub fn set_hardware_acceleration(&mut self, hardware_acceleration: HardwareAccelerationMode) {
        self.hardware_acceleration = hardware_acceleration;
    }

    pub fn set_syntax_theme(&mut self, syntax_theme: highlighter::Theme) {
        self.syntax_theme = syntax_theme;
    }

    pub fn set_show_line_numbers(&mut self, show_line_numbers: bool) {
        self.decorations.show_line_numbers = show_line_numbers;
    }

    pub fn set_show_spaces(&mut self, show_spaces: bool) {
        self.decorations.show_spaces = show_spaces;
    }

    pub fn set_show_tabs(&mut self, show_tabs: bool) {
        self.decorations.show_tabs = show_tabs;
    }

    pub fn set_show_end_of_line_markers(&mut self, show_end_of_line_markers: bool) {
        self.decorations.show_end_of_line_markers = show_end_of_line_markers;
    }

    pub fn set_show_indentation_guides(&mut self, show_indentation_guides: bool) {
        self.decorations.show_indentation_guides = show_indentation_guides;
    }

    pub fn set_show_folding_controls(&mut self, show_folding_controls: bool) {
        self.decorations.show_folding_controls = show_folding_controls;
    }

    pub fn to_xml_string(&self) -> String {
        let mut shortcuts = XmlElement::new("shortcuts");
        for entry in self.shortcuts.entries() {
            let mut shortcut =
                XmlElement::new("shortcut").attribute("command", entry.command.key());
            if let Some(binding) = entry.binding {
                shortcut = shortcut.attribute("binding", binding.persisted());
            }
            shortcuts.push_child(shortcut);
        }

        XmlElement::new("fragile-notepad-settings")
            .attribute("version", "1")
            .child(
                XmlElement::new("general")
                    .attribute("appearance", appearance_key(self.appearance))
                    .attribute(
                        "hardware-acceleration",
                        hardware_acceleration_key(self.hardware_acceleration),
                    )
                    .attribute("syntax-theme", self.syntax_theme.to_string()),
            )
            .child(
                XmlElement::new("editor")
                    .attribute("word-wrap", self.word_wrap)
                    .attribute("indentation", indentation_key(self.indentation))
                    .attribute("scroll-speed", format!("{:.3}", self.scroll_speed)),
            )
            .child(XmlElement::new("appearance").attribute("zoom", format!("{:.3}", self.zoom)))
            .child(
                XmlElement::new("decorations")
                    .attribute("line-numbers", self.decorations.show_line_numbers)
                    .attribute("spaces", self.decorations.show_spaces)
                    .attribute("tabs", self.decorations.show_tabs)
                    .attribute("eol-markers", self.decorations.show_end_of_line_markers)
                    .attribute(
                        "indentation-guides",
                        self.decorations.show_indentation_guides,
                    )
                    .attribute("folding-controls", self.decorations.show_folding_controls),
            )
            .child(shortcuts)
            .document()
    }

    pub fn from_xml_str(input: &str) -> Self {
        let mut settings = Self::default();
        let Ok(document) = roxmltree::Document::parse(input) else {
            return settings;
        };
        let root = document.root_element();

        if root.tag_name().name() != "fragile-notepad-settings" {
            return settings;
        }

        if let Some(general) = child(root, "general") {
            if let Some(appearance) = general.attribute("appearance").and_then(parse_appearance) {
                settings.appearance = appearance;
            }
            if let Some(theme) = general
                .attribute("syntax-theme")
                .and_then(parse_syntax_theme)
            {
                settings.syntax_theme = theme;
            }
            if let Some(hardware_acceleration) = general
                .attribute("hardware-acceleration")
                .and_then(parse_hardware_acceleration)
            {
                settings.hardware_acceleration = hardware_acceleration;
            }
        }

        if let Some(editor) = child(root, "editor") {
            if let Some(word_wrap) = editor.attribute("word-wrap").and_then(parse_bool) {
                settings.word_wrap = word_wrap;
            }
            if let Some(indentation) = editor.attribute("indentation").and_then(parse_indentation) {
                settings.set_indentation(indentation);
            }
            if let Some(scroll_speed) = editor
                .attribute("scroll-speed")
                .and_then(|value| value.parse::<f32>().ok())
            {
                settings.set_scroll_speed(scroll_speed);
            }
        }

        if let Some(appearance) = child(root, "appearance")
            && let Some(zoom) = appearance
                .attribute("zoom")
                .and_then(|value| value.parse::<f32>().ok())
        {
            settings.set_zoom(zoom);
        }

        if let Some(decorations) = child(root, "decorations") {
            if let Some(show_line_numbers) =
                decorations.attribute("line-numbers").and_then(parse_bool)
            {
                settings.decorations.show_line_numbers = show_line_numbers;
            }
            if let Some(show_spaces) = decorations.attribute("spaces").and_then(parse_bool) {
                settings.decorations.show_spaces = show_spaces;
            }
            if let Some(show_tabs) = decorations.attribute("tabs").and_then(parse_bool) {
                settings.decorations.show_tabs = show_tabs;
            }
            if let Some(show_end_of_line_markers) =
                decorations.attribute("eol-markers").and_then(parse_bool)
            {
                settings.decorations.show_end_of_line_markers = show_end_of_line_markers;
            }
            if let Some(show_indentation_guides) = decorations
                .attribute("indentation-guides")
                .and_then(parse_bool)
            {
                settings.decorations.show_indentation_guides = show_indentation_guides;
            }
            if let Some(show_folding_controls) = decorations
                .attribute("folding-controls")
                .and_then(parse_bool)
            {
                settings.decorations.show_folding_controls = show_folding_controls;
            }
        }

        if let Some(shortcuts) = child(root, "shortcuts") {
            let mut shortcut_map = ShortcutMap::default();

            for command in ShortcutCommand::ALL {
                if let Some(node) = shortcuts
                    .children()
                    .filter(|node| node.has_tag_name("shortcut"))
                    .find(|node| {
                        node.attribute("command")
                            .and_then(ShortcutCommand::from_key)
                            == Some(command)
                    })
                {
                    shortcut_map.set_unchecked(
                        command,
                        node.attribute("binding").and_then(KeyBinding::parse),
                    );
                }
            }

            settings.shortcuts = shortcut_map;
        }

        settings
    }
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            word_wrap: true,
            zoom: Self::DEFAULT_ZOOM,
            scroll_speed: Self::DEFAULT_SCROLL_SPEED,
            indentation: IndentationMode::Spaces(IndentationMode::DEFAULT_SPACE_WIDTH),
            appearance: AppearanceMode::System,
            hardware_acceleration: HardwareAccelerationMode::Lazy,
            syntax_theme: highlighter::Theme::SolarizedDark,
            decorations: DecorationSettings {
                indent_width: IndentationMode::DEFAULT_SPACE_WIDTH as usize,
                ..DecorationSettings::default()
            },
            shortcuts: ShortcutMap::default(),
        }
    }
}

fn child<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    name: &str,
) -> Option<roxmltree::Node<'a, 'input>> {
    node.children().find(|child| child.has_tag_name(name))
}

fn appearance_key(appearance: AppearanceMode) -> &'static str {
    match appearance {
        AppearanceMode::System => "system",
        AppearanceMode::Light => "light",
        AppearanceMode::Dark => "dark",
    }
}

fn parse_appearance(value: &str) -> Option<AppearanceMode> {
    match value {
        "system" => Some(AppearanceMode::System),
        "light" => Some(AppearanceMode::Light),
        "dark" => Some(AppearanceMode::Dark),
        _ => None,
    }
}

fn hardware_acceleration_key(mode: HardwareAccelerationMode) -> &'static str {
    match mode {
        HardwareAccelerationMode::Off => "off",
        HardwareAccelerationMode::Lazy => "lazy",
        HardwareAccelerationMode::Diagnostic => "diagnostic",
    }
}

fn parse_hardware_acceleration(value: &str) -> Option<HardwareAccelerationMode> {
    match value {
        "off" | "software" => Some(HardwareAccelerationMode::Off),
        "lazy" | "lazy-gpu" => Some(HardwareAccelerationMode::Lazy),
        "diagnostic" | "hardware-diagnostic" => Some(HardwareAccelerationMode::Diagnostic),
        _ => None,
    }
}

fn indentation_key(indentation: IndentationMode) -> String {
    match indentation {
        IndentationMode::Tabs => String::from("tabs"),
        IndentationMode::Spaces(width) => format!("spaces:{width}"),
    }
}

fn parse_indentation(value: &str) -> Option<IndentationMode> {
    if value == "tabs" {
        return Some(IndentationMode::Tabs);
    }

    let width = value.strip_prefix("spaces:")?.parse::<u8>().ok()?;
    (width > 0).then_some(IndentationMode::Spaces(width))
}

fn parse_syntax_theme(value: &str) -> Option<highlighter::Theme> {
    highlighter::Theme::ALL
        .iter()
        .copied()
        .find(|theme| theme.to_string() == value)
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

#[derive(Debug)]
struct XmlElement {
    name: &'static str,
    attributes: Vec<XmlAttribute>,
    children: Vec<XmlElement>,
}

#[derive(Debug)]
struct XmlAttribute {
    name: &'static str,
    value: String,
}

impl XmlElement {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            attributes: Vec::new(),
            children: Vec::new(),
        }
    }

    fn attribute(mut self, name: &'static str, value: impl ToString) -> Self {
        self.attributes.push(XmlAttribute {
            name,
            value: value.to_string(),
        });
        self
    }

    fn child(mut self, child: XmlElement) -> Self {
        self.children.push(child);
        self
    }

    fn push_child(&mut self, child: XmlElement) {
        self.children.push(child);
    }

    fn document(self) -> String {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        self.write(&mut xml, 0);
        xml
    }

    fn write(&self, xml: &mut String, depth: usize) {
        let indent = "  ".repeat(depth);
        xml.push_str(&indent);
        xml.push('<');
        xml.push_str(self.name);

        for attribute in &self.attributes {
            xml.push(' ');
            xml.push_str(attribute.name);
            xml.push_str("=\"");
            xml.push_str(&escape_xml_attr(&attribute.value));
            xml.push('"');
        }

        if self.children.is_empty() {
            xml.push_str(" />\n");
            return;
        }

        xml.push_str(">\n");
        for child in &self.children {
            child.write(xml, depth + 1);
        }
        xml.push_str(&indent);
        xml.push_str("</");
        xml.push_str(self.name);
        xml.push_str(">\n");
    }
}

fn escape_xml_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
