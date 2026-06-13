use fragile_notepad::core::search::{
    FindState, PreparedSearch, SearchMode, SearchOptions, TextMatch, compute_matches, replace_all,
    replace_current,
};

#[test]
fn compute_matches_respects_case_sensitivity() {
    assert_eq!(
        compute_matches("Note note NOTE", "note", true),
        vec![TextMatch::new(5, 9)]
    );

    assert_eq!(
        compute_matches("Note note NOTE", "note", false),
        vec![
            TextMatch::new(0, 4),
            TextMatch::new(5, 9),
            TextMatch::new(10, 14),
        ]
    );
}

#[test]
fn compute_matches_returns_no_matches_for_empty_input_or_query() {
    assert!(compute_matches("", "note", false).is_empty());
    assert!(compute_matches("note", "", false).is_empty());
}

#[test]
fn replace_current_rejects_invalid_ranges() {
    assert_eq!(replace_current("abc", TextMatch::new(2, 1), "x"), None);
    assert_eq!(replace_current("abc", TextMatch::new(0, 4), "x"), None);
    assert_eq!(replace_current("éx", TextMatch::new(1, 2), "x"), None);
}

#[test]
fn find_state_refreshes_and_navigates_matches_with_wrapping() {
    let mut find = FindState::with_query("one");

    find.refresh_matches("one two one");

    assert_eq!(find.current(), Some(TextMatch::new(0, 3)));
    assert_eq!(find.next(), Some(TextMatch::new(8, 11)));
    assert_eq!(find.next(), Some(TextMatch::new(0, 3)));
    assert_eq!(find.previous(), Some(TextMatch::new(8, 11)));
}

#[test]
fn find_state_clears_navigation_when_query_has_no_matches() {
    let mut find = FindState::with_query("missing");

    find.refresh_matches("one two one");

    assert!(find.matches.is_empty());
    assert_eq!(find.current(), None);
    assert_eq!(find.next(), None);
    assert_eq!(find.previous(), None);
}

#[test]
fn find_state_replace_current_replaces_selected_match_and_refreshes() {
    let mut find = FindState::with_query("cat");
    find.set_replacement("dog");
    find.refresh_matches("cat cat");

    let replaced = find.replace_current("cat cat");

    assert_eq!(replaced.as_deref(), Some("dog cat"));
    assert_eq!(find.matches, vec![TextMatch::new(4, 7)]);
    assert_eq!(find.current(), Some(TextMatch::new(4, 7)));
}

#[test]
fn find_state_replace_all_replaces_all_matches_and_reports_count() {
    let mut find = FindState::with_query("cat");
    find.set_replacement("dog");
    find.refresh_matches("cat Cat scatter cat");

    let (replaced, count) = find.replace_all("cat Cat scatter cat");

    assert_eq!(replaced, "dog dog sdogter dog");
    assert_eq!(count, 4);
    assert!(find.matches.is_empty());
    assert_eq!(find.current(), None);
}

#[test]
fn replace_all_helper_uses_case_sensitive_matching_when_requested() {
    let (replaced, count) = replace_all("cat Cat cat", "cat", "dog", true);

    assert_eq!(replaced, "dog Cat dog");
    assert_eq!(count, 2);
}

#[test]
fn compute_matches_reports_utf8_byte_offsets_for_multiline_text() {
    let text = "one\n茅cho\none";

    assert_eq!(
        compute_matches(text, "one", true),
        vec![TextMatch::new(0, 3), TextMatch::new(11, 14)]
    );
    assert_eq!(
        compute_matches(text, "茅c", true),
        vec![TextMatch::new(4, 8)]
    );
}

#[test]
fn replace_current_handles_multibyte_boundaries_and_preserves_surrounding_text() {
    let text = "alpha 茅cho omega";
    let start = "alpha ".len();
    let end = start + "茅cho".len();

    assert_eq!(
        replace_current(text, TextMatch::new(start, end), "echo").as_deref(),
        Some("alpha echo omega")
    );
}

#[test]
fn find_state_replace_current_uses_current_match_after_navigation() {
    let mut find = FindState::with_query("one");
    find.set_replacement("two");
    find.refresh_matches("one one one");

    assert_eq!(find.next(), Some(TextMatch::new(4, 7)));

    let replaced = find.replace_current("one one one");

    assert_eq!(replaced.as_deref(), Some("one two one"));
    assert_eq!(
        find.matches,
        vec![TextMatch::new(0, 3), TextMatch::new(8, 11)]
    );
    assert_eq!(find.current(), Some(TextMatch::new(8, 11)));
}

#[test]
fn prepared_regex_search_expands_capture_replacements() {
    let search = PreparedSearch::new(
        r"(\w+)-(\d+)",
        SearchOptions {
            case_sensitive: true,
            whole_word: false,
            mode: SearchMode::Regex,
        },
    )
    .unwrap()
    .unwrap();
    let text_match = search.matches("task-42").remove(0);

    assert_eq!(
        search.replacement_for_match("task-42", text_match, "$2:$1"),
        "42:task"
    );
}

#[test]
fn prepared_extended_search_expands_query_and_replacement_escapes() {
    let search = PreparedSearch::new(
        r"one\ntwo",
        SearchOptions {
            case_sensitive: true,
            whole_word: false,
            mode: SearchMode::Extended,
        },
    )
    .unwrap()
    .unwrap();
    let text = "zero\none\ntwo\nthree";
    let text_match = search.matches(text).remove(0);

    assert_eq!(
        text_match,
        TextMatch::new("zero\n".len(), "zero\none\ntwo".len())
    );
    assert_eq!(
        search.replacement_for_match(text, text_match, r"alpha\tbeta"),
        "alpha\tbeta"
    );
}
