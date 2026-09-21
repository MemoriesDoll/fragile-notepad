//! Render the real About/title-bar widgets for local artwork review.
//! Run `cargo run --example preview_branding`; outputs stay under target/.

use fragile_notepad::{
    core::AppearanceMode,
    message::{AboutTab, Message},
    ui::{about_dialog, styles, title_bar},
};
use iced::advanced::graphics::core::shell::Waker;
use iced::advanced::renderer::{self, Headless, Renderer as _};
use iced::advanced::widget::Tree;
use iced::advanced::{Layout, Shell, layout, mouse};
use iced::{Element, Event, Rectangle, Renderer, Size, window};
use std::time::{Duration, Instant};

fn main() {
    let output = std::path::Path::new("target/bunny-review");
    std::fs::create_dir_all(output).expect("create review directory");
    let mut renderer = futures::executor::block_on(<Renderer as Headless>::new(
        renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .expect("software renderer");
    let size = Size::new(900.0, 640.0);
    let viewport = Rectangle::with_size(size);

    for (style, name) in [
        (title_bar::ControlStyle::Windows, "windows"),
        (title_bar::ControlStyle::MacOS, "macos"),
    ] {
        for (appearance, theme_name) in [
            (AppearanceMode::Light, "light"),
            (AppearanceMode::Dark, "dark"),
        ] {
            let theme = styles::modern_theme(appearance).expect("explicit theme");
            let about = about_dialog::view(
                AboutTab::About,
                about_dialog::RenderingDebugInfo {
                    current_renderer: "Software".into(),
                    rendering_policy: "Software only".into(),
                    title_bar_style: style,
                },
                1.0,
                true,
            );
            let mut content: Element<'_, Message> = title_bar::frame(
                about,
                window::Id::unique(),
                "notes.md — Fragile Notepad".into(),
                style,
                true,
                false,
            );
            let mut tree = Tree::empty();
            tree.diff(content.as_widget_mut());
            let node = content.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(size, size),
            );
            let start = Instant::now();
            // Advance the widget's real animation clock, rather than imitating
            // its motion in an HTML/CSS mockup. One four-second float/blink cycle.
            let frames = if name == "windows" && theme_name == "light" {
                97
            } else {
                1
            };
            for frame in 0..frames {
                let mut messages = Vec::new();
                let mut shell = Shell::new(&window::Headless, Waker::noop(), &mut messages);
                content.as_widget_mut().update(
                    &mut tree,
                    &Event::Window(window::Event::RedrawRequested(
                        start + Duration::from_nanos(41_666_667) * frame,
                    )),
                    Layout::new(&node),
                    mouse::Cursor::Unavailable,
                    &renderer,
                    &mut shell,
                    &viewport,
                );
                renderer.reset(viewport);
                content.as_widget().draw(
                    &tree,
                    &mut renderer,
                    &theme,
                    &renderer::Style::default(),
                    Layout::new(&node),
                    mouse::Cursor::Unavailable,
                    &viewport,
                );
                let pixels = renderer.screenshot(Size::new(900, 640), 1.0, iced::Color::WHITE);
                // This screenshot has an opaque background, so its straight
                // RGBA bytes are also valid premultiplied pixels for PNG output.
                let image = tiny_skia::Pixmap::from_vec(
                    pixels,
                    tiny_skia::IntSize::from_wh(900, 640).unwrap(),
                )
                .unwrap();
                let filename = if frame == 0 {
                    format!("{name}-{theme_name}.png")
                } else {
                    format!("float-{frame:03}.png")
                };
                image.save_png(output.join(filename)).unwrap();
            }
        }
    }
    println!("Review screenshots: {}", output.display());
}
