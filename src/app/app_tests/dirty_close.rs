use super::test_support::*;
use crate::core::DocumentId;
use std::time::{Duration, Instant};

fn settle_prompt(app: &mut App) -> Instant {
    let start = Instant::now();
    let end = start + Duration::from_millis(140);
    let _ = app.update(Message::ChromeAnimationFrame(start));
    let _ = app.update(Message::ChromeAnimationFrame(end));
    assert_eq!(app.chrome_animation_info().dirty_close_progress, 1.0);
    end
}

fn finish_fade(app: &mut App, document: DocumentId, start: Instant) {
    let _ = app.update(Message::ChromeAnimationFrame(start));
    let _ = app.update(Message::ChromeAnimationFrame(
        start + Duration::from_millis(140),
    ));
    assert!(!app.needs_animation_frames());
    assert_eq!(app.chrome_animation_info().dirty_close_progress, 0.0);
    let _ = app.update(Message::DirtyCloseFadeFinished(document));
}

#[test]
fn decisions_wait_for_fade_and_ignore_repeat_clicks_and_early_completion() {
    for decision in [
        DirtyCloseDecision::Save,
        DirtyCloseDecision::Discard,
        DirtyCloseDecision::Cancel,
    ] {
        let (mut app, _) = App::new();
        let document = app.workspace.active_document_id();
        app.workspace.active_document_mut().unwrap().mark_dirty();
        let _ = app.update(Message::CloseFile);
        let start = settle_prompt(&mut app);
        let _ = app.update(Message::DirtyCloseResolved(document, decision));

        assert!(!app.chrome_animation_info().dirty_close_interactive);
        assert!(app.needs_animation_frames());
        let _ = app.update(Message::ChromeAnimationFrame(start));
        let _ = app.update(Message::ChromeAnimationFrame(
            start + Duration::from_millis(70),
        ));
        let progress = app.chrome_animation_info().dirty_close_progress;
        assert!(progress > 0.0 && progress < 1.0);
        let _ = app.update(Message::DirtyCloseResolved(
            document,
            DirtyCloseDecision::Cancel,
        ));
        let _ = app.update(Message::DirtyCloseFadeFinished(document));
        let _ = app.update(Message::WindowCloseRequested(app.main_window_id.unwrap()));
        assert_eq!(app.close_prompt.decision(), Some(decision));
        assert_eq!(app.close_prompt.document(), Some(document));
        assert!(app.workspace.document(document).is_some());
        assert!(app.files.pending_save().is_none());

        finish_fade(&mut app, document, start);
        assert_eq!(app.close_prompt.document(), None);
        assert_eq!(app.close_prompt.decision(), None);
        match decision {
            DirtyCloseDecision::Save => {
                let request = app
                    .files
                    .pending_save()
                    .cloned()
                    .expect("save starts after fading");
                assert!(app.workspace.document(document).is_some());
                let _ = app.update(Message::FileSaved(
                    request,
                    Err(crate::message::FileError::DialogClosed),
                ));
                assert!(app.workspace.document(document).is_some());
                assert_eq!(app.files.pending_close_after_save(), None);
            }
            DirtyCloseDecision::Discard => assert!(app.workspace.document(document).is_none()),
            DirtyCloseDecision::Cancel => assert!(app.workspace.document(document).is_some()),
        }
        let _ = app.update(Message::DirtyCloseFadeFinished(document));
        assert_eq!(app.close_prompt.document(), None);
    }
}

#[test]
fn queued_dirty_documents_wait_for_previous_prompt_to_fade() {
    let (mut app, _) = App::new();
    let first = app.workspace.active_document_id();
    let second = app.workspace.create_untitled();
    for id in [first, second] {
        app.workspace.document_mut(id).unwrap().mark_dirty();
    }
    let _ = app.update(Message::CloseAllFiles);
    let start = settle_prompt(&mut app);
    let _ = app.update(Message::DirtyCloseResolved(
        first,
        DirtyCloseDecision::Discard,
    ));
    assert_eq!(app.close_prompt.document(), Some(first));
    finish_fade(&mut app, first, start);
    assert!(app.workspace.document(first).is_none());
    assert_eq!(app.close_prompt.document(), Some(second));
    assert_eq!(app.chrome_animation_info().dirty_close_progress, 0.0);
    assert!(app.chrome_animation_info().dirty_close_interactive);
    assert!(app.needs_animation_frames());
}

#[test]
fn exit_and_session_changes_wait_for_dirty_prompt_fade() {
    let (mut app, _) = App::new();
    let document = app.workspace.active_document_id();
    app.workspace.active_document_mut().unwrap().mark_dirty();
    let _ = app.update(Message::WindowCloseRequested(app.main_window_id.unwrap()));
    let start = settle_prompt(&mut app);
    let task = app.update(Message::DirtyCloseResolved(
        document,
        DirtyCloseDecision::Discard,
    ));
    assert_eq!(task.units(), 0);
    assert_eq!(app.files.close_goal(), CloseGoal::ExitApp);
    assert!(app.workspace.document(document).is_some());
    finish_fade(&mut app, document, start);
    assert_eq!(app.files.close_goal(), CloseGoal::KeepOpen);
    assert!(app.workspace.document(document).is_none());

    app.session.set_enabled(true);
    let _ = app.update(Message::ChromeAnimationFrame(start));
    assert!(!app.session.is_dirty());
}

#[test]
fn rounded_fade_endpoint_does_not_leave_an_invisible_prompt_blocking_the_app() {
    let (mut app, _) = App::new();
    let document = app.workspace.active_document_id();
    app.workspace.active_document_mut().unwrap().mark_dirty();

    // Repeated open/cancel cycles with the last pre-deadline frame landing
    // inside the floating-point rounding region of the easing curve.
    for last_frame_us in [139_400, 139_500, 139_900, 139_999] {
        let _ = app.update(Message::CloseFile);
        let start = settle_prompt(&mut app);
        let _ = app.update(Message::DirtyCloseResolved(
            document,
            DirtyCloseDecision::Cancel,
        ));
        let _ = app.update(Message::ChromeAnimationFrame(start));
        let _ = app.update(Message::ChromeAnimationFrame(
            start + Duration::from_micros(last_frame_us),
        ));

        assert!(app.close_prompt.progress() <= f32::EPSILON);
        assert!(app.close_prompt.is_closing());
        assert!(
            app.needs_animation_frames(),
            "completion must remain scheduled after the fade rounds to zero at {last_frame_us} us"
        );
        let _ = app.update(Message::ChromeAnimationFrame(
            start + Duration::from_millis(150),
        ));
        let _ = app.update(Message::DirtyCloseFadeFinished(document));
        assert_eq!(
            app.close_prompt.document(),
            None,
            "remove the input barrier"
        );
        assert!(!app.needs_animation_frames());
        assert!(app.workspace.document(document).unwrap().is_dirty);
    }
}
