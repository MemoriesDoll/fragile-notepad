use super::{DEFAULT_SYNTAX_TOKEN, SyntaxRenderSpan};
use crate::editor::buffer::EditorBuffer;
use iced::advanced::text::Highlighter as _;
use iced::highlighter;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub struct SyntaxLineCache {
    settings: Option<highlighter::Settings>,
    lines: Vec<Vec<SyntaxRenderSpan>>,
    highlighter: Option<iced::highlighter::Highlighter>,
    generation: Arc<()>,
    failed: bool,
    preview: BTreeMap<usize, Vec<SyntaxRenderSpan>>,
    preview_parser: Option<PreviewParser>,
    preview_anchor: usize,
    preview_batches: usize,
}

impl Clone for SyntaxLineCache {
    fn clone(&self) -> Self {
        Self {
            settings: self.settings.clone(),
            lines: self.lines.clone(),
            // Parser snapshots are not cloneable. Resume a cloned cache by
            // replaying context on the worker, keeping its valid spans visible.
            highlighter: None,
            ..Self::default()
        }
    }
}

impl PartialEq for SyntaxLineCache {
    fn eq(&self, other: &Self) -> bool {
        self.settings == other.settings && self.lines == other.lines
    }
}

impl SyntaxLineCache {
    /// Synchronous reference path for offline callers and tests. UI drawing
    /// uses only cached spans; the application schedules progressive requests.
    pub fn rebuild(buffer: &EditorBuffer, settings: &highlighter::Settings) -> Self {
        let mut cache = Self::new(settings);
        cache.ensure_visible(buffer, settings, 0, buffer.line_count().saturating_sub(1));
        cache
    }

    pub fn new(settings: &highlighter::Settings) -> Self {
        if !uses_syntax_highlighting(settings) {
            return Self::default();
        }

        Self {
            settings: Some(settings.clone()),
            lines: Vec::new(),
            highlighter: Some(iced::highlighter::Highlighter::new(settings)),
            ..Self::default()
        }
    }

    pub fn is_current(&self, settings: &highlighter::Settings, line_count: usize) -> bool {
        if uses_syntax_highlighting(settings) {
            self.settings.as_ref() == Some(settings) && self.lines.len() >= line_count
        } else {
            self.settings.is_none() && self.lines.is_empty()
        }
    }

    pub fn is_compatible(&self, settings: &highlighter::Settings) -> bool {
        if uses_syntax_highlighting(settings) {
            self.settings.as_ref() == Some(settings)
        } else {
            self.settings.is_none()
        }
    }

    pub fn ensure_visible(
        &mut self,
        buffer: &EditorBuffer,
        settings: &highlighter::Settings,
        _first_line: usize,
        last_line: usize,
    ) {
        if !uses_syntax_highlighting(settings) {
            self.clear();
            return;
        }

        if !self.is_compatible(settings) {
            *self = Self::new(settings);
        }

        let target_last_line = last_line.min(buffer.line_count().saturating_sub(1));
        if target_last_line < self.lines.len() {
            return;
        }

        let highlighter = self
            .highlighter
            .get_or_insert_with(|| highlighter::Highlighter::new(settings));
        self.lines.truncate(highlighter.current_line());

        while self.lines.len() <= target_last_line {
            let line = self.lines.len();
            let text = buffer.line(line).unwrap_or_default();
            self.lines.push(syntax_span_plan(highlighter, &text));
        }
    }

    pub fn invalidate_from(&mut self, line: usize) {
        self.generation = Arc::new(());
        self.failed = false;
        self.preview.clear();
        self.preview_parser = None;
        self.preview_batches = 0;
        if let Some(highlighter) = self.highlighter.as_mut() {
            highlighter.change_line(line);
            self.lines
                .truncate(highlighter.current_line().min(self.lines.len()));
        } else {
            self.lines.truncate(line.min(self.lines.len()));
        }
    }

    pub fn cached_line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn clear(&mut self) {
        self.settings = None;
        self.lines.clear();
        self.highlighter = None;
        self.generation = Arc::new(());
        self.failed = false;
        self.preview.clear();
        self.preview_parser = None;
        self.preview_batches = 0;
    }

    pub(super) fn spans(&self, line: usize) -> Option<Vec<SyntaxRenderSpan>> {
        self.lines
            .get(line)
            .or_else(|| self.preview.get(&line))
            .cloned()
    }

    /// Invalidates incompatible spans without initializing or running a parser.
    pub fn configure(&mut self, settings: &highlighter::Settings) {
        if !self.is_compatible(settings) {
            self.clear();
            if uses_syntax_highlighting(settings) {
                self.settings = Some(settings.clone());
            }
        }
    }

    pub(crate) fn needs_parse(&self, last_line: usize) -> bool {
        self.settings.is_some() && !self.failed && self.lines.len() <= last_line
    }

    pub(crate) fn generation(&self) -> &Arc<()> {
        &self.generation
    }

    /// Prefer missing visible lines, then their surroundings. Interleave exact
    /// context batches so continuous scrolling cannot starve refinement.
    /// The caller permits only one outstanding request at a time.
    pub(crate) fn parse_request(
        &mut self,
        buffer: Arc<EditorBuffer>,
        priority_lines: &[usize],
    ) -> SyntaxParseRequest {
        self.preview_anchor = priority_lines.first().copied().unwrap_or(0);
        let missing: Vec<_> = priority_lines
            .iter()
            .copied()
            .filter(|line| {
                *line < buffer.line_count()
                    && *line >= self.lines.len()
                    && !self.preview.contains_key(line)
            })
            .collect();
        let work = if !missing.is_empty() && self.preview_batches < 2 {
            self.preview_batches += 1;
            SyntaxWork::Viewport {
                targets: missing,
                parser: self.preview_parser.take(),
            }
        } else {
            self.preview_batches = 0;
            SyntaxWork::Context(self.highlighter.take())
        };
        SyntaxParseRequest {
            buffer,
            settings: self.settings.clone().expect("configured syntax"),
            generation: self.generation.clone(),
            work,
        }
    }

    pub(crate) fn apply_parsed(&mut self, result: SyntaxParseResult) -> bool {
        if !Arc::ptr_eq(&self.generation, &result.generation)
            || self.settings.as_ref() != Some(&result.settings)
        {
            return false;
        }
        let Some(chunk) = result.chunk.lock().expect("syntax result").take() else {
            return false;
        };
        match chunk {
            SyntaxChunk::Context {
                first_line,
                lines,
                highlighter,
            } => {
                if first_line > self.lines.len() {
                    return false;
                }
                // Replay must not erase an unchanged prefix after an edit.
                for (offset, spans) in lines.into_iter().enumerate() {
                    let line = first_line + offset;
                    self.preview.remove(&line);
                    if line < self.lines.len() {
                        self.lines[line] = spans;
                    } else {
                        self.lines.push(spans);
                    }
                }
                self.highlighter = Some(highlighter);
            }
            SyntaxChunk::Viewport { lines, parser } => {
                for (line, spans) in lines {
                    if line >= self.lines.len() {
                        self.preview.insert(line, spans);
                    }
                }
                self.preview_parser = parser;
                // Bound provisional storage around the most recent viewport.
                while self.preview.len() > PREVIEW_CACHE_LINES {
                    let first = *self.preview.first_key_value().unwrap().0;
                    let last = *self.preview.last_key_value().unwrap().0;
                    let furthest = if first.abs_diff(self.preview_anchor)
                        > last.abs_diff(self.preview_anchor)
                    {
                        first
                    } else {
                        last
                    };
                    self.preview.remove(&furthest);
                }
            }
        }
        true
    }

    pub(crate) fn stop_parsing(&mut self) {
        self.failed = true;
    }
}

const PARSE_BATCH_LINES: usize = 128;
const PARSE_BATCH_TIME: Duration = Duration::from_millis(4);
const PREVIEW_CACHE_LINES: usize = 1024;

#[derive(Debug)]
struct PreviewParser {
    next_line: usize,
    highlighter: highlighter::Highlighter,
}

enum SyntaxWork {
    Context(Option<highlighter::Highlighter>),
    Viewport {
        targets: Vec<usize>,
        parser: Option<PreviewParser>,
    },
}

pub(crate) struct SyntaxParseRequest {
    buffer: Arc<EditorBuffer>,
    settings: highlighter::Settings,
    generation: Arc<()>,
    work: SyntaxWork,
}

impl SyntaxParseRequest {
    /// Runs only on a blocking worker. A pathological single line may exceed
    /// the batch budget, but it cannot block input or rendering.
    pub(crate) fn parse(self) -> SyntaxParseResult {
        let started = Instant::now();
        let chunk = match self.work {
            SyntaxWork::Context(highlighter) => {
                let mut highlighter =
                    highlighter.unwrap_or_else(|| highlighter::Highlighter::new(&self.settings));
                let first_line = highlighter.current_line();
                let mut lines = Vec::new();
                for line in first_line..self.buffer.line_count() {
                    let text = self.buffer.line(line).unwrap_or_default();
                    lines.push(syntax_span_plan(&mut highlighter, &text));
                    if lines.len() >= PARSE_BATCH_LINES || started.elapsed() >= PARSE_BATCH_TIME {
                        break;
                    }
                }
                SyntaxChunk::Context {
                    first_line,
                    lines,
                    highlighter,
                }
            }
            SyntaxWork::Viewport {
                targets,
                mut parser,
            } => {
                let mut lines = Vec::new();
                for line in targets {
                    if !parser
                        .as_ref()
                        .is_some_and(|parser| parser.next_line == line)
                    {
                        // Starting mid-document is deliberately provisional:
                        // exact parsing later supplies comments/embedded-language
                        // context from preceding lines. Never store this as exact.
                        parser = Some(PreviewParser {
                            next_line: line,
                            highlighter: highlighter::Highlighter::new(&self.settings),
                        });
                    }
                    let parser = parser.as_mut().unwrap();
                    let text = self.buffer.line(line).unwrap_or_default();
                    lines.push((line, syntax_span_plan(&mut parser.highlighter, &text)));
                    parser.next_line = line + 1;
                    if lines.len() >= PARSE_BATCH_LINES || started.elapsed() >= PARSE_BATCH_TIME {
                        break;
                    }
                }
                SyntaxChunk::Viewport { lines, parser }
            }
        };
        SyntaxParseResult {
            settings: self.settings,
            generation: self.generation,
            chunk: Arc::new(Mutex::new(Some(chunk))),
        }
    }
}

enum SyntaxChunk {
    Context {
        first_line: usize,
        lines: Vec<Vec<SyntaxRenderSpan>>,
        highlighter: highlighter::Highlighter,
    },
    Viewport {
        lines: Vec<(usize, Vec<SyntaxRenderSpan>)>,
        parser: Option<PreviewParser>,
    },
}

/// Cloneable delivery envelope; parser ownership is transferred exactly once.
#[derive(Clone)]
pub struct SyntaxParseResult {
    settings: highlighter::Settings,
    generation: Arc<()>,
    chunk: Arc<Mutex<Option<SyntaxChunk>>>,
}

impl std::fmt::Debug for SyntaxParseResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyntaxParseResult")
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

fn syntax_span_plan(
    highlighter: &mut iced::highlighter::Highlighter,
    text: &str,
) -> Vec<SyntaxRenderSpan> {
    let mut spans = highlighter
        .highlight_line(text)
        .map(|(range, highlight)| SyntaxRenderSpan {
            range,
            color: highlight.color(),
        })
        .collect::<Vec<_>>();

    merge_adjacent_syntax_spans(&mut spans);

    spans
}

fn uses_syntax_highlighting(settings: &highlighter::Settings) -> bool {
    settings.token != DEFAULT_SYNTAX_TOKEN
}

fn merge_adjacent_syntax_spans(spans: &mut Vec<SyntaxRenderSpan>) {
    let mut merged: Vec<SyntaxRenderSpan> = Vec::with_capacity(spans.len());

    for span in spans.drain(..) {
        if let Some(previous) = merged.last_mut()
            && previous.range.end == span.range.start
            && previous.color == span.color
        {
            previous.range.end = span.range.end;
            continue;
        }

        merged.push(span);
    }

    *spans = merged;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> highlighter::Settings {
        highlighter::Settings {
            token: "html".into(),
            theme: highlighter::Theme::InspiredGitHub,
        }
    }

    fn finish(cache: &mut SyntaxLineCache, buffer: Arc<EditorBuffer>, viewport: &[usize]) {
        let mut batches = 0;
        while cache.needs_parse(buffer.line_count() - 1) {
            let result = cache.parse_request(buffer.clone(), viewport).parse();
            assert!(cache.apply_parsed(result));
            batches += 1;
            assert!(
                batches < buffer.line_count() * 4 + 10,
                "parser must make progress"
            );
        }
    }

    #[test]
    fn deep_viewport_gets_spans_before_document_prefix() {
        let buffer = Arc::new(EditorBuffer::from_text("<p>hello</p>\n".repeat(2000)));
        let mut cache = SyntaxLineCache::default();
        cache.configure(&settings());
        let request = cache.parse_request(buffer.clone(), &[1500, 1501, 1800]);
        assert!(cache.spans(1500).is_none(), "scheduling must not parse");
        assert!(
            cache.highlighter.is_none(),
            "parser initialization belongs on worker"
        );
        assert!(cache.apply_parsed(request.parse()));
        assert!(cache.spans(1500).is_some());
        assert_eq!(
            cache.cached_line_count(),
            0,
            "preview must not require parsing 1500 earlier lines"
        );
        // A changed viewport takes priority over unfinished previous work.
        let request = cache.parse_request(buffer, &[1900, 1901]);
        assert!(cache.apply_parsed(request.parse()));
        assert!(cache.spans(1900).is_some());
        assert_eq!(cache.cached_line_count(), 0);
    }

    #[test]
    fn exact_context_refines_provisional_multiline_html_and_embedded_languages() {
        let source = format!(
            "<!--\n{}-->\n<script>\n/*\n{}*/\nconst answer = 42;\n</script>\n<style>\np {{ color: red; }}\n</style>",
            "<a href=\"test\">inside comment</a>\n".repeat(300),
            "let value = 'inside JS comment';\n".repeat(150)
        );
        let buffer = Arc::new(EditorBuffer::from_text(source));
        let expected = SyntaxLineCache::rebuild(&buffer, &settings());
        let mut cache = SyntaxLineCache::default();
        cache.configure(&settings());
        let request = cache.parse_request(buffer.clone(), &[200, 201, 400]);
        cache.apply_parsed(request.parse());
        assert_ne!(
            cache.spans(200),
            expected.spans(200),
            "mid-comment preview starts without earlier context"
        );
        finish(&mut cache, buffer, &[200, 201, 400]);
        assert_eq!(cache, expected);
        assert!(
            cache.preview.is_empty(),
            "exact spans replace provisional entries"
        );
    }

    #[test]
    fn context_batches_are_bounded_and_not_starved_by_new_viewports() {
        let buffer = Arc::new(EditorBuffer::from_text("<p>x</p>\n".repeat(4000)));
        let mut cache = SyntaxLineCache::default();
        cache.configure(&settings());
        for line in [2000, 2500, 3000] {
            let request = cache.parse_request(buffer.clone(), &[line]);
            cache.apply_parsed(request.parse());
        }
        assert!((1..=PARSE_BATCH_LINES).contains(&cache.cached_line_count()));
    }

    #[test]
    fn edits_reject_inflight_spans_and_reparse_with_correct_context() {
        let mut buffer =
            EditorBuffer::from_text(format!("<!--\n{}-->\n", "<p>x</p>\n".repeat(180)));
        let mut cache = SyntaxLineCache::default();
        cache.configure(&settings());
        let old = cache
            .parse_request(Arc::new(buffer.clone()), &[120])
            .parse();
        buffer.replace_range(
            crate::editor::EditorRange::new(
                crate::editor::EditorPosition::new(0, 0),
                crate::editor::EditorPosition::new(0, 4),
            ),
            "<div>",
        );
        cache.invalidate_from(0);
        assert!(!cache.apply_parsed(old));
        assert!(cache.spans(120).is_none());
        finish(&mut cache, Arc::new(buffer.clone()), &[120]);
        assert_eq!(cache, SyntaxLineCache::rebuild(&buffer, &settings()));
    }

    #[test]
    fn changed_settings_and_cloned_documents_reject_old_results() {
        let buffer = Arc::new(EditorBuffer::from_text("<p>hello</p>\n".repeat(200)));
        for new_settings in [
            highlighter::Settings {
                token: "js".into(),
                ..settings()
            },
            highlighter::Settings {
                theme: highlighter::Theme::SolarizedDark,
                ..settings()
            },
        ] {
            let mut cache = SyntaxLineCache::default();
            cache.configure(&settings());
            let result = cache.parse_request(buffer.clone(), &[150]).parse();
            assert!(!cache.clone().apply_parsed(result.clone()));
            cache.configure(&new_settings);
            assert!(!cache.apply_parsed(result));
            finish(&mut cache, buffer.clone(), &[150]);
            assert_eq!(cache, SyntaxLineCache::rebuild(&buffer, &new_settings));
        }
    }

    #[test]
    fn cloned_prefix_replays_parser_state_before_extending_it() {
        let buffer = Arc::new(EditorBuffer::from_text(format!(
            "<!--\n{}-->\n",
            "<p>x</p>\n".repeat(180)
        )));
        let mut cache = SyntaxLineCache::new(&settings());
        cache.ensure_visible(&buffer, &settings(), 0, 100);
        let mut clone = cache.clone();
        finish(&mut clone, buffer.clone(), &[160]);
        assert_eq!(clone, SyntaxLineCache::rebuild(&buffer, &settings()));
    }
}
