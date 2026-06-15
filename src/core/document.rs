use iced::highlighter;
use iced::widget::text_editor::LineEnding;

use crate::core::encoding::{
    DecodedText, TextEncoding, encode_text, encode_utf8_chunks_for_save, strip_text_bom,
};
use crate::editor::{
    DecorationModel, DecorationSettings, EditorBuffer, EditorHistory, EditorPosition,
    EditorSelection, FoldModel, FoldProvider, IndentBraceFoldProvider, IndentGuide, ScrollOffset,
    SelectionSet, SyntaxLineCache, ViewportModel,
};

use std::cell::RefCell;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const DEFAULT_SYNTAX_TOKEN: &str = "txt";
const DEFAULT_REVEAL_CONTEXT_ROWS: usize = 3;
static NEXT_LOAD_GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyntaxTokenSource {
    Auto,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocumentId(u64);

impl DocumentId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for DocumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocumentLoadGeneration(u64);

impl DocumentLoadGeneration {
    pub fn next() -> Self {
        Self(NEXT_LOAD_GENERATION.fetch_add(1, Ordering::Relaxed))
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for DocumentLoadGeneration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentLoadState {
    Complete,
    Loading {
        generation: DocumentLoadGeneration,
        bytes_read: u64,
        total_bytes: Option<u64>,
    },
    Failed {
        generation: DocumentLoadGeneration,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentIndexState {
    Complete,
    Pending { generation: DocumentLoadGeneration },
}

#[derive(Debug, Clone)]
pub struct Document {
    pub id: DocumentId,
    pub path: Option<PathBuf>,
    pub buffer: EditorBuffer,
    pub selection: EditorSelection,
    selection_set: SelectionSet,
    pub preferred_vertical_column: Option<usize>,
    pub history: EditorHistory,
    pub folds: FoldModel,
    pub viewport: ViewportModel,
    pub decorations: DecorationModel,
    pub scroll: ScrollOffset,
    pub is_dirty: bool,
    pub is_pinned: bool,
    pub syntax_token: String,
    pub syntax_cache: RefCell<SyntaxLineCache>,
    pub line_ending: Option<LineEnding>,
    pub encoding: TextEncoding,
    pub load_state: DocumentLoadState,
    pub index_state: DocumentIndexState,
    revision: u64,
    metadata_dirty: bool,
    syntax_token_source: SyntaxTokenSource,
}

impl Document {
    pub fn untitled(id: DocumentId) -> Self {
        Self::from_parts(
            id,
            None,
            String::new(),
            DEFAULT_SYNTAX_TOKEN.to_owned(),
            Some(LineEnding::Lf),
            SyntaxTokenSource::Auto,
        )
    }

    pub fn from_path(id: DocumentId, path: impl Into<PathBuf>, text: &str) -> Self {
        let path = path.into();
        let text = strip_text_bom(text);
        let line_ending = detect_line_ending(text);

        Self::from_parts(
            id,
            Some(path.clone()),
            text.to_owned(),
            syntax_token_for_path(&path),
            line_ending,
            SyntaxTokenSource::Auto,
        )
    }

    pub fn from_decoded(id: DocumentId, path: impl Into<PathBuf>, decoded: DecodedText) -> Self {
        let path = path.into();
        let text = strip_text_bom(&decoded.text);
        let line_ending = detect_line_ending(text);
        let mut document = Self::from_parts(
            id,
            Some(path.clone()),
            text.to_owned(),
            syntax_token_for_path(&path),
            line_ending,
            SyntaxTokenSource::Auto,
        );
        document.encoding = decoded.encoding;
        document
    }

    pub fn loading(
        id: DocumentId,
        path: impl Into<PathBuf>,
        generation: DocumentLoadGeneration,
    ) -> Self {
        let path = path.into();
        let mut document = Self::from_parts(
            id,
            Some(path.clone()),
            String::new(),
            syntax_token_for_path(&path),
            None,
            SyntaxTokenSource::Auto,
        );
        document.load_state = DocumentLoadState::Loading {
            generation,
            bytes_read: 0,
            total_bytes: None,
        };
        document.index_state = DocumentIndexState::Pending { generation };
        document
    }

    fn from_parts(
        id: DocumentId,
        path: Option<PathBuf>,
        text: String,
        syntax_token: String,
        line_ending: Option<LineEnding>,
        syntax_token_source: SyntaxTokenSource,
    ) -> Self {
        let buffer = EditorBuffer::from_text(text);
        let selection = EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(0, 0));
        let selection_set = SelectionSet::single(selection);
        let history = EditorHistory::new("");
        let decoration_settings = DecorationSettings::default();
        let folds = FoldModel::new(
            fold_provider(decoration_settings, &syntax_token).compute_folds(&buffer),
        );
        let viewport = ViewportModel::new(buffer.line_count(), &folds);
        let decorations = DecorationModel::from_folds(
            decoration_settings,
            buffer.line_count(),
            &folds,
            indent_guides(&buffer, decoration_settings.indent_width),
        );

        Self {
            id,
            path,
            buffer,
            selection,
            selection_set,
            preferred_vertical_column: None,
            history,
            folds,
            viewport,
            decorations,
            scroll: ScrollOffset::ZERO,
            is_dirty: false,
            is_pinned: false,
            syntax_token,
            syntax_cache: RefCell::new(SyntaxLineCache::default()),
            line_ending,
            encoding: TextEncoding::Utf8,
            load_state: DocumentLoadState::Complete,
            index_state: DocumentIndexState::Complete,
            revision: 0,
            metadata_dirty: false,
            syntax_token_source,
        }
    }

    pub fn title(&self) -> String {
        self.path
            .as_deref()
            .and_then(title_for_path)
            .map(str::to_owned)
            .unwrap_or_else(|| format!("Untitled {}", self.id))
    }

    pub fn set_path(&mut self, path: impl Into<PathBuf>) {
        let path = path.into();
        if self.syntax_token_source == SyntaxTokenSource::Auto {
            let syntax_token = syntax_token_for_path(&path);
            if self.syntax_token != syntax_token {
                self.syntax_token = syntax_token;
                self.refresh_after_syntax_change();
            }
        }
        self.path = Some(path);
    }

    pub fn mark_dirty(&mut self) {
        self.metadata_dirty = true;
        self.is_dirty = true;
    }

    pub fn mark_clean(&mut self) {
        self.history.mark_clean("");
        self.metadata_dirty = false;
        self.is_dirty = false;
    }

    pub fn is_loading(&self) -> bool {
        matches!(self.load_state, DocumentLoadState::Loading { .. })
    }

    pub fn is_indexing(&self) -> bool {
        matches!(self.index_state, DocumentIndexState::Pending { .. })
    }

    pub fn is_loading_or_indexing(&self) -> bool {
        self.is_loading() || self.is_indexing()
    }

    pub fn has_complete_text_index(&self) -> bool {
        matches!(self.load_state, DocumentLoadState::Complete)
            && matches!(self.index_state, DocumentIndexState::Complete)
    }

    pub fn can_run_full_document_analysis(&self) -> bool {
        self.has_complete_text_index()
    }

    pub fn load_generation(&self) -> Option<DocumentLoadGeneration> {
        match self.load_state {
            DocumentLoadState::Loading { generation, .. }
            | DocumentLoadState::Failed { generation } => Some(generation),
            DocumentLoadState::Complete => match self.index_state {
                DocumentIndexState::Pending { generation } => Some(generation),
                DocumentIndexState::Complete => None,
            },
        }
    }

    pub fn accepts_load_generation(&self, generation: DocumentLoadGeneration) -> bool {
        self.load_generation() == Some(generation)
    }

    pub fn update_load_progress(
        &mut self,
        generation: DocumentLoadGeneration,
        bytes_read: u64,
        total_bytes: Option<u64>,
    ) -> bool {
        let DocumentLoadState::Loading {
            generation: current,
            bytes_read: current_bytes,
            total_bytes: current_total,
        } = &mut self.load_state
        else {
            return false;
        };

        if *current != generation {
            return false;
        }

        *current_bytes = bytes_read;
        *current_total = total_bytes;
        true
    }

    pub fn has_active_load(&self, generation: DocumentLoadGeneration) -> bool {
        matches!(
            self.load_state,
            DocumentLoadState::Loading {
                generation: current,
                ..
            } if current == generation
        )
    }

    pub fn replace_loading_preview(
        &mut self,
        generation: DocumentLoadGeneration,
        text: &str,
        bytes_read: u64,
        total_bytes: Option<u64>,
    ) -> bool {
        if !self.update_load_progress(generation, bytes_read, total_bytes) {
            return false;
        }

        self.buffer.append_text(text);
        self.selection = EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(0, 0));
        self.selection_set = SelectionSet::single(self.selection);
        self.refresh_view_models();
        self.syntax_cache.borrow_mut().clear();
        self.revision = self.revision.saturating_add(1);
        true
    }

    pub fn complete_loading(
        &mut self,
        generation: DocumentLoadGeneration,
        decoded: DecodedText,
    ) -> bool {
        if !self.has_active_load(generation) {
            return false;
        }

        let text = strip_text_bom(&decoded.text);
        self.buffer = EditorBuffer::from_text(text.to_owned());
        self.selection = EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(0, 0));
        self.selection_set = SelectionSet::single(self.selection);
        self.history = EditorHistory::new("");
        self.metadata_dirty = false;
        self.is_dirty = false;
        self.line_ending = detect_line_ending(text);
        self.encoding = decoded.encoding;
        self.load_state = DocumentLoadState::Complete;
        self.index_state = DocumentIndexState::Complete;
        self.refresh_after_text_change();
        true
    }

    pub fn complete_streaming_load(
        &mut self,
        generation: DocumentLoadGeneration,
        encoding: TextEncoding,
    ) -> bool {
        if !self.has_active_load(generation) {
            return false;
        }

        self.history = EditorHistory::new("");
        self.metadata_dirty = false;
        self.is_dirty = false;
        self.line_ending = self.detect_current_line_ending();
        self.encoding = encoding;
        self.load_state = DocumentLoadState::Complete;
        self.index_state = DocumentIndexState::Complete;
        self.refresh_after_text_change();
        true
    }

    pub fn fail_loading(&mut self, generation: DocumentLoadGeneration) -> bool {
        if !self.has_active_load(generation) {
            return false;
        }

        self.load_state = DocumentLoadState::Failed { generation };
        self.index_state = DocumentIndexState::Complete;
        true
    }

    pub fn text(&self) -> String {
        self.buffer.text()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn text_for_save(&self) -> String {
        let mut text = self.buffer.text();
        let Some(line_ending) = self.line_ending else {
            return text;
        };

        let ending = line_ending.as_str();

        if !text.is_empty() && !text.ends_with(ending) {
            text.push_str(ending);
        }

        text
    }

    pub fn save_appended_line_ending(&self) -> Option<&'static str> {
        let line_ending = self.line_ending?;
        let ending = line_ending.as_str();
        let mut last = None;
        for chunk in self.buffer.chunks() {
            last = Some(chunk);
        }
        let last = last?;

        (!last.is_empty() && !last.ends_with(ending)).then_some(ending)
    }

    pub fn bytes_for_save(&self) -> Result<Vec<u8>, crate::core::encoding::EncodingError> {
        if matches!(self.encoding, TextEncoding::Utf8 | TextEncoding::Utf8Bom) {
            return Ok(encode_utf8_chunks_for_save(
                self.buffer.chunks(),
                self.save_appended_line_ending(),
                self.encoding == TextEncoding::Utf8Bom,
            ));
        }

        // Non-UTF encoders operate on scalar values and may report unmappable
        // characters, so they still materialize the compatibility string.
        encode_text(&self.text_for_save(), self.encoding)
    }

    pub fn set_encoding(&mut self, encoding: TextEncoding) {
        if self.encoding != encoding {
            self.encoding = encoding;
            self.mark_dirty();
        }
    }

    pub fn refresh_syntax_from_path(&mut self) {
        self.syntax_token_source = SyntaxTokenSource::Auto;
        let syntax_token = self
            .path
            .as_deref()
            .map(syntax_token_for_path)
            .unwrap_or_else(|| DEFAULT_SYNTAX_TOKEN.to_owned());
        if self.syntax_token != syntax_token {
            self.syntax_token = syntax_token;
            self.refresh_after_syntax_change();
        }
    }

    pub fn set_syntax_token(&mut self, syntax_token: impl Into<String>) {
        let syntax_token = syntax_token.into();

        let syntax_token = if syntax_token.is_empty() {
            DEFAULT_SYNTAX_TOKEN.to_owned()
        } else {
            syntax_token
        };
        let syntax_token_source = if syntax_token == DEFAULT_SYNTAX_TOKEN {
            SyntaxTokenSource::Auto
        } else {
            SyntaxTokenSource::Manual
        };

        if self.syntax_token != syntax_token || self.syntax_token_source != syntax_token_source {
            self.syntax_token = syntax_token;
            self.syntax_token_source = syntax_token_source;
            self.refresh_after_syntax_change();
        }
    }

    pub fn uses_syntax_highlighting(&self) -> bool {
        self.syntax_token != DEFAULT_SYNTAX_TOKEN
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn undo(&mut self) -> bool {
        let Some(selection_set) = self.history.undo_selection_set(&mut self.buffer) else {
            return false;
        };

        self.set_selection_set(selection_set);
        self.clamp_selection_set();
        self.refresh_after_text_change();
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(selection_set) = self.history.redo_selection_set(&mut self.buffer) else {
            return false;
        };

        self.set_selection_set(selection_set);
        self.clamp_selection_set();
        self.refresh_after_text_change();
        true
    }

    pub fn refresh_after_text_change(&mut self) {
        self.refresh_text_from(0);
    }

    pub fn refresh_text_from(&mut self, first_changed_line: usize) {
        if self.can_run_full_document_analysis() {
            self.folds
                .recompute(self.fold_provider().compute_folds(&self.buffer));
        }
        self.refresh_view_models();
        self.refresh_dirty_state();
        self.syntax_cache
            .borrow_mut()
            .invalidate_from(first_changed_line);
        self.revision = self.revision.saturating_add(1);
    }

    pub fn refresh_view_models(&mut self) {
        self.viewport = ViewportModel::new(self.buffer.line_count(), &self.folds);
        self.decorations = DecorationModel::from_folds(
            self.decorations.settings,
            self.buffer.line_count(),
            &self.folds,
            indent_guides(&self.buffer, self.decorations.settings.indent_width),
        );
        self.scroll.first_visible_row = self
            .scroll
            .first_visible_row
            .min(self.viewport.visible_row_count().saturating_sub(1));
    }

    pub fn reveal_line(&mut self, line: usize) {
        self.reveal_line_with_context(line, DEFAULT_REVEAL_CONTEXT_ROWS);
    }

    pub fn reveal_line_with_context(&mut self, line: usize, context_rows: usize) {
        let Some(visible_row) = self.viewport.document_line_to_visible_row(line) else {
            return;
        };

        let max = self.viewport.visible_row_count().saturating_sub(1);
        self.scroll.first_visible_row = visible_row.saturating_sub(context_rows).min(max);
    }

    pub fn set_decoration_settings(&mut self, settings: DecorationSettings) {
        self.decorations.settings = settings;
        if self.can_run_full_document_analysis() {
            self.folds
                .recompute(fold_provider(settings, &self.syntax_token).compute_folds(&self.buffer));
        }
        self.refresh_view_models();
    }

    pub fn ensure_syntax_cache(&self, theme: highlighter::Theme) {
        let settings = highlighter::Settings {
            token: self.syntax_token.clone(),
            theme,
        };

        if !self
            .syntax_cache
            .borrow()
            .is_current(&settings, self.buffer.line_count())
        {
            *self.syntax_cache.borrow_mut() = SyntaxLineCache::rebuild(&self.buffer, &settings);
        }
    }

    pub fn ensure_visible_syntax_cache(
        &self,
        theme: highlighter::Theme,
        first_line: usize,
        last_line: usize,
    ) {
        let settings = highlighter::Settings {
            token: self.syntax_token.clone(),
            theme,
        };

        self.syntax_cache.borrow_mut().ensure_visible(
            &self.buffer,
            &settings,
            first_line,
            last_line,
        );
    }

    pub fn clamp_selection(&self, selection: EditorSelection) -> EditorSelection {
        EditorSelection::new(
            self.buffer.clamp_position(selection.anchor),
            self.buffer.clamp_position(selection.cursor),
        )
    }

    pub fn main_selection(&self) -> EditorSelection {
        self.selection_set.main()
    }

    pub fn set_main_selection(&mut self, selection: EditorSelection) {
        self.selection = self.clamp_selection(selection);
        self.selection_set = SelectionSet::single(self.selection);
    }

    pub fn selection_set(&self) -> &SelectionSet {
        &self.selection_set
    }

    pub fn sync_selection_mirror(&mut self) {
        let selection = self.clamp_selection(self.selection);
        if selection != self.selection_set.main() {
            self.selection = selection;
            self.selection_set = SelectionSet::single(selection);
        }
    }

    pub fn set_selection_set(&mut self, selection_set: SelectionSet) {
        self.selection_set = selection_set.clamped(&self.buffer);
        self.selection = self.selection_set.main();
    }

    pub fn clamp_selection_set(&mut self) {
        self.selection_set = self.selection_set.clamped(&self.buffer);
        self.selection = self.selection_set.main();
    }

    fn refresh_dirty_state(&mut self) {
        self.is_dirty = self.metadata_dirty || self.history.is_dirty("");
    }

    fn refresh_after_syntax_change(&mut self) {
        self.syntax_cache.borrow_mut().clear();
        if self.can_run_full_document_analysis() {
            self.folds
                .recompute(self.fold_provider().compute_folds(&self.buffer));
        }
        self.refresh_view_models();
        self.revision = self.revision.saturating_add(1);
    }

    fn detect_current_line_ending(&self) -> Option<LineEnding> {
        let mut previous_was_cr = false;

        for chunk in self.buffer.chunks() {
            if chunk.is_empty() {
                continue;
            }

            if previous_was_cr {
                return Some(if chunk.as_bytes().first() == Some(&b'\n') {
                    LineEnding::CrLf
                } else {
                    LineEnding::Cr
                });
            }

            let bytes = chunk.as_bytes();
            let mut index = 0;
            while index < bytes.len() {
                match bytes[index] {
                    b'\r' => {
                        if index + 1 == bytes.len() {
                            previous_was_cr = true;
                            break;
                        }
                        return Some(if bytes.get(index + 1) == Some(&b'\n') {
                            LineEnding::CrLf
                        } else {
                            LineEnding::Cr
                        });
                    }
                    b'\n' => {
                        return Some(if bytes.get(index + 1) == Some(&b'\r') {
                            LineEnding::LfCr
                        } else {
                            LineEnding::Lf
                        });
                    }
                    _ => index += 1,
                }
            }
        }

        if previous_was_cr {
            return Some(LineEnding::Cr);
        }

        None
    }

    fn fold_provider(&self) -> IndentBraceFoldProvider {
        fold_provider(self.decorations.settings, &self.syntax_token)
    }
}

pub fn title_for_path(path: &Path) -> Option<&str> {
    path.file_name().and_then(|name| name.to_str())
}

pub fn syntax_token_for_path(path: &Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| !extension.is_empty())
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| DEFAULT_SYNTAX_TOKEN.to_owned())
}

pub fn detect_line_ending(text: &str) -> Option<LineEnding> {
    let bytes = text.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'\r' => {
                return Some(if bytes.get(index + 1) == Some(&b'\n') {
                    LineEnding::CrLf
                } else {
                    LineEnding::Cr
                });
            }
            b'\n' => {
                return Some(if bytes.get(index + 1) == Some(&b'\r') {
                    LineEnding::LfCr
                } else {
                    LineEnding::Lf
                });
            }
            _ => index += 1,
        }
    }

    None
}

fn indent_guides(buffer: &EditorBuffer, indent_width: usize) -> Vec<IndentGuide> {
    let mut guides = Vec::new();
    let indent_width = indent_width.max(1);

    for line in 0..buffer.line_count() {
        let Some(text) = buffer.line(line) else {
            continue;
        };

        let columns = text
            .chars()
            .take_while(|ch| *ch == ' ' || *ch == '\t')
            .map(|ch| if ch == '\t' { indent_width } else { 1 })
            .sum::<usize>();

        for depth in 1..columns / indent_width {
            guides.push(IndentGuide { line, depth });
        }
    }

    guides
}

fn fold_provider(settings: DecorationSettings, syntax_token: &str) -> IndentBraceFoldProvider {
    IndentBraceFoldProvider::for_syntax(settings.indent_width, syntax_token)
}
