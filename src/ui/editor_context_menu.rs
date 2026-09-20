use iced::advanced::Renderer as _;
use iced::advanced::text::Renderer as _;
use iced::advanced::widget::operation::Focusable;
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Layout, Shell, Widget, layout, mouse, overlay, renderer, text};
use iced::keyboard::{self, Key, key::Named};
use iced::{
    Background, Color, Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
};

use crate::core::{
    Document, DocumentId, EditorSettings, ShortcutDisplayPart, ShortcutModifierIcon,
};
use crate::editor::{AdvancedEditorState, EditorMetrics};
use crate::message::Message;
use crate::ui::menu::{self, MenuNode, MenuShortcutHint};
use crate::ui::{styles, toolbar};

const ROW_HEIGHT: f32 = 28.0;
const SEPARATOR_HEIGHT: f32 = 9.0;
const PADDING: f32 = 5.0;
const EDGE: f32 = 8.0;

pub(super) fn wrap<'a>(
    editor: Element<'a, Message>,
    document: &'a Document,
    settings: &'a EditorSettings,
    metrics: EditorMetrics,
) -> Element<'a, Message> {
    Element::new(EditorContextMenu {
        editor,
        document,
        settings,
        metrics,
    })
}

struct EditorContextMenu<'a> {
    editor: Element<'a, Message>,
    document: &'a Document,
    settings: &'a EditorSettings,
    metrics: EditorMetrics,
}

#[derive(Default, Clone, PartialEq)]
struct State {
    document: Option<DocumentId>,
    anchor: Option<Point>,
    path: Vec<usize>,
    highlighted: Vec<Option<usize>>,
    offsets: Vec<f32>,
}

impl State {
    fn open(&mut self, anchor: Point, entries: &[MenuNode], keyboard: bool) {
        self.anchor = Some(anchor);
        self.path.clear();
        self.highlighted = vec![keyboard.then(|| first_enabled(entries)).flatten()];
        self.offsets = vec![0.0];
    }

    fn close(&mut self) {
        self.anchor = None;
        self.path.clear();
        self.highlighted.clear();
        self.offsets.clear();
    }

    fn highlight(&mut self, depth: usize, index: usize) {
        self.path.truncate(depth);
        self.highlighted.truncate(depth + 1);
        self.highlighted.resize(depth + 1, None);
        self.highlighted[depth] = Some(index);
        self.offsets.truncate(depth + 1);
        self.offsets.resize(depth + 1, 0.0);
    }

    fn enter(&mut self, depth: usize, index: usize, children: &[MenuNode], keyboard: bool) {
        if self.path.get(depth) == Some(&index) {
            return;
        }
        self.highlight(depth, index);
        self.path.push(index);
        self.highlighted
            .push(keyboard.then(|| first_enabled(children)).flatten());
        self.offsets.push(0.0);
    }
}

impl Widget<Message, Theme, Renderer> for EditorContextMenu<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<State>();
        if state.document != Some(self.document.id) {
            state.close();
            state.document = Some(self.document.id);
        }
        tree.diff_children(std::slice::from_mut(&mut self.editor));
    }

    fn size(&self) -> Size<Length> {
        self.editor.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.editor
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.editor.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.traverse(&mut |operation| {
            self.editor
                .as_widget_mut()
                .operate(&mut tree.children[0], layout, renderer, operation);
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if shell.is_event_captured() {
            return;
        }
        let bounds = layout.bounds();
        let pointer_open = matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right))
        ) && cursor.position_in(bounds).is_some_and(|point| {
            point.x >= self.metrics.text_origin_x(&self.document.decorations)
                && point.x < bounds.width - 14.0
        });
        let focused = tree.children[0]
            .state
            .downcast_ref::<AdvancedEditorState<<Renderer as text::Renderer>::Paragraph>>()
            .is_focused();
        let keyboard_open = focused && is_context_menu_key(event);
        if keyboard_open {
            let editor_state = tree.children[0]
                .state
                .downcast_mut::<AdvancedEditorState<<Renderer as text::Renderer>::Paragraph>>();
            editor_state.unfocus();
            editor_state.focus();
            let editor_layout = crate::editor::EditorLayout::new(
                self.metrics,
                self.document.scroll,
                bounds.width,
                bounds.height,
            );
            let caret = self.document.main_selection().cursor;
            let caret_point = crate::editor::widget::measured_position_point(
                &self.document.buffer,
                &self.document.viewport,
                &self.document.decorations,
                editor_layout,
                caret,
                self.document.caret_visible_row(),
                renderer,
            );
            let anchor = Point::new(
                caret_point.x.clamp(
                    self.metrics
                        .text_origin_x(&self.document.decorations)
                        .min(bounds.width),
                    bounds.width,
                ),
                (caret_point.y + self.metrics.line_height).clamp(0.0, bounds.height),
            );
            tree.state.downcast_mut::<State>().open(
                anchor,
                &entries(self.document, self.settings),
                true,
            );
            shell.publish(Message::MenuClosed);
            *shell.input_method_mut() = iced::advanced::InputMethod::Disabled;
            shell.capture_event();
            shell.invalidate_widgets();
            shell.request_redraw();
            return;
        }
        self.editor.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            shell,
            viewport,
        );
        if pointer_open {
            tree.state.downcast_mut::<State>().open(
                cursor.position_in(bounds).unwrap(),
                &[],
                false,
            );
            shell.publish(Message::MenuClosed);
            shell.capture_event();
            shell.invalidate_widgets();
            shell.request_redraw();
        } else if matches!(event, Event::Window(iced::window::Event::Unfocused)) {
            tree.state.downcast_mut::<State>().close();
        }
        if tree.state.downcast_ref::<State>().anchor.is_some() {
            *shell.input_method_mut() = iced::advanced::InputMethod::Disabled;
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.editor.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        _renderer: &Renderer,
        _viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        let state = tree.state.downcast_mut::<State>();
        let anchor =
            state.anchor? + Vector::new(layout.bounds().x, layout.bounds().y) + translation;
        Some(overlay::Element::new(Box::new(ContextMenu {
            state,
            entries: entries(self.document, self.settings),
            settings: self.settings,
            anchor,
        })))
    }
}

fn is_context_menu_key(event: &Event) -> bool {
    matches!(
        event,
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: Key::Named(Named::ContextMenu),
            ..
        })
    ) || matches!(event, Event::Keyboard(keyboard::Event::KeyPressed { key: Key::Named(Named::F10), modifiers, .. })
            if modifiers.shift() && !modifiers.command() && !modifiers.alt())
}

fn entries(document: &Document, settings: &EditorSettings) -> Vec<MenuNode> {
    let mut entries = toolbar::edit_menu_entries(settings);
    entries.push(menu::separator());
    entries.push(menu::submenu(
        "context-search",
        "Search",
        vec![
            toolbar::menu_item(
                settings,
                "Find…",
                crate::core::ShortcutCommand::ToggleFind,
                Message::ToggleFind,
            ),
            toolbar::menu_item(
                settings,
                "Replace…",
                crate::core::ShortcutCommand::AdvancedReplace,
                Message::ToggleAdvancedSearch(crate::message::AdvancedSearchTab::Replace),
            ),
            menu::item("Find Next", Message::FindNext),
            menu::item("Find Previous", Message::FindPrevious),
        ],
    ));
    entries.push(menu::submenu(
        "context-navigation",
        "Navigation",
        toolbar::matching_and_function_items(settings),
    ));
    entries.push(toolbar::fold_commands_menu(settings));
    toolbar::apply_editor_availability(&mut entries, Some(document));
    text_shortcuts(&mut entries);
    entries
}

fn text_shortcuts(entries: &mut [MenuNode]) {
    for entry in entries {
        match entry {
            MenuNode::Item {
                shortcut: Some(shortcut),
                ..
            }
            | MenuNode::Disabled {
                shortcut: Some(shortcut),
                ..
            } => {
                *shortcut = MenuShortcutHint::Text(shortcut_label(shortcut));
            }
            MenuNode::Submenu { children, .. } => text_shortcuts(children),
            _ => {}
        }
    }
}

struct ContextMenu<'a> {
    state: &'a mut State,
    entries: Vec<MenuNode>,
    settings: &'a EditorSettings,
    anchor: Point,
}

impl ContextMenu<'_> {
    fn at_depth(&self, depth: usize) -> &[MenuNode] {
        entries_at_depth(&self.entries, &self.state.path, depth)
    }

    fn hit(&self, layout: Layout<'_>, point: Point) -> Option<(usize, usize)> {
        if self.state.anchor.is_none() {
            return None;
        }
        for (depth, panel) in layout
            .children()
            .take(self.state.path.len() + 1)
            .enumerate()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            let bounds = panel.bounds();
            if bounds.contains(point) {
                let y = point.y - bounds.y - PADDING + self.state.offsets[depth];
                let mut top = 0.0;
                for (index, entry) in self.at_depth(depth).iter().enumerate() {
                    let bottom = top + row_height(entry);
                    if y >= top
                        && y < bottom
                        && point.y >= bounds.y + PADDING
                        && point.y < bounds.y + bounds.height - PADDING
                    {
                        return Some((depth, index));
                    }
                    top = bottom;
                }
                return None;
            }
        }
        None
    }

    fn activate(
        &mut self,
        depth: usize,
        index: usize,
        shell: &mut Shell<'_, Message>,
        keyboard: bool,
    ) {
        match self.at_depth(depth).get(index).cloned() {
            Some(MenuNode::Item { message, .. }) => {
                shell.publish(message);
                self.state.close();
            }
            Some(MenuNode::Submenu { children, .. }) if children.iter().any(enabled) => {
                self.state.enter(depth, index, &children, keyboard);
            }
            _ => {}
        }
    }

    fn reveal_highlight(&mut self, layout: Layout<'_>) {
        let depth = self.state.path.len();
        if let Some(index) = self.state.highlighted.get(depth).copied().flatten()
            && let Some(panel) = layout.children().nth(depth)
        {
            let top = rows_height(&self.at_depth(depth)[..index]);
            let height = (panel.bounds().height - 2.0 * PADDING).max(1.0);
            let offset = &mut self.state.offsets[depth];
            if top < *offset {
                *offset = top;
            }
            if top + ROW_HEIGHT > *offset + height {
                *offset = top + ROW_HEIGHT - height;
            }
        }
    }
}

impl overlay::Overlay<Message, Theme, Renderer> for ContextMenu<'_> {
    fn layout(&mut self, _renderer: &Renderer, bounds: Size) -> layout::Node {
        if self.state.anchor.is_none() {
            return layout::Node::new(bounds);
        }
        let mut panels = Vec::new();
        let mut anchor = self.anchor;
        let mut parent: Option<Rectangle> = None;
        for depth in 0..=self.state.path.len() {
            let entries = self.at_depth(depth);
            let size = Size::new(
                menu::panel_width(entries, 264.0).min((bounds.width - EDGE * 2.0).max(1.0)),
                (rows_height(entries) + PADDING * 2.0).min((bounds.height - EDGE * 2.0).max(1.0)),
            );
            let mut rectangle = place_panel(anchor, size, bounds);
            if let Some(parent) = parent {
                let right = parent.x + parent.width - 1.0;
                rectangle.x = if right + size.width <= bounds.width - EDGE {
                    right
                } else {
                    (parent.x - size.width + 1.0).max(EDGE.min(bounds.width / 2.0))
                };
            }
            let maximum_offset = (rows_height(entries) + PADDING * 2.0 - size.height).max(0.0);
            self.state.offsets[depth] = self.state.offsets[depth].clamp(0.0, maximum_offset);
            if let Some(index) = self.state.path.get(depth) {
                anchor = Point::new(
                    rectangle.x + rectangle.width,
                    rectangle.y + rows_height(&self.at_depth(depth)[..*index])
                        - self.state.offsets[depth],
                );
            }
            parent = Some(rectangle);
            panels.push(layout::Node::new(size).move_to(rectangle.position()));
        }
        layout::Node::with_children(bounds, panels)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
    ) {
        if self.state.anchor.is_none() {
            return;
        }
        let surface = styles::menu_dropdown_band(theme);
        let normal = surface.text_color.unwrap_or(Color::BLACK);
        let muted = styles::menu_shortcut_hint_color(theme);
        for (depth, panel) in layout
            .children()
            .take(self.state.path.len() + 1)
            .enumerate()
        {
            let bounds = panel.bounds();
            renderer.with_layer(bounds.expand(24.0), |renderer| {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds,
                        border: surface.border,
                        shadow: surface.shadow,
                        ..Default::default()
                    },
                    surface
                        .background
                        .unwrap_or(Background::Color(Color::WHITE)),
                );
                let clip = Rectangle {
                    x: bounds.x + PADDING,
                    y: bounds.y + PADDING,
                    width: (bounds.width - PADDING * 2.0).max(0.0),
                    height: (bounds.height - PADDING * 2.0).max(0.0),
                };
                renderer.with_layer(clip, |renderer| {
                    let mut y = bounds.y + PADDING - self.state.offsets[depth];
                    for (index, entry) in self.at_depth(depth).iter().enumerate() {
                        let height = row_height(entry);
                        let row = Rectangle {
                            x: clip.x,
                            y,
                            width: clip.width,
                            height,
                        };
                        y += height;
                        if !row.intersects(&clip) {
                            continue;
                        }
                        if matches!(entry, MenuNode::Separator) {
                            renderer.fill_quad(
                                renderer::Quad {
                                    bounds: Rectangle {
                                        x: row.x + 7.0,
                                        y: row.center_y(),
                                        width: (row.width - 14.0).max(0.0),
                                        height: 1.0,
                                    },
                                    ..Default::default()
                                },
                                styles::separator(theme)
                                    .background
                                    .unwrap_or(Background::Color(muted)),
                            );
                            continue;
                        }
                        let available = enabled(entry);
                        if available && self.state.highlighted.get(depth) == Some(&Some(index)) {
                            let highlight = styles::menu_submenu_item(true)(theme);
                            renderer.fill_quad(
                                renderer::Quad {
                                    bounds: row,
                                    border: highlight.border,
                                    ..Default::default()
                                },
                                highlight
                                    .background
                                    .unwrap_or(Background::Color(Color::TRANSPARENT)),
                            );
                        }
                        let (label, shortcut) = match entry {
                            MenuNode::Item {
                                label, shortcut, ..
                            } => (
                                label,
                                shortcut.as_ref().map(shortcut_label).unwrap_or_default(),
                            ),
                            MenuNode::Disabled { label, shortcut } => (
                                label,
                                shortcut.as_ref().map(shortcut_label).unwrap_or_default(),
                            ),
                            MenuNode::Submenu { label, .. } => (label, String::from("›")),
                            MenuNode::Separator => unreachable!(),
                        };
                        draw_text(
                            renderer,
                            label,
                            Point::new(row.x + 10.0, row.center_y()),
                            if available { normal } else { muted },
                            13.0,
                            text::Alignment::Left,
                            clip,
                        );
                        draw_text(
                            renderer,
                            &shortcut,
                            Point::new(row.x + row.width - 10.0, row.center_y()),
                            muted,
                            12.0,
                            text::Alignment::Right,
                            clip,
                        );
                    }
                });
                let total = rows_height(self.at_depth(depth));
                if self.state.offsets[depth] > 0.0 {
                    draw_text(
                        renderer,
                        "▴",
                        Point::new(bounds.center_x(), bounds.y + 3.0),
                        muted,
                        9.0,
                        text::Alignment::Center,
                        bounds,
                    );
                }
                if self.state.offsets[depth] + clip.height + 0.5 < total {
                    draw_text(
                        renderer,
                        "▾",
                        Point::new(bounds.center_x(), bounds.y + bounds.height - 3.0),
                        muted,
                        9.0,
                        text::Alignment::Center,
                        bounds,
                    );
                }
            });
        }
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        if self.state.anchor.is_none() {
            return;
        }
        let before = self.state.clone();
        match event {
            Event::Window(iced::window::Event::Unfocused | iced::window::Event::Resized(_)) => {
                self.state.close()
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if let Some((depth, index)) = self.hit(layout, *position) {
                    let entry = self.at_depth(depth)[index].clone();
                    match entry {
                        MenuNode::Submenu { children, .. } if children.iter().any(enabled) => {
                            self.state.enter(depth, index, &children, false)
                        }
                        _ => self.state.highlight(depth, index),
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(button)) => {
                if let Some((depth, index)) =
                    cursor.position().and_then(|point| self.hit(layout, point))
                {
                    if *button == mouse::Button::Left {
                        self.activate(depth, index, shell, false);
                    }
                } else if !layout
                    .children()
                    .take(self.state.path.len() + 1)
                    .any(|panel| cursor.is_over(panel.bounds()))
                {
                    self.state.close();
                }
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonReleased(_)) => {
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if let Some(depth) = layout
                    .children()
                    .take(self.state.path.len() + 1)
                    .enumerate()
                    .filter(|(_, panel)| cursor.is_over(panel.bounds()))
                    .map(|(depth, _)| depth)
                    .last()
                {
                    let delta = match delta {
                        mouse::ScrollDelta::Lines { y, .. } => y * ROW_HEIGHT * 3.0,
                        mouse::ScrollDelta::Pixels { y, .. } => *y,
                    };
                    self.state.path.truncate(depth);
                    self.state.offsets[depth] = (self.state.offsets[depth] - delta).max(0.0);
                }
                shell.capture_event();
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modified_key,
                modifiers,
                ..
            }) => {
                let depth = self.state.path.len();
                let current = self.state.highlighted.get(depth).copied().flatten();
                let modified_shortcut = (!modifiers.is_empty()
                    && !(key == &Key::Named(Named::Tab)
                        && *modifiers == keyboard::Modifiers::SHIFT))
                    .then(|| {
                        self.settings
                            .shortcuts
                            .resolve(key, modified_key, *modifiers)
                    })
                    .flatten();
                if let Some(command) = modified_shortcut {
                    shell.publish(Message::Shortcut(command));
                    self.state.close();
                } else {
                    match key.as_ref() {
                        Key::Named(
                            Named::Escape | Named::Tab | Named::ContextMenu | Named::F10,
                        ) => self.state.close(),
                        Key::Named(Named::ArrowLeft) if depth > 0 => {
                            self.state.path.pop();
                            self.state.highlighted.truncate(depth);
                            self.state.offsets.truncate(depth);
                        }
                        Key::Named(Named::ArrowRight) => {
                            if let Some(index) = current
                                && matches!(
                                    self.at_depth(depth).get(index),
                                    Some(MenuNode::Submenu { .. })
                                )
                            {
                                self.activate(depth, index, shell, true);
                            }
                        }
                        Key::Named(Named::Enter | Named::Space) => {
                            if let Some(index) = current {
                                self.activate(depth, index, shell, true);
                            }
                        }
                        Key::Named(
                            Named::ArrowDown | Named::ArrowUp | Named::Home | Named::End,
                        ) => {
                            let entries = self.at_depth(depth);
                            let next = match key.as_ref() {
                                Key::Named(Named::Home) => first_enabled(entries),
                                Key::Named(Named::End) => entries.iter().rposition(enabled),
                                Key::Named(Named::ArrowUp) => next_enabled(entries, current, false),
                                _ => next_enabled(entries, current, true),
                            };
                            if let Some(index) = next {
                                self.state.highlight(depth, index);
                            }
                        }
                        _ => {
                            if let Some(command) =
                                self.settings
                                    .shortcuts
                                    .resolve(key, modified_key, *modifiers)
                            {
                                shell.publish(Message::Shortcut(command));
                                self.state.close();
                            } else if let Key::Character(character) = key.as_ref() {
                                let entries = self.at_depth(depth);
                                let prefix = character.to_lowercase();
                                let start = current.map_or(0, |index| index + 1);
                                let next = (0..entries.len())
                                    .map(|step| (start + step) % entries.len())
                                    .find(|index| {
                                        enabled(&entries[*index])
                                            && node_label(&entries[*index])
                                                .to_lowercase()
                                                .starts_with(&prefix)
                                    });
                                if let Some(index) = next {
                                    self.state.highlight(depth, index);
                                }
                            }
                        }
                    }
                }
                self.reveal_highlight(layout);
                shell.capture_event();
            }
            Event::Keyboard(_) | Event::InputMethod(_) => {
                shell.capture_event();
            }
            _ => return,
        }
        if *self.state == before {
            return;
        }
        if self.state.anchor.is_none() {
            shell.invalidate_widgets();
        } else if self.state.path != before.path || self.state.offsets != before.offsets {
            shell.invalidate_layout();
        }
        shell.request_redraw();
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor
            .position()
            .and_then(|point| self.hit(layout, point))
            .is_some_and(|(depth, index)| enabled(&self.at_depth(depth)[index]))
        {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::Idle
        }
    }
}

fn entries_at_depth<'a>(entries: &'a [MenuNode], path: &[usize], depth: usize) -> &'a [MenuNode] {
    let mut entries = entries;
    for index in path.iter().take(depth) {
        match entries.get(*index) {
            Some(MenuNode::Submenu { children, .. }) => entries = children,
            _ => return &[],
        }
    }
    entries
}

fn row_height(entry: &MenuNode) -> f32 {
    if matches!(entry, MenuNode::Separator) {
        SEPARATOR_HEIGHT
    } else {
        ROW_HEIGHT
    }
}

fn rows_height(entries: &[MenuNode]) -> f32 {
    entries.iter().map(row_height).sum()
}

fn enabled(entry: &MenuNode) -> bool {
    match entry {
        MenuNode::Item { .. } => true,
        MenuNode::Submenu { children, .. } => children.iter().any(enabled),
        _ => false,
    }
}

fn first_enabled(entries: &[MenuNode]) -> Option<usize> {
    entries.iter().position(enabled)
}

fn next_enabled(entries: &[MenuNode], current: Option<usize>, forward: bool) -> Option<usize> {
    let count = entries.len();
    (1..=count)
        .map(|step| {
            if forward {
                (current.unwrap_or(count - 1) + step) % count
            } else {
                (current.unwrap_or(0) + count - step) % count
            }
        })
        .find(|index| enabled(&entries[*index]))
}

fn node_label(entry: &MenuNode) -> &str {
    match entry {
        MenuNode::Item { label, .. }
        | MenuNode::Disabled { label, .. }
        | MenuNode::Submenu { label, .. } => label,
        MenuNode::Separator => "",
    }
}

fn place_panel(anchor: Point, size: Size, viewport: Size) -> Rectangle {
    let inset_x = EDGE.min((viewport.width - size.width).max(0.0) / 2.0);
    let inset_y = EDGE.min((viewport.height - size.height).max(0.0) / 2.0);
    Rectangle {
        x: anchor.x.clamp(
            inset_x,
            (viewport.width - size.width - inset_x).max(inset_x),
        ),
        y: anchor.y.clamp(
            inset_y,
            (viewport.height - size.height - inset_y).max(inset_y),
        ),
        width: size.width,
        height: size.height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{EditorPosition, EditorSelection};
    use iced::advanced::graphics::core::shell::Waker;
    use iced::advanced::overlay::Overlay;
    use iced::advanced::renderer::Headless;

    fn renderer() -> Renderer {
        futures::executor::block_on(<Renderer as Headless>::new(
            renderer::Settings::default(),
            Some("tiny-skia"),
        ))
        .expect("CPU renderer")
    }

    fn key(named: Named) -> Event {
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: Key::Named(named),
            modified_key: Key::Named(named),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Enter),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::empty(),
            text: None,
            repeat: false,
        })
    }

    fn dispatch(
        menu: &mut ContextMenu<'_>,
        renderer: &Renderer,
        viewport: Size,
        event: Event,
        cursor: mouse::Cursor,
    ) -> Vec<Message> {
        let node = menu.layout(renderer, viewport);
        let mut messages = Vec::new();
        let mut shell = Shell::new(&iced::window::Headless, Waker::noop(), &mut messages);
        menu.update(&event, Layout::new(&node), cursor, renderer, &mut shell);
        assert!(shell.is_event_captured());
        menu.layout(renderer, viewport);
        messages
    }

    #[test]
    fn keyboard_navigation_skips_unavailable_commands_and_dispatches_submenu_command() {
        let renderer = renderer();
        let settings = EditorSettings::default();
        let entries = vec![
            menu::disabled("Undo"),
            menu::separator(),
            menu::item("Copy", Message::Copy),
            menu::submenu(
                "case",
                "Convert",
                vec![
                    menu::disabled("Unavailable"),
                    menu::item("Uppercase", Message::Uppercase),
                ],
            ),
        ];
        let mut state = State::default();
        state.open(Point::ORIGIN, &entries, true);
        let mut menu = ContextMenu {
            state: &mut state,
            entries,
            settings: &settings,
            anchor: Point::new(50.0, 50.0),
        };
        let viewport = Size::new(800.0, 600.0);
        assert_eq!(menu.state.highlighted[0], Some(2));
        for named in [Named::ArrowDown, Named::ArrowRight] {
            assert!(
                dispatch(
                    &mut menu,
                    &renderer,
                    viewport,
                    key(named),
                    mouse::Cursor::Unavailable
                )
                .is_empty()
            );
        }
        assert_eq!(menu.state.path, vec![3]);
        assert_eq!(menu.state.highlighted[1], Some(1));
        let messages = dispatch(
            &mut menu,
            &renderer,
            viewport,
            key(Named::Enter),
            mouse::Cursor::Unavailable,
        );
        assert!(matches!(messages.as_slice(), [Message::Uppercase]));
        assert!(menu.state.anchor.is_none());
    }

    #[test]
    fn modified_navigation_shortcuts_dispatch_without_changing_plain_menu_controls() {
        let renderer = renderer();
        let mut settings = EditorSettings::default();
        settings
            .shortcuts
            .set_binding(
                crate::core::ShortcutCommand::NewFile,
                crate::core::KeyBinding::from_event(
                    &Key::Named(Named::Home),
                    keyboard::Modifiers::CTRL,
                )
                .unwrap(),
            )
            .unwrap();
        let entries = vec![
            menu::item("Copy", Message::Copy),
            menu::item("Paste", Message::Paste),
        ];
        let mut state = State::default();
        let viewport = Size::new(800.0, 600.0);
        let mut menu = ContextMenu {
            state: &mut state,
            entries,
            settings: &settings,
            anchor: Point::new(50.0, 50.0),
        };
        for (named, modifiers, command) in [
            (
                Named::ArrowDown,
                keyboard::Modifiers::COMMAND | keyboard::Modifiers::ALT,
                crate::core::ShortcutCommand::AddCaretBelow,
            ),
            (
                Named::ArrowUp,
                keyboard::Modifiers::COMMAND | keyboard::Modifiers::ALT,
                crate::core::ShortcutCommand::AddCaretAbove,
            ),
            (
                Named::Home,
                keyboard::Modifiers::CTRL,
                crate::core::ShortcutCommand::NewFile,
            ),
        ] {
            menu.state.open(Point::ORIGIN, &menu.entries, true);
            let mut event = key(named);
            if let Event::Keyboard(keyboard::Event::KeyPressed {
                modifiers: actual, ..
            }) = &mut event
            {
                *actual = modifiers;
            }
            let messages = dispatch(
                &mut menu,
                &renderer,
                viewport,
                event,
                mouse::Cursor::Unavailable,
            );
            assert!(
                matches!(messages.as_slice(), [Message::Shortcut(actual)] if *actual == command)
            );
            assert!(menu.state.anchor.is_none());
        }

        menu.state.open(Point::ORIGIN, &menu.entries, true);
        assert!(
            dispatch(
                &mut menu,
                &renderer,
                viewport,
                key(Named::ArrowDown),
                mouse::Cursor::Unavailable,
            )
            .is_empty()
        );
        assert_eq!(menu.state.highlighted[0], Some(1));
        let mut event = key(Named::Tab);
        if let Event::Keyboard(keyboard::Event::KeyPressed { modifiers, .. }) = &mut event {
            *modifiers = keyboard::Modifiers::SHIFT;
        }
        assert!(
            dispatch(
                &mut menu,
                &renderer,
                viewport,
                event,
                mouse::Cursor::Unavailable,
            )
            .is_empty()
        );
        assert!(menu.state.anchor.is_none());
    }

    #[test]
    fn menus_flip_and_fit_near_window_edges_and_keyboard_scroll_reveals_items() {
        let renderer = renderer();
        let settings = EditorSettings::default();
        let entries = vec![menu::submenu(
            "lines",
            "Lines",
            (0..30)
                .map(|n| menu::item(format!("Line {n}"), Message::Copy))
                .collect(),
        )];
        let mut state = State::default();
        state.open(Point::ORIGIN, &entries, true);
        let viewport = Size::new(800.0, 240.0);
        let mut menu = ContextMenu {
            state: &mut state,
            entries,
            settings: &settings,
            anchor: Point::new(790.0, 230.0),
        };
        dispatch(
            &mut menu,
            &renderer,
            viewport,
            key(Named::ArrowRight),
            mouse::Cursor::Unavailable,
        );
        dispatch(
            &mut menu,
            &renderer,
            viewport,
            key(Named::End),
            mouse::Cursor::Unavailable,
        );
        let node = menu.layout(&renderer, viewport);
        let root = node.children()[0].bounds();
        let child = node.children()[1].bounds();
        assert!(child.x < root.x);
        for panel in node.children() {
            assert!(panel.bounds().x >= 0.0 && panel.bounds().y >= 0.0);
            assert!(panel.bounds().x + panel.bounds().width <= viewport.width);
            assert!(panel.bounds().y + panel.bounds().height <= viewport.height);
        }
        assert_eq!(menu.state.highlighted[1], Some(29));
        assert!(menu.state.offsets[1] > 0.0);
        let point = Point::new(child.x + 20.0, child.y + child.height - PADDING - 2.0);
        assert_eq!(menu.hit(Layout::new(&node), point), Some((1, 29)));
    }

    #[test]
    fn disabled_click_and_dismissal_never_dispatch_an_editor_command() {
        let renderer = renderer();
        let settings = EditorSettings::default();
        let entries = vec![menu::disabled("Undo"), menu::item("Copy", Message::Copy)];
        let mut state = State::default();
        state.open(Point::ORIGIN, &entries, false);
        let viewport = Size::new(800.0, 600.0);
        let mut menu = ContextMenu {
            state: &mut state,
            entries,
            settings: &settings,
            anchor: Point::new(50.0, 50.0),
        };
        let click = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        assert!(
            dispatch(
                &mut menu,
                &renderer,
                viewport,
                click.clone(),
                mouse::Cursor::Available(Point::new(80.0, 70.0))
            )
            .is_empty()
        );
        assert!(menu.state.anchor.is_some());
        assert!(
            dispatch(
                &mut menu,
                &renderer,
                viewport,
                click,
                mouse::Cursor::Available(Point::new(700.0, 500.0))
            )
            .is_empty()
        );
        assert!(menu.state.anchor.is_none());
    }

    #[test]
    fn right_click_preserves_selection_and_escape_returns_keyboard_to_editor() {
        let renderer = renderer();
        let settings = EditorSettings::default();
        let mut document = Document::from_path(
            DocumentId::new(101),
            "context.txt",
            "alpha beta\nsecond line",
        );
        document.set_main_selection(EditorSelection::new(
            EditorPosition::new(0, 0),
            EditorPosition::new(0, 5),
        ));
        let mut editor = crate::ui::editor::view(&document, &settings);
        let mut tree = Tree::empty();
        tree.diff(editor.as_widget_mut());
        let viewport = Rectangle::with_size(Size::new(800.0, 600.0));
        let node = editor.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(viewport.size(), viewport.size()),
        );
        let mut messages = Vec::new();
        let mut shell = Shell::new(&iced::window::Headless, Waker::noop(), &mut messages);
        editor.as_widget_mut().update(
            &mut tree,
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)),
            Layout::new(&node),
            mouse::Cursor::Available(Point::new(100.0, 14.0)),
            &renderer,
            &mut shell,
            &viewport,
        );
        assert!(!messages.iter().any(|message| matches!(
            message,
            Message::EditorAction(_, crate::editor::EditorAction::PlaceCaret(_))
        )));
        let mut frame_messages = Vec::new();
        let mut shell = Shell::new(&iced::window::Headless, Waker::noop(), &mut frame_messages);
        editor.as_widget_mut().update(
            &mut tree,
            &Event::Window(iced::window::Event::RedrawRequested(
                iced::time::Instant::now(),
            )),
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &renderer,
            &mut shell,
            &viewport,
        );
        assert!(matches!(
            shell.input_method(),
            iced::advanced::InputMethod::Disabled
        ));
        {
            let mut overlay = editor
                .as_widget_mut()
                .overlay(
                    &mut tree,
                    Layout::new(&node),
                    &renderer,
                    &viewport,
                    Vector::ZERO,
                )
                .expect("context menu");
            let menu_node = overlay.as_overlay_mut().layout(&renderer, viewport.size());
            let mut shell = Shell::new(&iced::window::Headless, Waker::noop(), &mut messages);
            overlay.as_overlay_mut().update(
                &key(Named::Escape),
                Layout::new(&menu_node),
                mouse::Cursor::Unavailable,
                &renderer,
                &mut shell,
            );
        }
        assert!(tree.state.downcast_ref::<State>().anchor.is_none());
        assert!(
            tree.children[0]
                .state
                .downcast_ref::<AdvancedEditorState<<Renderer as text::Renderer>::Paragraph>>()
                .is_focused()
        );
        let mut shell = Shell::new(&iced::window::Headless, Waker::noop(), &mut frame_messages);
        editor.as_widget_mut().update(
            &mut tree,
            &Event::Window(iced::window::Event::RedrawRequested(
                iced::time::Instant::now(),
            )),
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &renderer,
            &mut shell,
            &viewport,
        );
        assert!(matches!(
            shell.input_method(),
            iced::advanced::InputMethod::Enabled { .. }
        ));
    }

    #[test]
    fn command_availability_and_custom_shortcut_survive_menu_construction() {
        let settings = EditorSettings::default();
        let document = Document::untitled(DocumentId::new(102));
        let items = entries(&document, &settings);
        assert!(
            matches!(&items[0], MenuNode::Disabled { label, shortcut: Some(_) } if label == "Undo")
        );
        assert!(matches!(&items[1], MenuNode::Disabled { label, .. } if label == "Redo"));
        assert!(items.iter().any(|entry| matches!(
            entry,
            MenuNode::Item {
                message: Message::Paste,
                ..
            }
        )));
        assert!(!items.iter().any(|entry| matches!(
            entry,
            MenuNode::Item {
                message: Message::Copy,
                ..
            }
        )));
    }

    #[test]
    fn dismissing_or_switching_submenus_tolerates_cursor_queries_with_previous_layout() {
        let mut renderer = renderer();
        let settings = EditorSettings::default();
        let items = vec![
            menu::submenu("lines", "Lines", vec![menu::item("Copy", Message::Copy)]),
            menu::item("Paste", Message::Paste),
        ];
        let mut state = State::default();
        state.open(Point::ORIGIN, &items, true);
        let viewport = Size::new(800.0, 600.0);
        let mut menu = ContextMenu {
            state: &mut state,
            entries: items,
            settings: &settings,
            anchor: Point::new(100.0, 100.0),
        };
        dispatch(
            &mut menu,
            &renderer,
            viewport,
            key(Named::ArrowRight),
            mouse::Cursor::Unavailable,
        );
        let old_node = menu.layout(&renderer, viewport);
        let cursor = mouse::Cursor::Available(old_node.children()[1].bounds().center());
        menu.state.highlight(0, 1);
        menu.mouse_interaction(Layout::new(&old_node), cursor, &renderer);
        let mut messages = Vec::new();
        let mut shell = Shell::new(&iced::window::Headless, Waker::noop(), &mut messages);
        menu.update(
            &Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
            }),
            Layout::new(&old_node),
            cursor,
            &renderer,
            &mut shell,
        );
        menu.state.close();
        assert_eq!(
            menu.mouse_interaction(Layout::new(&old_node), cursor, &renderer),
            mouse::Interaction::Idle
        );
        menu.draw(
            &mut renderer,
            &Theme::Light,
            &renderer::Style::default(),
            Layout::new(&old_node),
            cursor,
        );
        assert_eq!(
            menu.hit(
                Layout::new(&old_node),
                old_node.children()[1].bounds().center()
            ),
            None
        );
    }
}

fn shortcut_label(shortcut: &MenuShortcutHint) -> String {
    match shortcut {
        MenuShortcutHint::Text(label) => label.clone(),
        MenuShortcutHint::Binding(display) => display
            .modifiers
            .iter()
            .map(|part| match part {
                ShortcutDisplayPart::Text(text) => *text,
                ShortcutDisplayPart::Icon(ShortcutModifierIcon::Shift) => "Shift",
                ShortcutDisplayPart::Icon(ShortcutModifierIcon::Command) => "⌘",
                ShortcutDisplayPart::Icon(ShortcutModifierIcon::Option) => "⌥",
                ShortcutDisplayPart::Icon(ShortcutModifierIcon::Windows) => "Win",
            })
            .chain(std::iter::once(display.key.as_str()))
            .collect::<Vec<_>>()
            .join("+"),
    }
}

fn draw_text(
    renderer: &mut Renderer,
    content: &str,
    position: Point,
    color: Color,
    size: f32,
    align_x: text::Alignment,
    clip: Rectangle,
) {
    renderer.fill_text(
        text::Text {
            content: content.to_owned(),
            bounds: Size::new(clip.width, ROW_HEIGHT),
            size: iced::Pixels(size),
            line_height: text::LineHeight::Relative(1.0),
            font: iced::Font::DEFAULT,
            align_x,
            align_y: iced::alignment::Vertical::Center,
            shaping: text::Shaping::Advanced,
            wrapping: text::Wrapping::None,
            ellipsis: text::Ellipsis::None,
            hint_factor: renderer.scale_factor(),
        },
        position,
        color,
        clip,
    );
}
