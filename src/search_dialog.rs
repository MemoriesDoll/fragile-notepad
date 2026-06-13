use crate::core::{
    Document, DocumentId, PreparedSearch, SearchMode, SearchOptions, TextMatch, Workspace,
};
use crate::editor::{EditorSelection, position_for_byte_offset};
use crate::message::AdvancedSearchTab;

#[derive(Debug, Clone)]
pub struct SearchDialogState {
    pub active_tab: AdvancedSearchTab,
    pub query: String,
    pub replacement: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub wrap_around: bool,
    pub mode: SearchMode,
    pub include_pattern: String,
    pub results: Vec<SearchResult>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub document_id: DocumentId,
    pub document_title: String,
    pub selection: EditorSelection,
    pub preview: String,
}

impl SearchDialogState {
    pub fn new() -> Self {
        Self {
            active_tab: AdvancedSearchTab::Find,
            query: String::new(),
            replacement: String::new(),
            case_sensitive: false,
            whole_word: false,
            wrap_around: true,
            mode: SearchMode::Normal,
            include_pattern: String::new(),
            results: Vec::new(),
            status: String::from("No query"),
        }
    }

    pub fn options(&self) -> SearchOptions {
        SearchOptions {
            case_sensitive: self.case_sensitive,
            whole_word: self.whole_word,
            mode: self.mode,
        }
    }

    pub fn set_active_tab(&mut self, tab: AdvancedSearchTab) {
        self.active_tab = tab;
    }

    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.results.clear();
        self.status = if self.query.is_empty() {
            String::from("No query")
        } else {
            String::from("Ready")
        };
    }

    pub fn set_replacement(&mut self, replacement: impl Into<String>) {
        self.replacement = replacement.into();
    }

    pub fn set_case_sensitive(&mut self, case_sensitive: bool) {
        self.case_sensitive = case_sensitive;
        self.results.clear();
    }

    pub fn set_whole_word(&mut self, whole_word: bool) {
        self.whole_word = whole_word;
        self.results.clear();
    }

    pub fn set_wrap_around(&mut self, wrap_around: bool) {
        self.wrap_around = wrap_around;
    }

    pub fn set_mode(&mut self, mode: SearchMode) {
        self.mode = mode;
        self.results.clear();
    }

    pub fn set_include_pattern(&mut self, include_pattern: impl Into<String>) {
        self.include_pattern = include_pattern.into();
        self.results.clear();
    }

    pub fn refresh_from_workspace(&mut self, workspace: &Workspace) {
        let include_pattern = self.include_pattern.clone();

        self.refresh_from_documents(
            workspace
                .documents()
                .iter()
                .filter(|document| include_filter_matches(document, &include_pattern)),
        );
    }

    pub fn refresh_from_documents<'a>(
        &mut self,
        documents: impl IntoIterator<Item = &'a Document>,
    ) {
        if self.query.is_empty() {
            self.results.clear();
            self.status = String::from("No query");
            return;
        }

        match document_results(documents, &self.query, self.options()) {
            Ok(results) => {
                self.results = results;
                self.status = match self.results.len() {
                    0 => String::from("No matches"),
                    1 => String::from("1 match"),
                    count => format!("{count} matches"),
                };
            }
            Err(error) => {
                self.results.clear();
                self.status = search_error_status(error);
            }
        };
    }
}

impl Default for SearchDialogState {
    fn default() -> Self {
        Self::new()
    }
}

fn document_results<'a>(
    documents: impl IntoIterator<Item = &'a Document>,
    query: &str,
    options: SearchOptions,
) -> Result<Vec<SearchResult>, crate::core::SearchError> {
    let Some(search) = PreparedSearch::new(query, options)? else {
        return Ok(Vec::new());
    };
    let mut results = Vec::new();

    for document in documents {
        for text_match in search.matches(document.buffer.text()) {
            if let Some(result) = result_for_match(
                document.id,
                document.title(),
                document.buffer.text(),
                text_match,
            ) {
                results.push(result);
            }
        }
    }

    Ok(results)
}

pub fn search_error_status(error: crate::core::SearchError) -> String {
    match error {
        crate::core::SearchError::InvalidRegex(error) => format!("Invalid regex: {error}"),
    }
}

pub fn include_filter_matches(document: &Document, include_pattern: &str) -> bool {
    let pattern = include_pattern.trim();

    if pattern.is_empty() || pattern == "*" || pattern == "*.*" {
        return true;
    }

    pattern
        .split([';', ',', ' '])
        .filter(|part| !part.trim().is_empty())
        .any(|part| matches_one_pattern(document, part.trim()))
}

fn matches_one_pattern(document: &Document, pattern: &str) -> bool {
    let title = document.title();
    let path = document
        .path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| title.clone());
    let pattern = pattern.to_ascii_lowercase();

    wildcard_match(&title.to_ascii_lowercase(), &pattern)
        || wildcard_match(&path.to_ascii_lowercase(), &pattern)
}

fn wildcard_match(value: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return value.is_empty();
    }

    let value = value.as_bytes();
    let pattern = pattern.as_bytes();
    let (mut value_index, mut pattern_index) = (0usize, 0usize);
    let mut star_pattern = None;
    let mut star_value = 0usize;

    while value_index < value.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == b'?' || pattern[pattern_index] == value[value_index])
        {
            value_index += 1;
            pattern_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            star_pattern = Some(pattern_index);
            pattern_index += 1;
            star_value = value_index;
        } else if let Some(star) = star_pattern {
            pattern_index = star + 1;
            star_value += 1;
            value_index = star_value;
        } else {
            return false;
        }
    }

    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index += 1;
    }

    pattern_index == pattern.len()
}

fn result_for_match(
    document_id: DocumentId,
    document_title: String,
    text: &str,
    text_match: TextMatch,
) -> Option<SearchResult> {
    let start = position_for_byte_offset(text, text_match.start)?;
    let end = position_for_byte_offset(text, text_match.end)?;
    let line_start = text[..text_match.start]
        .rfind(['\r', '\n'])
        .map(|index| index + 1)
        .unwrap_or(0);
    let line_end = text[text_match.end..]
        .find(['\r', '\n'])
        .map(|index| text_match.end + index)
        .unwrap_or(text.len());
    let preview = text[line_start..line_end].trim().to_owned();

    Some(SearchResult {
        document_id,
        document_title,
        selection: EditorSelection::new(start, end),
        preview,
    })
}

#[cfg(test)]
mod tests {
    use super::{SearchDialogState, include_filter_matches};
    use crate::core::{Document, DocumentId, SearchMode};
    use crate::editor::{EditorPosition, EditorSelection};

    #[test]
    fn include_filter_accepts_empty_and_star_patterns() {
        let document = Document::from_path(DocumentId::new(1), "main.rs", "");

        assert!(include_filter_matches(&document, ""));
        assert!(include_filter_matches(&document, "*"));
        assert!(include_filter_matches(&document, "*.*"));
    }

    #[test]
    fn include_filter_matches_document_title_and_path_with_wildcards() {
        let document = Document::from_path(DocumentId::new(2), "src/app/search.rs", "");

        assert!(include_filter_matches(&document, "*.rs"));
        assert!(include_filter_matches(&document, "src*search.rs"));
        assert!(include_filter_matches(&document, "*.txt;*.rs"));
        assert!(!include_filter_matches(&document, "*.md;*.txt"));
    }

    #[test]
    fn search_results_preserve_full_match_selection() {
        let document = Document::from_path(DocumentId::new(3), "notes.txt", "alpha beta gamma");
        let mut dialog = SearchDialogState::new();

        dialog.set_query("beta");
        dialog.refresh_from_documents([&document]);

        assert_eq!(dialog.results.len(), 1);
        assert_eq!(
            dialog.results[0].selection,
            EditorSelection::new(EditorPosition::new(0, 6), EditorPosition::new(0, 10))
        );
    }

    #[test]
    fn extended_search_mode_matches_escape_sequences() {
        let document = Document::from_path(DocumentId::new(4), "notes.txt", "alpha\nbeta");
        let mut dialog = SearchDialogState::new();

        dialog.set_query(r"alpha\nbeta");
        dialog.set_mode(SearchMode::Extended);
        dialog.refresh_from_documents([&document]);

        assert_eq!(dialog.results.len(), 1);
    }

    #[test]
    fn regex_search_mode_reports_invalid_patterns() {
        let document = Document::from_path(DocumentId::new(5), "notes.txt", "alpha");
        let mut dialog = SearchDialogState::new();

        dialog.set_query("[");
        dialog.set_mode(SearchMode::Regex);
        dialog.refresh_from_documents([&document]);

        assert!(dialog.results.is_empty());
        assert!(dialog.status.starts_with("Invalid regex:"));
    }
}
