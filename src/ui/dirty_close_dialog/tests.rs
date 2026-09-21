use super::*;
use crate::core::DocumentId;
use iced::advanced::graphics::core::shell::Waker;
use iced::advanced::renderer::{self, Headless};
use iced::advanced::widget::{Id, Operation, Tree};
use iced::advanced::{Layout, Renderer as _, Shell, layout, mouse};
use iced::{Color, Event, Point, Rectangle, Renderer, Size, Theme, window};

const VIEWPORT: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 800.0,
    height: 600.0,
};

fn renderer() -> Renderer {
    futures::executor::block_on(<Renderer as Headless>::new(
        renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .expect("CPU headless renderer must be available")
}

fn mount(content: &mut Element<'_, Message>, renderer: &Renderer) -> (Tree, layout::Node) {
    let mut tree = Tree::empty();
    tree.diff(content.as_widget_mut());
    let node = content.as_widget_mut().layout(
        &mut tree,
        renderer,
        &layout::Limits::new(Size::ZERO, VIEWPORT.size()),
    );
    (tree, node)
}

#[test]
fn dialog_and_scrim_fade_to_the_exact_backdrop() {
    let mut renderer = renderer();
    let document = Document::untitled(DocumentId::new(7));
    let backdrop = Color::from_rgb8(23, 61, 97);
    renderer.reset(VIEWPORT);
    let background = renderer.screenshot(Size::new(800, 600), 1.0, backdrop);
    for theme in [Theme::Light, Theme::Dark] {
        let mut snapshots = Vec::new();
        for progress in [0.0, 0.5, 1.0] {
            let mut content = view(&document, progress, false);
            let (tree, node) = mount(&mut content, &renderer);
            renderer.reset(VIEWPORT);
            content.as_widget().draw(
                &tree,
                &mut renderer,
                &theme,
                &renderer::Style::default(),
                Layout::new(&node),
                mouse::Cursor::Unavailable,
                &VIEWPORT,
            );
            snapshots.push(renderer.screenshot(Size::new(800, 600), 1.0, backdrop));
        }
        assert!(
            snapshots[0] == background,
            "zero opacity must preserve the backdrop"
        );
        assert!(
            snapshots[1] != background,
            "the fading dialog must remain visible"
        );
        assert!(
            snapshots[1] != snapshots[2],
            "intermediate opacity must differ from opaque"
        );
        assert!(snapshots[2] != background);
    }
}

#[derive(Default)]
struct Buttons(Vec<Point>);

impl Operation for Buttons {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }

    fn text(&mut self, _: Option<&Id>, bounds: Rectangle, text: &str) {
        if matches!(text, "Save" | "Discard" | "Cancel") {
            self.0.push(bounds.center());
        }
    }
}

#[test]
fn closing_disables_all_decisions_and_keeps_the_modal_input_barrier() {
    let renderer = renderer();
    let document = Document::untitled(DocumentId::new(7));
    for interactive in [true, false] {
        let mut content: Element<'_, Message> = stack![
            button(space::vertical().width(Fill).height(Fill))
                .width(Fill)
                .height(Fill)
                .on_press(Message::NewFile),
            view(&document, 0.5, interactive),
        ]
        .into();
        let (mut tree, node) = mount(&mut content, &renderer);
        let mut buttons = Buttons::default();
        content
            .as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut buttons);
        assert_eq!(buttons.0.len(), 3);
        for (index, point) in buttons
            .0
            .into_iter()
            .chain([Point::new(10.0, 10.0)])
            .enumerate()
        {
            let mut messages = Vec::new();
            for event in [
                mouse::Event::ButtonPressed(mouse::Button::Left),
                mouse::Event::ButtonReleased(mouse::Button::Left),
            ] {
                let mut shell = Shell::new(&window::Headless, Waker::noop(), &mut messages);
                content.as_widget_mut().update(
                    &mut tree,
                    &Event::Mouse(event),
                    Layout::new(&node),
                    mouse::Cursor::Available(point),
                    &renderer,
                    &mut shell,
                    &VIEWPORT,
                );
            }
            if interactive && index < 3 {
                assert!(matches!(
                    messages.as_slice(),
                    [Message::DirtyCloseResolved(_, _)]
                ));
            } else {
                assert!(
                    messages.is_empty(),
                    "closing and backdrop clicks must be blocked: {messages:?}"
                );
            }
        }
    }
}
