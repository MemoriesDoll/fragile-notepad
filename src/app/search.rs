use iced::Task;
use iced::widget::operation;

use crate::core::{Document, PreparedSearch};
use crate::editor::{EditorSelection, position_for_byte_offset, word_range_at_position};
use crate::message::{AdvancedSearchTab, Message};
use crate::ui::find_panel::FIND_INPUT_ID;

use super::App;

impl App {
    pub(super) fn update_search(&mut self, message: Message) -> Task<Message> {
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
                Task::none()
            }
            Message::ShowInlineReplace => {
                self.is_find_visible = true;
                self.is_inline_replace_visible = true;
                operation::focus(FIND_INPUT_ID)
            }
            Message::ToggleFind => self.toggle_find_panel(),
            Message::HideFind => {
                self.is_find_visible = false;
                Task::none()
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
            Message::AdvancedSearchRun => {
                self.refresh_search_results();
                Task::none()
            }
            Message::AdvancedCountRun => {
                self.refresh_search_results();
                Task::none()
            }
            Message::AdvancedFindNextRun => {
                self.advanced_find_next();
                Task::none()
            }
            Message::AdvancedFindAllCurrentRun => {
                self.refresh_results_for(SearchScope::Current);
                Task::none()
            }
            Message::AdvancedFindAllOpenRun => {
                self.refresh_results_for(SearchScope::OpenDocuments);
                Task::none()
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
                    self.reveal_document_line(
                        document_id,
                        selection.range().normalized().start.line,
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

        if self.is_find_visible {
            operation::focus(FIND_INPUT_ID)
        } else {
            Task::none()
        }
    }

    fn toggle_advanced_search_window(&mut self, tab: AdvancedSearchTab) -> Task<Message> {
        self.active_menu = None;
        self.search_dialog.set_active_tab(tab);
        self.search_dialog.query = self.find.query.clone();
        self.search_dialog.replacement = self.find.replacement.clone();
        self.search_dialog.case_sensitive = self.find.case_sensitive;
        self.search_dialog.whole_word = self.find.whole_word;
        self.search_dialog.mode = crate::core::SearchMode::Normal;
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
        self.reveal_document_line(document_id, start_position.line);
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

        let mut changed = false;

        for text_match in matches.iter().rev() {
            let latest_text = self
                .workspace
                .active_document()
                .map(Document::text)
                .unwrap_or_default();

            changed |= self.replace_active_match(&latest_text, text_match.start, text_match.end);
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
        let replacement = self.search_dialog.replacement.clone();
        let Some(search) = self.prepare_advanced_search() else {
            return Task::none();
        };
        let document_ids = self.document_ids_for_scope(scope);

        let mut changed_documents = Vec::new();

        for document_id in document_ids {
            let Some(document) = self.workspace.document(document_id) else {
                continue;
            };
            if !document.has_complete_text_index() {
                continue;
            }
            let text = document.text();
            let matches = search.matches(&text);
            let mut document_changed = false;

            for text_match in matches.iter().rev() {
                let latest_text = self
                    .workspace
                    .document(document_id)
                    .map(Document::text)
                    .unwrap_or_default();
                let replacement =
                    search.replacement_for_match(&latest_text, *text_match, &replacement);

                document_changed |= self.replace_document_range_with(
                    document_id,
                    &latest_text,
                    text_match.start,
                    text_match.end,
                    replacement,
                );
            }

            if document_changed {
                changed_documents.push(document_id);
            }
        }

        self.refresh_find_matches();
        self.refresh_results_for(scope);

        Task::batch(
            changed_documents
                .into_iter()
                .map(|document_id| self.schedule_outline_parse(document_id)),
        )
    }

    fn refresh_search_results(&mut self) {
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

    fn refresh_results_for(&mut self, scope: SearchScope) {
        match scope {
            SearchScope::Current => {
                let Some(document) = self.workspace.active_document() else {
                    self.search_dialog.results.clear();
                    self.search_dialog.status = String::from("No document");
                    return;
                };

                self.search_dialog.refresh_from_documents([document]);
            }
            SearchScope::OpenDocuments => {
                self.search_dialog.refresh_from_workspace(&self.workspace);
            }
        }
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

    fn reveal_document_line(&mut self, document_id: crate::core::DocumentId, line: usize) {
        let Some(document) = self.workspace.document_mut(document_id) else {
            return;
        };
        document.reveal_line(line);
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
