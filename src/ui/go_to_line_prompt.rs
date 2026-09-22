use iced::widget::{
    button, column, container, mouse_area, opaque, row, space, stack, text, text_input,
};
use iced::{Element, Fill};

use crate::message::Message;
use crate::ui::{styles, utility};

pub const INPUT_ID: &str = "go-to-line-input";

pub fn view<'a>(input: &'a str, error: Option<&'a str>) -> Element<'a, Message> {
    let mut content = column![
        text("Go to line").size(18).font(utility::semibold()),
        text_input("Line number", input)
            .id(INPUT_ID)
            .on_input(Message::GoToLineChanged)
            .on_submit(Message::GoToLineSubmitted)
            .padding([8, 10])
            .size(14)
            .style(styles::input),
    ]
    .spacing(12);
    if let Some(error) = error {
        content = content.push(text(error).size(12));
    }
    content = content.push(
        row![
            space::horizontal(),
            button(text("Cancel").size(13))
                .padding([8, 14])
                .style(styles::command_button)
                .on_press(Message::GoToLineClosed),
            button(text("Go").size(13))
                .padding([8, 20])
                .style(styles::primary_command_button)
                .on_press_maybe((!input.trim().is_empty()).then_some(Message::GoToLineSubmitted)),
        ]
        .spacing(8),
    );
    stack![
        opaque(
            mouse_area(
                container(space::vertical())
                    .width(Fill)
                    .height(Fill)
                    .style(styles::modal_scrim)
            )
            .on_press(Message::GoToLineClosed)
        ),
        container(opaque(
            container(content)
                .padding(20)
                .width(360)
                .style(styles::utility_dialog)
        ))
        .padding(24)
        .center(Fill),
    ]
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::graphics::core::shell::Waker;
    use iced::advanced::renderer::{self, Headless};
    use iced::advanced::widget::{Tree, operation};
    use iced::advanced::{Layout, Shell, layout, mouse};
    use iced::{Event, Point, Rectangle, Renderer, Size, keyboard, window};

    #[test]
    fn go_to_line_input_replaces_selection_submits_and_backdrop_dismisses() {
        let renderer = futures::executor::block_on(<Renderer as Headless>::new(
            renderer::Settings::default(),
            Some("tiny-skia"),
        ))
        .unwrap();
        let mut content: Element<'_, Message> = stack![
            button(space::vertical().width(Fill).height(Fill))
                .width(Fill)
                .height(Fill)
                .on_press(Message::NewFile),
            view("120", None),
        ]
        .into();
        let viewport = Rectangle::with_size(Size::new(640.0, 364.0));
        let mut tree = Tree::empty();
        tree.diff(content.as_widget_mut());
        let node = content.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(viewport.size(), viewport.size()),
        );
        content.as_widget_mut().operate(
            &mut tree,
            Layout::new(&node),
            &renderer,
            &mut operation::focusable::focus::<()>(INPUT_ID.into()),
        );
        content.as_widget_mut().operate(
            &mut tree,
            Layout::new(&node),
            &renderer,
            &mut operation::text_input::select_all::<()>(INPUT_ID.into()),
        );
        let mut send = |event, cursor| {
            let mut messages = Vec::new();
            let mut shell = Shell::new(&window::Headless, Waker::noop(), &mut messages);
            content.as_widget_mut().update(
                &mut tree,
                &event,
                Layout::new(&node),
                cursor,
                &renderer,
                &mut shell,
                &viewport,
            );
            messages
        };
        let key_event = |key: keyboard::Key, text| {
            Event::Keyboard(keyboard::Event::KeyPressed {
                modified_key: key.clone(),
                key,
                physical_key: keyboard::key::Physical::Unidentified(
                    keyboard::key::NativeCode::Unidentified,
                ),
                location: keyboard::Location::Standard,
                modifiers: keyboard::Modifiers::empty(),
                text,
                repeat: false,
            })
        };
        let typed = send(
            key_event(keyboard::Key::Character("8".into()), Some("8".into())),
            mouse::Cursor::Unavailable,
        );
        assert!(matches!(typed.as_slice(), [Message::GoToLineChanged(value)] if value == "8"));
        let enter = send(
            key_event(keyboard::Key::Named(keyboard::key::Named::Enter), None),
            mouse::Cursor::Unavailable,
        );
        assert!(matches!(enter.as_slice(), [Message::GoToLineSubmitted]));
        let click = send(
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            mouse::Cursor::Available(Point::new(10.0, 10.0)),
        );
        assert!(matches!(click.as_slice(), [Message::GoToLineClosed]));
        let release = send(
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            mouse::Cursor::Available(Point::new(10.0, 10.0)),
        );
        assert!(
            release.is_empty(),
            "backdrop click must not activate editor controls"
        );
    }
}
