use super::App;
use crate::core::DocumentId;
use crate::editor::EditorBuffer;
use crate::editor::render::{SyntaxParseRequest, SyntaxParseResult};
use crate::message::Message;
use iced::{Task, highlighter};
use std::sync::Arc;

#[derive(Debug, Default)]
pub(super) struct SyntaxParsing {
    next_id: u64,
    in_flight: Option<PendingParse>,
    snapshot: Option<SyntaxSnapshot>,
}

#[derive(Debug)]
struct SyntaxSnapshot {
    document: DocumentId,
    revision: u64,
    generation: Arc<()>,
    buffer: Arc<EditorBuffer>,
}

#[derive(Debug)]
struct PendingParse {
    id: u64,
    document: DocumentId,
    revision: u64,
    settings: highlighter::Settings,
    generation: Arc<()>,
}

impl App {
    pub(super) fn schedule_syntax_parse(&mut self) -> Task<Message> {
        let Some((id, request)) = self.next_syntax_request() else {
            return Task::none();
        };
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || request.parse())
                    .await
                    .map_err(|error| error.to_string())
            },
            move |result| Message::SyntaxParsed(id, result),
        )
    }

    fn next_syntax_request(&mut self) -> Option<(u64, SyntaxParseRequest)> {
        if self.session.exiting {
            return None;
        }
        let document = self.workspace.active_document()?;
        let settings = highlighter::Settings {
            token: document.render_syntax_token().to_owned(),
            theme: self.settings.syntax_theme,
        };
        let mut cache = document.syntax_cache.borrow_mut();
        cache.configure(&settings);
        // At most one bounded worker batch is outstanding, even across tab
        // switches/edits. The next batch always uses the latest active viewport.
        if self.syntax_parsing.in_flight.is_some() {
            return None;
        }
        if !document.has_complete_text_index() || settings.token == "txt" {
            self.syntax_parsing.snapshot = None;
            return None;
        }
        let last_line = document.buffer.line_count().saturating_sub(1);
        if !cache.needs_parse(last_line) {
            return None;
        }
        let revision = document.revision();
        let snapshot = &mut self.syntax_parsing.snapshot;
        if !snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.document == document.id
                && snapshot.revision == revision
                && Arc::ptr_eq(&snapshot.generation, cache.generation())
        }) {
            // Clone the rope and its line index once per document revision,
            // not on each batch or frame.
            *snapshot = Some(SyntaxSnapshot {
                document: document.id,
                revision,
                generation: cache.generation().clone(),
                buffer: Arc::new(document.buffer.clone()),
            });
        }
        let first_row = document.scroll.first_visible_row;
        let end_row = first_row.saturating_add(document.viewport_visible_rows + 1);
        let mut seen = std::collections::HashSet::new();
        // Visible rows precede lookahead/lookbehind. Mapping each row also
        // skips folded blocks and deduplicates wrapped fragments of a line.
        let priority_lines: Vec<_> = (first_row..end_row.saturating_add(32))
            .chain(first_row.saturating_sub(32)..first_row)
            .filter_map(|row| document.viewport.visible_row_to_document_line(row))
            .filter(|line| seen.insert(*line))
            .collect();
        let request =
            cache.parse_request(snapshot.as_ref().unwrap().buffer.clone(), &priority_lines);
        self.syntax_parsing.next_id += 1;
        let id = self.syntax_parsing.next_id;
        self.syntax_parsing.in_flight = Some(PendingParse {
            id,
            document: document.id,
            revision,
            settings,
            generation: cache.generation().clone(),
        });
        Some((id, request))
    }

    pub(super) fn complete_syntax_parse(
        &mut self,
        id: u64,
        result: Result<SyntaxParseResult, String>,
    ) -> Task<Message> {
        if !self
            .syntax_parsing
            .in_flight
            .as_ref()
            .is_some_and(|pending| pending.id == id)
        {
            return Task::none();
        }
        let pending = self.syntax_parsing.in_flight.take().unwrap();
        let Some(document) = self.workspace.document(pending.document) else {
            self.syntax_parsing.snapshot = None;
            return Task::none();
        };
        if document.revision() != pending.revision
            || document.render_syntax_token() != pending.settings.token
            || self.settings.syntax_theme != pending.settings.theme
        {
            return Task::none();
        }
        let mut cache = document.syntax_cache.borrow_mut();
        if !Arc::ptr_eq(cache.generation(), &pending.generation) {
            return Task::none();
        }
        match result {
            Ok(result) => {
                cache.apply_parsed(result);
            }
            Err(_) => {
                // Do not spin on a worker failure. Editing or changing syntax
                // settings invalidates this suppression along with the cache.
                cache.stop_parsing();
            }
        }
        // This message causes a redraw, and App::update schedules the next
        // batch, first filling the latest viewport and then refining context.
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let (mut app, _) = App::new();
        app.workspace
            .insert_loaded_file("example.html", &"<p>hello</p>\n".repeat(500));
        app.workspace
            .active_document_mut()
            .unwrap()
            .scroll
            .first_visible_row = 300;
        app
    }

    #[test]
    fn scrolling_coalesces_work_and_ignores_duplicate_completions() {
        let mut app = app();
        let (id, request) = app.next_syntax_request().unwrap();
        app.workspace
            .active_document_mut()
            .unwrap()
            .scroll
            .first_visible_row = 450;
        assert!(
            app.next_syntax_request().is_none(),
            "only one batch can run"
        );
        let result = request.parse();
        let _ = app.complete_syntax_parse(id, Ok(result.clone()));
        let (next_id, next) = app.next_syntax_request().unwrap();
        let _ = app.complete_syntax_parse(id, Ok(result));
        assert_eq!(app.syntax_parsing.in_flight.as_ref().unwrap().id, next_id);
        let _ = app.complete_syntax_parse(next_id, Ok(next.parse()));
        assert_eq!(
            app.workspace
                .active_document()
                .unwrap()
                .syntax_cache
                .borrow()
                .cached_line_count(),
            0,
            "both batches should prioritize their new visible ranges before context"
        );
    }

    #[test]
    fn edits_and_theme_changes_reject_pending_work() {
        let mut app = app();
        let (id, request) = app.next_syntax_request().unwrap();
        let document = app.workspace.active_document_mut().unwrap();
        document.buffer = EditorBuffer::from_text("<script>let x = 1;</script>");
        document.refresh_after_text_change();
        let _ = app.complete_syntax_parse(id, Ok(request.parse()));
        assert_eq!(
            app.workspace
                .active_document()
                .unwrap()
                .syntax_cache
                .borrow()
                .cached_line_count(),
            0
        );
        let (id, request) = app.next_syntax_request().unwrap();
        app.settings.syntax_theme = highlighter::Theme::SolarizedDark;
        let _ = app.complete_syntax_parse(id, Ok(request.parse()));
        assert!(app.next_syntax_request().is_some());
    }

    #[test]
    fn closing_or_switching_tabs_does_not_continue_obsolete_work() {
        let mut app = app();
        let (id, request) = app.next_syntax_request().unwrap();
        let document = app.workspace.active_document_id;
        app.workspace.close(document);
        let _ = app.complete_syntax_parse(id, Ok(request.parse()));
        assert!(app.next_syntax_request().is_none());
        assert!(app.syntax_parsing.snapshot.is_none());
    }

    #[test]
    fn worker_failure_does_not_start_a_retry_loop() {
        let mut app = app();
        let (id, _) = app.next_syntax_request().unwrap();
        let _ = app.complete_syntax_parse(id, Err("worker stopped".into()));
        assert!(app.next_syntax_request().is_none());
        app.workspace
            .active_document_mut()
            .unwrap()
            .syntax_cache
            .borrow_mut()
            .invalidate_from(0);
        assert!(app.next_syntax_request().is_some());
    }

    #[test]
    fn reload_with_reused_document_revision_gets_a_fresh_snapshot() {
        let mut app = app();
        let (id, request) = app.next_syntax_request().unwrap();
        let document_id = app.workspace.active_document_id;
        let old_revision = app.workspace.active_document().unwrap().revision();
        *app.workspace.document_mut(document_id).unwrap() = crate::core::Document::from_path(
            document_id,
            "example.html",
            "<script>const replacement = 42;</script>",
        );
        assert_eq!(
            app.workspace.active_document().unwrap().revision(),
            old_revision
        );
        let _ = app.complete_syntax_parse(id, Ok(request.parse()));
        while let Some((id, request)) = app.next_syntax_request() {
            let _ = app.complete_syntax_parse(id, Ok(request.parse()));
        }
        let document = app.workspace.active_document().unwrap();
        let expected = crate::editor::SyntaxLineCache::rebuild(
            &document.buffer,
            &highlighter::Settings {
                token: "html".into(),
                theme: app.settings.syntax_theme,
            },
        );
        assert_eq!(*document.syntax_cache.borrow(), expected);
    }

    #[test]
    fn completion_during_shutdown_does_not_leave_a_stuck_worker_after_failed_exit() {
        let mut app = app();
        let (id, request) = app.next_syntax_request().unwrap();
        app.session.exiting = true;
        let _ = app.update(Message::SyntaxParsed(id, Ok(request.parse())));
        assert!(app.syntax_parsing.in_flight.is_none());
        app.session.exiting = false;
        assert!(app.next_syntax_request().is_some());
    }

    #[test]
    fn syntax_results_do_not_dirty_session_state() {
        let mut app = app();
        app.session.enabled = true;
        let (id, request) = app.next_syntax_request().unwrap();
        assert!(!app.session_should_track(&Message::SyntaxParsed(id, Ok(request.parse()))));
    }
}
