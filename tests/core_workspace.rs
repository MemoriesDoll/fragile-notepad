use fragile_notepad::core::Workspace;

#[test]
fn pinning_moves_document_to_pinned_group_without_changing_active_document() {
    let mut workspace = Workspace::new();
    let first = workspace.active_document_id;
    let second = workspace.create_untitled();
    let third = workspace.create_untitled();

    assert!(workspace.toggle_pin(third));

    assert_eq!(
        workspace
            .documents()
            .iter()
            .map(|document| document.id)
            .collect::<Vec<_>>(),
        vec![third, first, second]
    );
    assert_eq!(workspace.pinned_count(), 1);
    assert_eq!(workspace.active_document_id, third);
}

#[test]
fn unpinning_moves_document_to_end_of_unpinned_group() {
    let mut workspace = Workspace::new();
    let first = workspace.active_document_id;
    let second = workspace.create_untitled();
    let third = workspace.create_untitled();

    workspace.toggle_pin(second);
    workspace.toggle_pin(third);
    assert_eq!(workspace.pinned_count(), 2);

    assert!(workspace.toggle_pin(second));

    assert_eq!(
        workspace
            .documents()
            .iter()
            .map(|document| document.id)
            .collect::<Vec<_>>(),
        vec![third, first, second]
    );
    assert_eq!(workspace.pinned_count(), 1);
}

#[test]
fn reorder_forward_places_document_after_target() {
    let mut workspace = Workspace::new();
    let first = workspace.active_document_id;
    let second = workspace.create_untitled();
    let third = workspace.create_untitled();

    assert!(workspace.reorder(first, second));

    assert_eq!(
        workspace
            .documents()
            .iter()
            .map(|document| document.id)
            .collect::<Vec<_>>(),
        vec![second, first, third]
    );
}

#[test]
fn reorder_backward_places_document_before_target() {
    let mut workspace = Workspace::new();
    let first = workspace.active_document_id;
    let second = workspace.create_untitled();
    let third = workspace.create_untitled();

    assert!(workspace.reorder(third, first));

    assert_eq!(
        workspace
            .documents()
            .iter()
            .map(|document| document.id)
            .collect::<Vec<_>>(),
        vec![third, first, second]
    );
}

#[test]
fn reorder_rejects_crossing_pinned_boundary() {
    let mut workspace = Workspace::new();
    let first = workspace.active_document_id;
    let second = workspace.create_untitled();

    workspace.toggle_pin(second);

    assert!(!workspace.reorder(first, second));
    assert_eq!(
        workspace
            .documents()
            .iter()
            .map(|document| document.id)
            .collect::<Vec<_>>(),
        vec![second, first]
    );
}
