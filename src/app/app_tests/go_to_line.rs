use super::test_support::*;

#[test]
fn go_to_line_clamps_high_input_to_last_line() {
    let (mut app, _) = App::new();

    set_active_document_text(
        &mut app,
        "one\ntwo\nthree",
        EditorSelection::new(EditorPosition::new(0, 2), EditorPosition::new(0, 2)),
    );

    let _ = app.update(Message::GoToLineOpened);
    let _ = app.update(Message::GoToLineChanged(String::from("999")));
    let _ = app.update(Message::GoToLineSubmitted);

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(
        document.selection,
        EditorSelection::new(EditorPosition::new(2, 0), EditorPosition::new(2, 0))
    );
    assert!(app.go_to_line_prompt.is_none());
}

#[test]
fn go_to_line_zero_targets_first_line() {
    let (mut app, _) = App::new();

    set_active_document_text(
        &mut app,
        "one\ntwo\nthree",
        EditorSelection::new(EditorPosition::new(2, 3), EditorPosition::new(2, 3)),
    );

    let _ = app.update(Message::GoToLineOpened);
    let _ = app.update(Message::GoToLineChanged(String::from("0")));
    let _ = app.update(Message::GoToLineSubmitted);

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(
        document.selection,
        EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(0, 0))
    );
    assert!(app.go_to_line_prompt.is_none());
}

#[test]
fn go_to_line_invalid_input_preserves_selection() {
    let (mut app, _) = App::new();
    let selection = EditorSelection::new(EditorPosition::new(1, 2), EditorPosition::new(1, 2));

    set_active_document_text(&mut app, "one\ntwo\nthree", selection);

    let _ = app.update(Message::GoToLineOpened);
    let _ = app.update(Message::GoToLineChanged(String::from("two")));
    let _ = app.update(Message::GoToLineSubmitted);

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(document.selection, selection);
    assert_eq!(
        app.go_to_line_prompt.as_ref().unwrap().error.as_deref(),
        Some("Enter a valid line number.")
    );
}

fn key_event(key: iced::keyboard::Key, modifiers: iced::keyboard::Modifiers) -> iced::Event {
    iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
        modified_key: key.clone(),
        key,
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers,
        text: None,
        repeat: false,
    })
}

#[test]
fn go_to_line_shortcut_preserves_search_state_and_blocks_editor_shortcuts_until_escape() {
    use iced::keyboard::{Key, Modifiers, key::Named};
    let (mut app, _) = App::new();
    let selection = EditorSelection::new(EditorPosition::new(1, 2), EditorPosition::new(1, 2));
    set_active_document_text(&mut app, "one\ntwo\nthree", selection);
    app.search_dialog.query = "two".into();
    app.search_dialog.replacement = "second".into();
    app.search_dialog.refresh_from_workspace(&app.workspace);
    let results = app.search_dialog.results.clone();
    let status = app.search_dialog.status.clone();
    let main = app.main_window_id.unwrap();
    let primary = if cfg!(target_os = "macos") {
        Modifiers::LOGO
    } else {
        Modifiers::CTRL
    };
    let _ = app.update_runtime_event(
        key_event(Key::Character("g".into()), primary),
        iced::event::Status::Ignored,
        main,
    );
    assert_eq!(app.go_to_line_prompt.as_ref().unwrap().input, "2");
    assert!(app.advanced_search_window.is_none());
    let count = app.workspace.documents().len();
    let _ = app.update_runtime_event(
        key_event(Key::Character("n".into()), primary),
        iced::event::Status::Ignored,
        main,
    );
    let _ = app.update_runtime_event(
        key_event(Key::Named(Named::Tab), Modifiers::empty()),
        iced::event::Status::Ignored,
        main,
    );
    assert_eq!(app.workspace.documents().len(), count);
    assert_eq!(
        app.workspace.active_document().unwrap().text(),
        "one\ntwo\nthree"
    );
    let escape = super::super::shortcuts::event_to_message(
        key_event(Key::Named(Named::Escape), Modifiers::empty()),
        iced::event::Status::Captured,
        main,
    )
    .expect("Escape must reach the prompt even when the input captures it");
    let _ = app.update(escape);
    assert!(app.go_to_line_prompt.is_none());
    assert_eq!(
        app.workspace.active_document().unwrap().selection,
        selection
    );
    assert_eq!(app.search_dialog.query, "two");
    assert_eq!(app.search_dialog.replacement, "second");
    assert_eq!(app.search_dialog.results, results);
    assert_eq!(app.search_dialog.status, status);
}

#[test]
fn go_to_line_cancel_and_empty_input_do_not_move_the_caret() {
    let (mut app, _) = App::new();
    let selection = EditorSelection::new(EditorPosition::new(1, 2), EditorPosition::new(1, 2));
    set_active_document_text(&mut app, "one\ntwo\nthree", selection);
    let _ = app.update(Message::GoToLineOpened);
    let _ = app.update(Message::GoToLineChanged(" ".into()));
    let _ = app.update(Message::GoToLineSubmitted);
    assert!(app.go_to_line_prompt.as_ref().unwrap().error.is_some());
    let _ = app.update(Message::GoToLineChanged("3".into()));
    assert!(app.go_to_line_prompt.as_ref().unwrap().error.is_none());
    let _ = app.update(Message::GoToLineClosed);
    assert!(app.go_to_line_prompt.is_none());
    assert_eq!(
        app.workspace.active_document().unwrap().selection,
        selection
    );
}

#[test]
fn go_to_line_yields_to_unsaved_changes_when_the_window_closes() {
    let (mut app, _) = App::new();
    let _ = app.update(Message::GoToLineOpened);
    let document = app.workspace.active_document_mut().unwrap();
    document.is_dirty = true;
    let id = document.id;
    let _ = app.update(Message::WindowCloseRequested(app.main_window_id.unwrap()));
    assert!(app.go_to_line_prompt.is_none());
    assert_eq!(app.close_prompt.document(), Some(id));
}

#[test]
fn go_to_line_animation_keeps_the_barrier_until_dismissal_finishes() {
    use std::time::{Duration, Instant};
    for visible_for in [35, 140] {
        for submit in [false, true] {
            let (mut app, _) = App::new();
            set_active_document_text(
                &mut app,
                "one\ntwo\nthree",
                EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(0, 0)),
            );
            let _ = app.update(Message::GoToLineOpened);
            assert!(app.needs_animation_frames());
            assert_eq!(
                app.go_to_line_prompt.as_ref().unwrap().animation.progress(),
                0.0
            );
            let start = Instant::now();
            let _ = app.update(Message::ChromeAnimationFrame(start));
            let close_start = start + Duration::from_millis(visible_for);
            let _ = app.update(Message::ChromeAnimationFrame(close_start));
            let visible_progress = app.go_to_line_prompt.as_ref().unwrap().animation.progress();
            assert!(visible_progress > 0.0);
            let _ = app.update(Message::GoToLineChanged("3".into()));
            let dismissal = if submit {
                Message::GoToLineSubmitted
            } else {
                Message::GoToLineClosed
            };
            assert_eq!(
                app.update(dismissal).units(),
                0,
                "focus stays in the prompt during closing"
            );
            assert!(
                !app.go_to_line_prompt
                    .as_ref()
                    .unwrap()
                    .animation
                    .target_visible()
            );
            let _ = app.update(Message::ChromeAnimationFrame(close_start));
            let _ = app.update(Message::ChromeAnimationFrame(
                close_start + Duration::from_millis(70),
            ));
            let progress = app.go_to_line_prompt.as_ref().unwrap().animation.progress();
            assert!(progress > 0.0 && progress < visible_progress);
            let _ = app.update(Message::GoToLineChanged("2".into()));
            let _ = app.update(Message::GoToLineSubmitted);
            let _ = app.update(Message::GoToLineClosed);
            let _ = app.update(Message::GoToLineOpened);
            assert_eq!(app.go_to_line_prompt.as_ref().unwrap().input, "3");
            assert_eq!(
                app.workspace
                    .active_document()
                    .unwrap()
                    .selection
                    .cursor
                    .line,
                if submit { 2 } else { 0 }
            );
            let _ = app.update(Message::ChromeAnimationFrame(
                close_start + Duration::from_micros(139_900),
            ));
            assert!(
                app.needs_animation_frames(),
                "rounded zero must still schedule final removal"
            );
            let focus = app.update(Message::ChromeAnimationFrame(
                close_start + Duration::from_millis(140),
            ));
            assert!(focus.units() > 0, "restore editor focus after the fade");
            assert!(app.go_to_line_prompt.is_none());
            assert!(!app.needs_animation_frames());
            let _ = app.update(Message::GoToLineOpened);
            assert_eq!(
                app.go_to_line_prompt.as_ref().unwrap().animation.progress(),
                0.0
            );
        }
    }
}
