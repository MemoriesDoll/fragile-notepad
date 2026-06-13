use super::test_support::*;

#[test]
fn advanced_search_result_selection_scrolls_target_line_into_view() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;
    let contents = (0..120)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n");

    {
        let document = app
            .workspace
            .active_document_mut()
            .expect("active document");
        document.buffer = crate::editor::EditorBuffer::from_text(contents);
        document.refresh_after_text_change();
        document.mark_clean();
    }

    let selection = EditorSelection::new(EditorPosition::new(90, 5), EditorPosition::new(90, 7));
    let _ = app.update(Message::AdvancedSearchResultSelected(
        document_id,
        selection,
    ));

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(document.selection, selection);
    assert!(
        document.scroll.first_visible_row <= 90,
        "scroll should move before or to the target row"
    );
    assert!(
        document.scroll.first_visible_row >= 80,
        "scroll should move near the target row, got {}",
        document.scroll.first_visible_row
    );
}

#[test]
fn advanced_find_next_respects_disabled_wrap_around() {
    let (mut app, _) = App::new();

    {
        let document = app
            .workspace
            .active_document_mut()
            .expect("active document");
        document.buffer = EditorBuffer::from_text("alpha beta");
        document.selection =
            EditorSelection::new(EditorPosition::new(0, 10), EditorPosition::new(0, 10));
        document.refresh_after_text_change();
        document.mark_clean();
    }

    app.search_dialog.set_query("alpha");
    let _ = app.update(Message::AdvancedSearchWrapAroundToggled(false));
    let _ = app.update(Message::AdvancedFindNextRun);

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(
        document.selection,
        EditorSelection::new(EditorPosition::new(0, 10), EditorPosition::new(0, 10))
    );
}

#[test]
fn select_and_find_next_sets_query_from_selection_and_selects_next_match() {
    let (mut app, _) = App::new();

    set_active_document_text(
        &mut app,
        "alpha beta alpha",
        EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(0, 5)),
    );

    let _ = app.update(Message::SelectAndFindNext);

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(app.find.query, "alpha");
    assert_eq!(
        document.selection,
        EditorSelection::new(EditorPosition::new(0, 11), EditorPosition::new(0, 16))
    );
}

#[test]
fn volatile_find_next_uses_selection_without_replacing_existing_query() {
    let (mut app, _) = App::new();
    app.find.set_query("beta");

    set_active_document_text(
        &mut app,
        "alpha beta alpha",
        EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(0, 5)),
    );

    let _ = app.update(Message::VolatileFindNext);

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(app.find.query, "beta");
    assert_eq!(
        document.selection,
        EditorSelection::new(EditorPosition::new(0, 11), EditorPosition::new(0, 16))
    );
}

#[test]
fn advanced_find_next_wraps_when_enabled() {
    let (mut app, _) = App::new();

    {
        let document = app
            .workspace
            .active_document_mut()
            .expect("active document");
        document.buffer = EditorBuffer::from_text("alpha beta");
        document.selection =
            EditorSelection::new(EditorPosition::new(0, 10), EditorPosition::new(0, 10));
        document.refresh_after_text_change();
        document.mark_clean();
    }

    app.search_dialog.set_query("alpha");
    let _ = app.update(Message::AdvancedFindNextRun);

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(
        document.selection,
        EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(0, 5))
    );
}
