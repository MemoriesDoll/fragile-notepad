use iced::Task;
use iced::widget::operation;

use crate::core::{Document, PreparedSearch};
use crate::editor::{
    EditorRange, EditorSelection, position_for_byte_offset, word_range_at_position,
};
use crate::message::{AdvancedSearchTab, Message};
use crate::ui::find_panel::FIND_INPUT_ID;

use super::App;

#[derive(Debug)]
pub(super) struct PendingSearch {
    dialog: crate::search_dialog::SearchDialogState,
    search: PreparedSearch,
    documents: Vec<crate::core::DocumentId>,
    replace: bool,
}

impl App {
    pub(super) fn update_search(&mut self, message: Message) -> Task<Message> {
        // Editing the request or starting a new search cancels its queued work.
        // Loading already requested documents may finish, but cannot mutate text.
        if !matches!(message, Message::AdvancedSearchResultSelected(_, _)) {
            if self.pending_search.is_some() {
                self.search_dialog.status = String::from("Search canceled.");
            }
            self.pending_search = None;
        }
        if let Some(document) = self.workspace.active_document_mut() {
            document.sync_selection_mirror();
        }

        match message {
            Message::FindQueryChanged(query) => {
                self.find.set_query(query);
                self.refresh_find_matches();
                Task::none()
            }
            Message::FindReplacementChanged(replacement) => {
                self.find.set_replacement(replacement);
                Task::none()
            }
            Message::FindCaseSensitiveToggled(case_sensitive) => {
                self.find.set_case_sensitive(case_sensitive);
                self.refresh_find_matches();
                Task::none()
            }
            Message::FindWholeWordToggled(whole_word) => {
                self.find.set_whole_word(whole_word);
                self.refresh_find_matches();
                Task::none()
            }
            Message::ToggleInlineReplace => {
                self.is_inline_replace_visible = !self.is_inline_replace_visible;
                self.chrome_animation
                    .inline_replace
                    .set_visible(self.is_inline_replace_visible);
                Task::none()
            }
            Message::ShowInlineReplace => {
                self.is_find_visible = true;
                self.is_inline_replace_visible = true;
                self.chrome_animation.find.set_visible(true);
                self.chrome_animation.inline_replace.set_visible(true);
                operation::focus(FIND_INPUT_ID)
            }
            Message::ToggleFind => self.toggle_find_panel(),
            Message::HideFind => {
                self.is_find_visible = false;
                self.chrome_animation.find.set_visible(false);
                operation::focus(crate::ui::editor::EDITOR_ID)
            }
            Message::FindNext => {
                self.active_menu = None;
                let text_match = self.find.next();
                self.select_active_match(text_match);
                Task::none()
            }
            Message::FindPrevious => {
                self.active_menu = None;
                let text_match = self.find.previous();
                self.select_active_match(text_match);
                Task::none()
            }
            Message::SelectAndFindNext => {
                self.select_text_for_find(true, true);
                Task::none()
            }
            Message::SelectAndFindPrevious => {
                self.select_text_for_find(true, false);
                Task::none()
            }
            Message::VolatileFindNext => {
                self.select_text_for_find(false, true);
                Task::none()
            }
            Message::VolatileFindPrevious => {
                self.select_text_for_find(false, false);
                Task::none()
            }
            Message::ReplaceCurrent => self.replace_current(),
            Message::ReplaceAll => self.replace_all(),
            Message::ToggleAdvancedSearch(tab) => self.toggle_advanced_search_window(tab),
            Message::AdvancedSearchTabSelected(tab) => {
                self.search_dialog.set_active_tab(tab);
                self.refresh_search_results();
                Task::none()
            }
            Message::AdvancedSearchQueryChanged(query) => {
                self.search_dialog.set_query(query);
                Task::none()
            }
            Message::AdvancedSearchReplacementChanged(replacement) => {
                self.search_dialog.set_replacement(replacement);
                Task::none()
            }
            Message::AdvancedSearchCaseSensitiveToggled(case_sensitive) => {
                self.search_dialog.set_case_sensitive(case_sensitive);
                Task::none()
            }
            Message::AdvancedSearchWholeWordToggled(whole_word) => {
                self.search_dialog.set_whole_word(whole_word);
                Task::none()
            }
            Message::AdvancedSearchWrapAroundToggled(wrap_around) => {
                self.search_dialog.set_wrap_around(wrap_around);
                Task::none()
            }
            Message::AdvancedSearchModeSelected(mode) => {
                self.search_dialog.set_mode(mode);
                self.refresh_search_results();
                Task::none()
            }
            Message::AdvancedSearchIncludeChanged(include_pattern) => {
                self.search_dialog.set_include_pattern(include_pattern);
                Task::none()
            }
            Message::AdvancedSearchRun | Message::AdvancedCountRun => {
                self.begin_pending_search(self.dialog_scope(), false)
            }
            Message::AdvancedFindNextRun => {
                if matches!(self.search_dialog.active_tab, AdvancedSearchTab::GoToLine) {
                    self.go_to_line();
                } else {
                    self.advanced_find_next();
                }
                Task::none()
            }
            Message::AdvancedFindAllCurrentRun => {
                self.begin_pending_search(SearchScope::Current, false)
            }
            Message::AdvancedFindAllOpenRun => {
                self.begin_pending_search(SearchScope::OpenDocuments, false)
            }
            Message::AdvancedReplaceRun => self.advanced_replace_current(),
            Message::AdvancedReplaceAllRun => self.advanced_replace_all(),
            Message::AdvancedReplaceAllCurrentRun => self.replace_all_in(SearchScope::Current),
            Message::AdvancedReplaceAllOpenRun => self.replace_all_in(SearchScope::OpenDocuments),
            Message::AdvancedSearchResultSelected(document_id, selection) => {
                if self.workspace.select(document_id) {
                    self.refresh_find_matches();
                    let _ = self.update_editor(
                        document_id,
                        crate::editor::EditorAction::SelectRegion(selection),
                    );
                    self.reveal_document_position(
                        document_id,
                        selection.range().normalized().start,
                    );
                }
                Task::none()
            }
            Message::AdvancedSearchClosed => self.close_advanced_search_window(),
            _ => unreachable!("search handler received non-search message"),
        }
    }

    fn toggle_find_panel(&mut self) -> Task<Message> {
        self.active_menu = None;
        self.is_find_visible = !self.is_find_visible;
        self.chrome_animation.find.set_visible(self.is_find_visible);

        if self.is_find_visible {
            self.chrome_animation
                .inline_replace
                .set_visible(self.is_inline_replace_visible);
            operation::focus(FIND_INPUT_ID)
        } else {
            operation::focus(crate::ui::editor::EDITOR_ID)
        }
    }

    fn toggle_advanced_search_window(&mut self, tab: AdvancedSearchTab) -> Task<Message> {
        self.active_menu = None;
        self.search_dialog.set_active_tab(tab);
        if !matches!(tab, AdvancedSearchTab::GoToLine) {
            self.search_dialog.query = self.find.query.clone();
            self.search_dialog.replacement = self.find.replacement.clone();
            self.search_dialog.case_sensitive = self.find.case_sensitive;
            self.search_dialog.whole_word = self.find.whole_word;
            self.search_dialog.mode = crate::core::SearchMode::Normal;
        }
        self.refresh_search_results();
        self.open_advanced_search_window()
    }

    fn replace_current(&mut self) -> Task<Message> {
        let Some(document) = self.workspace.active_document() else {
            return Task::none();
        };
        if !document.has_complete_text_index() {
            return Task::none();
        }
        let text = document.text();

        let Some(text_match) = self.find.current() else {
            return Task::none();
        };

        if self.replace_active_match(&text, text_match.start, text_match.end) {
            self.refresh_find_matches();
            return self.schedule_outline_parse(self.workspace.active_document_id);
        }

        Task::none()
    }

    fn select_text_for_find(&mut self, persist_query: bool, forward: bool) {
        self.active_menu = None;

        let Some(query) = self.active_find_text() else {
            return;
        };

        let previous_query = self.find.query.clone();
        let previous_case_sensitive = self.find.case_sensitive;
        let previous_whole_word = self.find.whole_word;

        self.find.set_query(query);
        if !persist_query {
            self.find.set_case_sensitive(false);
            self.find.set_whole_word(false);
        }
        self.refresh_find_matches();

        let text_match = if forward {
            self.find.next()
        } else {
            self.find.previous()
        };
        self.select_active_match(text_match);

        if !persist_query {
            self.find.query = previous_query;
            self.find.case_sensitive = previous_case_sensitive;
            self.find.whole_word = previous_whole_word;
            self.refresh_find_matches();
        }
    }

    fn active_find_text(&self) -> Option<String> {
        let document = self.workspace.active_document()?;
        let range = document
            .buffer
            .clamp_range(document.main_selection().range());

        if !range.is_empty() {
            return Some(document.buffer.slice_text(range));
        }

        let range = word_range_at_position(&document.buffer, range.start, &document.syntax_token)?;

        Some(document.buffer.slice_text(range))
    }

    fn select_active_match(&mut self, text_match: Option<crate::core::TextMatch>) {
        let Some(text_match) = text_match else {
            return;
        };
        let Some(document) = self.workspace.active_document() else {
            return;
        };
        let Some(start_position) = document.buffer.position_for_byte_offset(text_match.start)
        else {
            return;
        };
        let Some(end_position) = document.buffer.position_for_byte_offset(text_match.end) else {
            return;
        };
        let document_id = self.workspace.active_document_id;

        let _ = self.update_editor(
            document_id,
            crate::editor::EditorAction::SelectRegion(EditorSelection::new(
                start_position,
                end_position,
            )),
        );
        self.reveal_document_position(document_id, start_position);
    }

    fn replace_all(&mut self) -> Task<Message> {
        let Some(document) = self.workspace.active_document() else {
            return Task::none();
        };
        if !document.has_complete_text_index() {
            return Task::none();
        }
        let text = document.text();

        let matches = crate::core::search::compute_matches_with_options(
            &text,
            &self.find.query,
            crate::core::SearchOptions::normal(self.find.case_sensitive, self.find.whole_word),
        );

        if matches.is_empty() {
            self.find.refresh_matches(&text);
            return Task::none();
        }

        let replacements = matches
            .into_iter()
            .filter_map(|found| {
                Some((
                    EditorRange::new(
                        document.buffer.position_for_byte_offset(found.start)?,
                        document.buffer.position_for_byte_offset(found.end)?,
                    ),
                    self.find.replacement.clone(),
                ))
            })
            .collect();
        let document = self
            .workspace
            .active_document_mut()
            .expect("active document");
        let changed = super::editor_ops::replace_ranges_for_search(document, replacements);
        if changed {
            document.ensure_caret_visible();
        }

        self.refresh_find_matches();
        if changed {
            return self.schedule_outline_parse(self.workspace.active_document_id);
        }

        Task::none()
    }

    fn replace_active_match(&mut self, text: &str, start: usize, end: usize) -> bool {
        let Some(start_position) = position_for_byte_offset(text, start) else {
            return false;
        };
        let Some(end_position) = position_for_byte_offset(text, end) else {
            return false;
        };

        self.replace_active_document_range(
            start_position,
            end_position,
            self.find.replacement.clone(),
        )
    }

    fn advanced_replace_current(&mut self) -> Task<Message> {
        let Some(document) = self.workspace.active_document() else {
            return Task::none();
        };
        if !document.has_complete_text_index() {
            return Task::none();
        }
        let text = document.text();
        let Some(search) = self.prepare_advanced_search() else {
            return Task::none();
        };
        let matches = search.matches(&text);
        let Some(text_match) = current_selection_match(self.workspace.active_document(), &matches)
            .or_else(|| matches.first().copied())
        else {
            self.refresh_search_results();
            return Task::none();
        };

        let replacement =
            search.replacement_for_match(&text, text_match, &self.search_dialog.replacement);
        if self.replace_active_range_with(&text, text_match.start, text_match.end, replacement) {
            self.refresh_search_results();
            return self.schedule_outline_parse(self.workspace.active_document_id);
        }

        Task::none()
    }

    fn advanced_replace_all(&mut self) -> Task<Message> {
        let scope = if matches!(
            self.search_dialog.active_tab,
            AdvancedSearchTab::FindInFiles | AdvancedSearchTab::ReplaceInFiles
        ) {
            SearchScope::OpenDocuments
        } else {
            SearchScope::Current
        };

        self.replace_all_in(scope)
    }

    fn replace_all_in(&mut self, scope: SearchScope) -> Task<Message> {
        self.begin_pending_search(scope, true)
    }

    fn dialog_scope(&self) -> SearchScope {
        if matches!(
            self.search_dialog.active_tab,
            AdvancedSearchTab::FindInFiles | AdvancedSearchTab::ReplaceInFiles
        ) {
            SearchScope::OpenDocuments
        } else {
            SearchScope::Current
        }
    }

    fn begin_pending_search(&mut self, scope: SearchScope, replace: bool) -> Task<Message> {
        self.pending_search = None;
        if matches!(self.search_dialog.active_tab, AdvancedSearchTab::GoToLine) && !replace {
            self.go_to_line();
            return Task::none();
        }
        let Some(search) = self.prepare_advanced_search() else {
            return Task::none();
        };
        let documents = self.document_ids_for_scope(scope);
        let deferred = documents
            .iter()
            .copied()
            .filter(|id| {
                self.workspace.document(*id).is_some_and(|document| {
                    matches!(
                        document.load_state,
                        crate::core::document::DocumentLoadState::Deferred { .. }
                    )
                })
            })
            .collect::<Vec<_>>();
        self.pending_search = Some(PendingSearch {
            dialog: self.search_dialog.clone(),
            search,
            documents,
            replace,
        });
        let mut tasks = deferred
            .into_iter()
            .map(|id| self.activate_document(id))
            .collect::<Vec<_>>();
        tasks.push(self.resume_pending_search());
        Task::batch(tasks)
    }

    pub(super) fn resume_pending_search(&mut self) -> Task<Message> {
        let Some(pending) = self.pending_search.as_ref() else {
            return Task::none();
        };
        if pending.documents.iter().any(|id| {
            self.workspace.document(*id).is_none_or(|document| {
                matches!(
                    document.load_state,
                    crate::core::document::DocumentLoadState::Failed { .. }
                )
            })
        }) {
            self.pending_search = None;
            self.search_dialog.results.clear();
            self.search_dialog.status = String::from(
                "Search canceled: a target document was closed or could not be loaded. No replacements were made.",
            );
            return Task::none();
        }
        let waiting = pending
            .documents
            .iter()
            .filter(|id| {
                self.workspace
                    .document(**id)
                    .is_some_and(|document| !document.has_complete_text_index())
            })
            .count();
        if waiting > 0 {
            self.search_dialog.results.clear();
            self.search_dialog.status = format!("Loading {waiting} documents for search...");
            return Task::none();
        }
        let pending = self.pending_search.take().expect("ready search");
        let mut changed_documents = Vec::new();
        let active_id = self.workspace.active_document_id;
        if pending.replace {
            for document_id in &pending.documents {
                let Some(document) = self.workspace.document_mut(*document_id) else {
                    continue;
                };
                if !document.has_complete_text_index() {
                    continue;
                }
                let text = document.text();
                let matches = pending.search.matches(&text);
                let replacements = matches
                    .into_iter()
                    .filter_map(|found| {
                        Some((
                            EditorRange::new(
                                document.buffer.position_for_byte_offset(found.start)?,
                                document.buffer.position_for_byte_offset(found.end)?,
                            ),
                            pending.search.replacement_for_match(
                                &text,
                                found,
                                &pending.dialog.replacement,
                            ),
                        ))
                    })
                    .collect();
                let document_changed =
                    super::editor_ops::replace_ranges_for_search(document, replacements);

                if document_changed {
                    if *document_id == active_id {
                        document.ensure_caret_visible();
                    }
                    changed_documents.push(*document_id);
                }
            }
        }
        self.refresh_find_matches();
        let mut completed = pending.dialog;
        completed.refresh_from_documents(
            pending
                .documents
                .iter()
                .filter_map(|id| self.workspace.document(*id)),
        );
        self.search_dialog.results = completed.results;
        self.search_dialog.status = completed.status;

        Task::batch(
            changed_documents
                .into_iter()
                .map(|document_id| self.schedule_outline_parse(document_id)),
        )
    }

    fn refresh_search_results(&mut self) {
        if matches!(self.search_dialog.active_tab, AdvancedSearchTab::GoToLine) {
            self.search_dialog.results.clear();
            self.search_dialog.status = if self.search_dialog.go_to_line.trim().is_empty() {
                String::from("No line")
            } else {
                String::from("Ready")
            };
            return;
        }

        if matches!(
            self.search_dialog.active_tab,
            AdvancedSearchTab::FindInFiles | AdvancedSearchTab::ReplaceInFiles
        ) {
            self.search_dialog.refresh_from_workspace(&self.workspace);
            return;
        }

        let Some(document) = self.workspace.active_document() else {
            self.search_dialog.results.clear();
            self.search_dialog.status = String::from("No document");
            return;
        };

        self.search_dialog.refresh_from_documents([document]);
    }

    fn advanced_find_next(&mut self) {
        let Some(search) = self.prepare_advanced_search() else {
            return;
        };
        if self.find.query != self.search_dialog.query {
            self.find.set_query(self.search_dialog.query.clone());
        }
        self.find
            .set_case_sensitive(self.search_dialog.case_sensitive);
        self.find.set_whole_word(self.search_dialog.whole_word);
        let Some(document) = self.workspace.active_document() else {
            self.search_dialog.results.clear();
            self.search_dialog.status = String::from("No document");
            return;
        };
        if !document.has_complete_text_index() {
            self.search_dialog.refresh_from_documents([document]);
            return;
        }
        let text = document.text();
        let matches = search.matches(&text);

        let text_match = next_match_after_selection(self.workspace.active_document(), &matches)
            .or_else(|| {
                self.search_dialog
                    .wrap_around
                    .then(|| matches.first().copied())
                    .flatten()
            });
        self.select_active_match(text_match);
    }

    fn go_to_line(&mut self) {
        self.search_dialog.results.clear();
        let input = self.search_dialog.go_to_line.trim();

        let Some(document) = self.workspace.active_document_mut() else {
            self.search_dialog.status = String::from("No document");
            return;
        };

        if input.is_empty() {
            self.search_dialog.status = String::from("No line");
            return;
        }

        let Ok(line_number) = input.parse::<usize>() else {
            self.search_dialog.status = String::from("Invalid line number");
            return;
        };

        let line_count = document.buffer.line_count();
        let last_line = line_count.saturating_sub(1);
        let target_line = line_number.saturating_sub(1).min(last_line);
        let position = document
            .buffer
            .clamp_position(crate::editor::EditorPosition::new(target_line, 0));

        document.set_main_selection(EditorSelection::new(position, position));
        document.reveal_position(position);
        self.search_dialog.status = format!("Line {} of {}", target_line + 1, line_count);
    }

    fn prepare_advanced_search(&mut self) -> Option<PreparedSearch> {
        match PreparedSearch::new(&self.search_dialog.query, self.search_dialog.options()) {
            Ok(Some(search)) => Some(search),
            Ok(None) => {
                self.search_dialog.results.clear();
                self.search_dialog.status = String::from("No query");
                None
            }
            Err(error) => {
                self.search_dialog.results.clear();
                self.search_dialog.status = crate::search_dialog::search_error_status(error);
                None
            }
        }
    }

    fn document_ids_for_scope(&self, scope: SearchScope) -> Vec<crate::core::DocumentId> {
        match scope {
            SearchScope::Current => vec![self.workspace.active_document_id],
            SearchScope::OpenDocuments => self
                .workspace
                .documents()
                .iter()
                .filter(|document| {
                    crate::search_dialog::include_filter_matches(
                        document,
                        &self.search_dialog.include_pattern,
                    )
                })
                .map(|document| document.id)
                .collect(),
        }
    }

    fn replace_active_range_with(
        &mut self,
        text: &str,
        start: usize,
        end: usize,
        replacement: String,
    ) -> bool {
        self.replace_document_range_with(
            self.workspace.active_document_id,
            text,
            start,
            end,
            replacement,
        )
    }

    fn replace_document_range_with(
        &mut self,
        document_id: crate::core::DocumentId,
        text: &str,
        start: usize,
        end: usize,
        replacement: String,
    ) -> bool {
        let Some(start_position) = position_for_byte_offset(text, start) else {
            return false;
        };
        let Some(end_position) = position_for_byte_offset(text, end) else {
            return false;
        };

        let Some(document) = self.workspace.document_mut(document_id) else {
            return false;
        };

        document.set_main_selection(EditorSelection::new(start_position, end_position));
        let changed = super::editor_ops::replace_selection_for_search(document, &replacement);

        if changed && document_id == self.workspace.active_document_id {
            self.refresh_find_matches();
        }

        changed
    }

    fn reveal_document_position(
        &mut self,
        document_id: crate::core::DocumentId,
        position: crate::editor::EditorPosition,
    ) {
        let Some(document) = self.workspace.document_mut(document_id) else {
            return;
        };
        document.reveal_position(position);
    }
}

#[derive(Debug, Clone, Copy)]
enum SearchScope {
    Current,
    OpenDocuments,
}

fn current_selection_match(
    document: Option<&Document>,
    matches: &[crate::core::TextMatch],
) -> Option<crate::core::TextMatch> {
    let document = document?;
    let range = document.main_selection().range();
    let start = document.buffer.byte_offset(range.start);
    let end = document.buffer.byte_offset(range.end);

    matches
        .iter()
        .copied()
        .find(|text_match| start == text_match.start && end == text_match.end)
}

fn next_match_after_selection(
    document: Option<&Document>,
    matches: &[crate::core::TextMatch],
) -> Option<crate::core::TextMatch> {
    let document = document?;
    let cursor = document
        .buffer
        .byte_offset(document.main_selection().range().normalized().end);

    matches
        .iter()
        .copied()
        .find(|text_match| text_match.start >= cursor)
}
