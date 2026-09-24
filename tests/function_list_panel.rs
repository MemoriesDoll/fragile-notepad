//! Exercise the sidebar through real widget events and the software renderer.
use fragile_notepad::core::{Document, DocumentId};
use fragile_notepad::editor::{
    EditorPosition, EditorRange, FunctionEntry, FunctionKind, OutlineSnapshotMetadata, OutlineState,
};
use fragile_notepad::message::Message;
use fragile_notepad::ui::function_list_panel;
use iced::advanced::{
    Layout, Shell,
    graphics::core::shell::Waker,
    layout, mouse,
    renderer::{self, Headless},
    widget::Tree,
};
use iced::{Color, Event, Point, Rectangle, Size, Theme, keyboard};

fn header_layout(node: &layout::Node) -> Layout<'_> {
    // Panel container -> column -> header container -> header column.
    Layout::new(node).child(0).child(0).child(0)
}

fn symbol_row_center(node: &layout::Node, index: usize) -> Point {
    // Panel container -> column -> scrollable -> rows -> symbol button.
    Layout::new(node)
        .child(0)
        .child(1)
        .child(0)
        .child(index)
        .bounds()
        .center()
}

#[test]
fn filtered_sidebar_navigates_clears_closes_and_renders_a_focused_caret() {
    let document = Document::from_path(DocumentId::new(1), "example.rs", &"\n".repeat(100));
    let functions = [
        ("unrelated", 2),
        ("complete_outline_parse", 56),
        ("outline_changed", 76),
    ]
    .into_iter()
    .map(|(name, line)| FunctionEntry {
        name: name.into(),
        kind: FunctionKind::Function,
        depth: 0,
        range: EditorRange::new(
            EditorPosition::new(line, 0),
            EditorPosition::new(line + 5, 0),
        ),
        body_range: None,
    })
    .collect();
    let outline = OutlineState::ready_from_functions(
        OutlineSnapshotMetadata::from_document(&document, 0),
        functions,
    );
    let mut renderer = futures::executor::block_on(<iced::Renderer as Headless>::new(
        renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .expect("software renderer");
    let size = Size::new(280.0, 520.0);
    let viewport = Rectangle::with_size(size);

    for theme in [Theme::Light, Theme::Dark] {
        let mut element = function_list_panel::view(&document, Some(&outline), " OUTLINE ");
        let mut tree = Tree::new(element.as_widget());
        tree.diff(element.as_widget_mut());
        let node =
            element
                .as_widget_mut()
                .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
        let mut messages = Vec::new();
        let header = header_layout(&node);
        let close = header.child(0).child(3).bounds();
        let filter = header.child(2);
        // Two filtered rows, clear, close, then focus the filter before pressing Enter.
        // Use the laid-out widget bounds: system font metrics differ across runners.
        for point in [
            symbol_row_center(&node, 0),
            symbol_row_center(&node, 1),
            filter.child(1).bounds().center(),
            close.center(),
            filter.child(0).bounds().center(),
        ] {
            assert!(
                viewport.contains(point),
                "click target outside sidebar: {point:?}"
            );
            for event in [
                mouse::Event::ButtonPressed(mouse::Button::Left),
                mouse::Event::ButtonReleased(mouse::Button::Left),
            ] {
                element.as_widget_mut().update(
                    &mut tree,
                    &Event::Mouse(event),
                    Layout::new(&node),
                    mouse::Cursor::Available(point),
                    &renderer,
                    &mut Shell::new(&iced::window::Headless, Waker::noop(), &mut messages),
                    &viewport,
                );
            }
        }
        let enter = Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Enter),
            modified_key: keyboard::Key::Named(keyboard::key::Named::Enter),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Enter),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::empty(),
            text: None,
            repeat: false,
        });
        element.as_widget_mut().update(
            &mut tree,
            &enter,
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &renderer,
            &mut Shell::new(&iced::window::Headless, Waker::noop(), &mut messages),
            &viewport,
        );
        assert!(
            matches!(&messages[..], [Message::FunctionListEntrySelected(first), Message::FunctionListEntrySelected(second), Message::FunctionListQueryChanged(query), Message::ToggleFunctionList, Message::FunctionListEntrySelected(submit)] if *first == EditorPosition::new(56, 0) && *second == EditorPosition::new(76, 0) && query.is_empty() && first == submit),
            "{messages:?}"
        );

        // A one-pixel caret at a fractional glyph position previously panicked in tiny-skia.
        renderer::Renderer::reset(&mut renderer, viewport);
        element.as_widget().draw(
            &tree,
            &mut renderer,
            &theme,
            &renderer::Style::default(),
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &viewport,
        );
        let pixels = renderer.screenshot(Size::new(280, 520), 1.0, Color::TRANSPARENT);
        assert_eq!(pixels.len(), 280 * 520 * 4);
    }
}

#[test]
fn parsed_impl_parent_is_visible_and_navigable_even_when_filtering_methods() {
    use fragile_notepad::editor::{
        outline_registry_hash, outline_request_for_document, parse_outline_snapshot,
    };
    let document = Document::from_path(
        DocumentId::new(2),
        "app.rs",
        "fn before() {}\n\nimpl App {\n    fn new() {}\n    fn update() {}\n}\n",
    );
    let outline = OutlineState::ready(parse_outline_snapshot(outline_request_for_document(
        &document,
        outline_registry_hash(),
    )));
    let parent = outline
        .tree
        .roots
        .iter()
        .find(|node| node.name == "App")
        .expect("the XML parser retained App");
    assert_eq!(
        parent
            .children
            .iter()
            .map(|node| node.name.as_str())
            .collect::<Vec<_>>(),
        ["new", "update"]
    );
    let renderer = futures::executor::block_on(<iced::Renderer as Headless>::new(
        renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .unwrap();
    let size = Size::new(280.0, 520.0);
    let viewport = Rectangle::with_size(size);
    for (query, parent_index) in [("", 1), ("update", 0)] {
        let mut element = function_list_panel::view(&document, Some(&outline), query);
        let mut tree = Tree::new(element.as_widget());
        tree.diff(element.as_widget_mut());
        let node =
            element
                .as_widget_mut()
                .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
        let mut messages = Vec::new();
        for event in [
            mouse::Event::ButtonPressed(mouse::Button::Left),
            mouse::Event::ButtonReleased(mouse::Button::Left),
        ] {
            element.as_widget_mut().update(
                &mut tree,
                &Event::Mouse(event),
                Layout::new(&node),
                mouse::Cursor::Available(symbol_row_center(&node, parent_index)),
                &renderer,
                &mut Shell::new(&iced::window::Headless, Waker::noop(), &mut messages),
                &viewport,
            );
        }
        assert!(
            matches!(&messages[..], [Message::FunctionListEntrySelected(position)] if *position == parent.range.start),
            "{query:?}: {messages:?}"
        );
    }
}

#[test]
fn enum_only_document_has_a_navigable_row_when_filtered_by_its_name() {
    use fragile_notepad::editor::{
        outline_registry_hash, outline_request_for_document, parse_outline_snapshot,
    };
    let document = Document::from_path(
        DocumentId::new(3),
        "goal.rs",
        "// goal\n\nenum CloseGoal { KeepOpen, ExitApp }\n",
    );
    let outline = OutlineState::ready(parse_outline_snapshot(outline_request_for_document(
        &document,
        outline_registry_hash(),
    )));
    assert!(outline.functions.is_empty());
    let members = &outline.tree.roots[0].children;
    assert_eq!(
        members
            .iter()
            .map(|member| member.name.as_str())
            .collect::<Vec<_>>(),
        ["KeepOpen", "ExitApp"]
    );
    let renderer = futures::executor::block_on(<iced::Renderer as Headless>::new(
        renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .unwrap();
    let size = Size::new(280.0, 520.0);
    let viewport = Rectangle::with_size(size);
    for (query, row_index, expected) in [
        ("", 0, EditorPosition::new(2, 0)),
        (" CLOSEGOAL ", 0, EditorPosition::new(2, 0)),
        ("", 1, members[0].range.start),
        ("", 2, members[1].range.start),
        ("ExitApp", 1, members[1].range.start),
    ] {
        let mut element = function_list_panel::view(&document, Some(&outline), query);
        let mut tree = Tree::new(element.as_widget());
        tree.diff(element.as_widget_mut());
        let node =
            element
                .as_widget_mut()
                .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
        let mut messages = Vec::new();
        for event in [
            mouse::Event::ButtonPressed(mouse::Button::Left),
            mouse::Event::ButtonReleased(mouse::Button::Left),
        ] {
            element.as_widget_mut().update(
                &mut tree,
                &Event::Mouse(event),
                Layout::new(&node),
                mouse::Cursor::Available(symbol_row_center(&node, row_index)),
                &renderer,
                &mut Shell::new(&iced::window::Headless, Waker::noop(), &mut messages),
                &viewport,
            );
        }
        assert!(
            matches!(&messages[..], [Message::FunctionListEntrySelected(position)] if *position == expected),
            "{query:?}: {messages:?}"
        );
    }
}
