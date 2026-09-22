//! Render real utility windows in both themes at normal and minimum sizes.
//! `cargo run --locked --example preview_dialogs`
//! Images are written to target/dialog-review/.

use fragile_notepad::{
    core::{AppearanceMode, Document, DocumentId, Workspace},
    message::{AdvancedSearchTab, Message, WindowTarget},
    search_dialog::SearchDialogState,
    ui::{
        advanced_search_panel, styles,
        window_list_dialog::{self, WindowListEntry},
    },
};
use iced::advanced::graphics::core::shell::Waker;
use iced::advanced::renderer::{self, Headless, Renderer as _};
use iced::advanced::widget::Tree;
use iced::advanced::{Layout, Shell, layout, mouse};
use iced::{Element, Event, Rectangle, Renderer, Size, Theme, window};
use std::time::{Duration, Instant};

fn main() {
    std::fs::create_dir_all("target/dialog-review").unwrap();
    let mut renderer = futures::executor::block_on(<Renderer as Headless>::new(
        renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .expect("software renderer");
    for (appearance, name) in [
        (AppearanceMode::Light, "light"),
        (AppearanceMode::Dark, "dark"),
    ] {
        let theme = styles::modern_theme(appearance).unwrap();
        for (width, height, size_name) in [(900, 600, "normal"), (640, 364, "minimum")] {
            let entries = vec![
                WindowListEntry {
                    target: WindowTarget::Main,
                    title: "Release notes — September.md - Fragile Notepad".into(),
                    is_focused: true,
                },
                WindowListEntry {
                    target: WindowTarget::AdvancedSearch,
                    title: "Find and Replace - Fragile Notepad".into(),
                    is_focused: false,
                },
                WindowListEntry {
                    target: WindowTarget::Settings,
                    title: "Settings - Fragile Notepad".into(),
                    is_focused: false,
                },
            ];
            render(
                &mut renderer,
                window_list_dialog::view(entries),
                &theme,
                Size::new(width, height),
                &format!("windows-{name}-{size_name}"),
            );
        }
        for (width, height, size_name) in [(900, 724, "normal"), (760, 524, "minimum")] {
            for (tab, tab_name) in [
                (AdvancedSearchTab::Find, "find"),
                (AdvancedSearchTab::Replace, "replace"),
                (AdvancedSearchTab::FindInFiles, "find-open"),
                (AdvancedSearchTab::ReplaceInFiles, "replace-open"),
                (AdvancedSearchTab::GoToLine, "go-to"),
            ] {
                let mut dialog = SearchDialogState::new();
                dialog.active_tab = tab;
                if tab != AdvancedSearchTab::Find {
                    dialog.query = "release".into();
                    dialog.replacement = "launch".into();
                    dialog.go_to_line = "120".into();
                    let mut workspace = Workspace::new();
                    workspace.documents.push(Document::from_path(DocumentId::new(20), "release-notes.md", "# September release\nPrepare the release notes.\nReview the release checklist.\n"));
                    workspace.documents.push(Document::from_path(
                        DocumentId::new(21),
                        "src/main.rs",
                        "fn prepare_release() {}\n// Publish the release after review.",
                    ));
                    dialog.refresh_from_workspace(&workspace);
                }
                render(
                    &mut renderer,
                    advanced_search_panel::view(&dialog),
                    &theme,
                    Size::new(width, height),
                    &format!("search-{tab_name}-{name}-{size_name}"),
                );
            }
        }
    }
    println!("Screenshots: target/dialog-review/");
}

fn render(
    renderer: &mut Renderer,
    mut content: Element<'_, Message>,
    theme: &Theme,
    pixels: Size<u32>,
    name: &str,
) {
    let size = Size::new(pixels.width as f32, pixels.height as f32);
    let viewport = Rectangle::with_size(size);
    let mut tree = Tree::empty();
    tree.diff(content.as_widget_mut());
    let node =
        content
            .as_widget_mut()
            .layout(&mut tree, renderer, &layout::Limits::new(size, size));
    let start = Instant::now();
    for elapsed in [Duration::ZERO, Duration::from_millis(200)] {
        let mut messages = Vec::new();
        let mut shell = Shell::new(&window::Headless, Waker::noop(), &mut messages);
        content.as_widget_mut().update(
            &mut tree,
            &Event::Window(window::Event::RedrawRequested(start + elapsed)),
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            renderer,
            &mut shell,
            &viewport,
        );
    }
    renderer.reset(viewport);
    content.as_widget().draw(
        &tree,
        renderer,
        theme,
        &renderer::Style::default(),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &viewport,
    );
    let bytes = renderer.screenshot(pixels, 1.0, iced::Color::from_rgb8(110, 116, 126));
    tiny_skia::Pixmap::from_vec(
        bytes,
        tiny_skia::IntSize::from_wh(pixels.width, pixels.height).unwrap(),
    )
    .unwrap()
    .save_png(format!("target/dialog-review/{name}.png"))
    .unwrap();
}
