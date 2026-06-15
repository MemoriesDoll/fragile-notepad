use std::collections::VecDeque;
use std::ops::Range;

use regex::{Regex, RegexBuilder};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextMatch {
    pub start: usize,
    pub end: usize,
}

impl TextMatch {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn range(self) -> Range<usize> {
        self.start..self.end
    }

    pub const fn len(self) -> usize {
        if self.end >= self.start {
            self.end - self.start
        } else {
            0
        }
    }

    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FindState {
    pub query: String,
    pub replacement: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub matches: Vec<TextMatch>,
    pub current_match: Option<usize>,
}

impl FindState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_query(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            ..Self::default()
        }
    }

    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.current_match = None;
        self.matches.clear();
    }

    pub fn set_replacement(&mut self, replacement: impl Into<String>) {
        self.replacement = replacement.into();
    }

    pub fn set_case_sensitive(&mut self, case_sensitive: bool) {
        if self.case_sensitive != case_sensitive {
            self.case_sensitive = case_sensitive;
            self.current_match = None;
            self.matches.clear();
        }
    }

    pub fn set_whole_word(&mut self, whole_word: bool) {
        if self.whole_word != whole_word {
            self.whole_word = whole_word;
            self.current_match = None;
            self.matches.clear();
        }
    }

    pub fn refresh_matches(&mut self, text: &str) {
        if self.query.is_empty() {
            self.matches.clear();
            self.current_match = None;
            return;
        }

        self.matches = compute_matches_with_options(
            text,
            &self.query,
            SearchOptions::normal(self.case_sensitive, self.whole_word),
        );
        self.current_match = match (self.current_match, self.matches.is_empty()) {
            (_, true) => None,
            (Some(index), false) => Some(index.min(self.matches.len() - 1)),
            (None, false) => Some(0),
        };
    }

    pub fn refresh_matches_in_chunks<'a>(&mut self, chunks: impl IntoIterator<Item = &'a str>) {
        if self.query.is_empty() {
            self.matches.clear();
            self.current_match = None;
            return;
        }

        self.matches = compute_matches_in_chunks(
            chunks,
            &self.query,
            SearchOptions::normal(self.case_sensitive, self.whole_word),
        );
        self.current_match = match (self.current_match, self.matches.is_empty()) {
            (_, true) => None,
            (Some(index), false) => Some(index.min(self.matches.len() - 1)),
            (None, false) => Some(0),
        };
    }

    pub fn current(&self) -> Option<TextMatch> {
        self.current_match
            .and_then(|index| self.matches.get(index).copied())
    }

    pub fn next(&mut self) -> Option<TextMatch> {
        if self.matches.is_empty() {
            self.current_match = None;
            return None;
        }

        let next = self
            .current_match
            .map(|index| (index + 1) % self.matches.len())
            .unwrap_or(0);

        self.current_match = Some(next);
        self.current()
    }

    pub fn previous(&mut self) -> Option<TextMatch> {
        if self.matches.is_empty() {
            self.current_match = None;
            return None;
        }

        let previous = self
            .current_match
            .map(|index| {
                if index == 0 {
                    self.matches.len() - 1
                } else {
                    index - 1
                }
            })
            .unwrap_or(0);

        self.current_match = Some(previous);
        self.current()
    }

    pub fn replace_current(&mut self, text: &str) -> Option<String> {
        let current = self.current()?;
        let mut replaced = String::with_capacity(text.len() + self.replacement.len());

        replaced.push_str(&text[..current.start]);
        replaced.push_str(&self.replacement);
        replaced.push_str(&text[current.end..]);

        self.refresh_matches(&replaced);

        Some(replaced)
    }

    pub fn replace_all(&mut self, text: &str) -> (String, usize) {
        let matches = compute_matches_with_options(
            text,
            &self.query,
            SearchOptions::normal(self.case_sensitive, self.whole_word),
        );

        if matches.is_empty() {
            self.matches.clear();
            self.current_match = None;
            return (text.to_owned(), 0);
        }

        let mut replaced = String::with_capacity(text.len());
        let mut cursor = 0;

        for text_match in &matches {
            replaced.push_str(&text[cursor..text_match.start]);
            replaced.push_str(&self.replacement);
            cursor = text_match.end;
        }

        replaced.push_str(&text[cursor..]);

        let count = matches.len();
        self.refresh_matches(&replaced);

        (replaced, count)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SearchMode {
    #[default]
    Normal,
    Extended,
    Regex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchError {
    InvalidRegex(String),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub mode: SearchMode,
}

impl SearchOptions {
    pub const fn normal(case_sensitive: bool, whole_word: bool) -> Self {
        Self {
            case_sensitive,
            whole_word,
            mode: SearchMode::Normal,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreparedSearch {
    matcher: PreparedMatcher,
    options: SearchOptions,
}

#[derive(Debug, Clone)]
enum PreparedMatcher {
    Literal(String),
    Regex(Regex),
}

impl PreparedSearch {
    pub fn new(query: &str, options: SearchOptions) -> Result<Option<Self>, SearchError> {
        if query.is_empty() {
            return Ok(None);
        }

        let matcher = match options.mode {
            SearchMode::Normal => PreparedMatcher::Literal(query.to_owned()),
            SearchMode::Extended => {
                let query = expand_extended_pattern(query);

                if query.is_empty() {
                    return Ok(None);
                }

                PreparedMatcher::Literal(query)
            }
            SearchMode::Regex => PreparedMatcher::Regex(build_regex(query, options)?),
        };

        Ok(Some(Self { matcher, options }))
    }

    pub fn matches(&self, text: &str) -> Vec<TextMatch> {
        if text.is_empty() {
            return Vec::new();
        }

        match &self.matcher {
            PreparedMatcher::Literal(query) => compute_literal_matches(text, query, self.options),
            PreparedMatcher::Regex(regex) => compute_regex_matches(text, regex, self.options),
        }
    }

    pub fn matches_in_chunks<'a>(
        &self,
        chunks: impl IntoIterator<Item = &'a str>,
    ) -> Vec<TextMatch> {
        match (&self.matcher, self.options.mode) {
            (PreparedMatcher::Literal(query), SearchMode::Normal | SearchMode::Extended) => {
                compute_literal_matches_in_chunks(chunks, query, self.options)
            }
            (PreparedMatcher::Regex(_), SearchMode::Regex) => {
                let text = chunks.into_iter().collect::<String>();
                self.matches(&text)
            }
            _ => Vec::new(),
        }
    }

    pub fn replacement_for_match(
        &self,
        text: &str,
        text_match: TextMatch,
        replacement: &str,
    ) -> String {
        match &self.matcher {
            PreparedMatcher::Literal(_) if self.options.mode == SearchMode::Extended => {
                expand_extended_pattern(replacement)
            }
            PreparedMatcher::Literal(_) => replacement.to_owned(),
            PreparedMatcher::Regex(regex) => {
                regex_replacement_for_match(text, text_match, regex, replacement)
                    .unwrap_or_else(|| replacement.to_owned())
            }
        }
    }
}

pub fn compute_matches(text: &str, query: &str, case_sensitive: bool) -> Vec<TextMatch> {
    compute_matches_with_options(text, query, SearchOptions::normal(case_sensitive, false))
}

pub fn compute_matches_with_options(
    text: &str,
    query: &str,
    options: SearchOptions,
) -> Vec<TextMatch> {
    try_matches(text, query, options).unwrap_or_default()
}

pub fn compute_matches_in_chunks<'a>(
    chunks: impl IntoIterator<Item = &'a str>,
    query: &str,
    options: SearchOptions,
) -> Vec<TextMatch> {
    let Ok(Some(search)) = PreparedSearch::new(query, options) else {
        return Vec::new();
    };

    search.matches_in_chunks(chunks)
}

pub fn try_matches(
    text: &str,
    query: &str,
    options: SearchOptions,
) -> Result<Vec<TextMatch>, SearchError> {
    Ok(PreparedSearch::new(query, options)?
        .map(|prepared| prepared.matches(text))
        .unwrap_or_default())
}

fn compute_literal_matches(text: &str, query: &str, options: SearchOptions) -> Vec<TextMatch> {
    if options.case_sensitive {
        return text
            .match_indices(query)
            .filter(|(start, value)| {
                !options.whole_word || is_whole_word_match(text, *start, start + value.len())
            })
            .map(|(start, value)| TextMatch::new(start, start + value.len()))
            .collect();
    }

    let query_folded = query.to_lowercase();
    let query_chars = query.chars().count();
    let mut matches = Vec::new();
    let mut cursor = 0;

    while cursor < text.len() {
        let Some(end) = end_after_chars(text, cursor, query_chars) else {
            break;
        };

        if text[cursor..end].to_lowercase() == query_folded
            && (!options.whole_word || is_whole_word_match(text, cursor, end))
        {
            matches.push(TextMatch::new(cursor, end));
            cursor = end;
        } else {
            cursor = next_char_boundary(text, cursor);
        }
    }

    matches
}

fn compute_literal_matches_in_chunks<'a>(
    chunks: impl IntoIterator<Item = &'a str>,
    query: &str,
    options: SearchOptions,
) -> Vec<TextMatch> {
    if query.is_empty() {
        return Vec::new();
    }

    let context_chars = query.chars().count().saturating_sub(1);
    let mut matches = Vec::new();
    let mut carry = String::new();
    let mut absolute_offset = 0usize;
    let mut emitted_until = 0usize;

    for chunk in chunks {
        if chunk.is_empty() {
            continue;
        }

        let carry_len = carry.len();
        let window_start = absolute_offset.saturating_sub(carry_len);
        let mut window = String::with_capacity(carry_len + chunk.len());
        window.push_str(&carry);
        window.push_str(chunk);

        for text_match in compute_literal_matches(&window, query, options) {
            let start = window_start + text_match.start;
            let end = window_start + text_match.end;
            if start >= emitted_until {
                matches.push(TextMatch::new(start, end));
                emitted_until = end;
            }
        }

        absolute_offset += chunk.len();
        carry = trailing_chars(&window, context_chars);
    }

    matches
}

fn trailing_chars(text: &str, max_chars: usize) -> String {
    if max_chars == 0 || text.is_empty() {
        return String::new();
    }

    let mut chars = VecDeque::with_capacity(max_chars);
    for ch in text.chars() {
        if chars.len() == max_chars {
            chars.pop_front();
        }
        chars.push_back(ch);
    }

    chars.into_iter().collect()
}

fn compute_regex_matches(text: &str, regex: &Regex, options: SearchOptions) -> Vec<TextMatch> {
    regex
        .find_iter(text)
        .filter(|text_match| {
            !text_match.is_empty()
                && (!options.whole_word
                    || is_whole_word_match(text, text_match.start(), text_match.end()))
        })
        .map(|text_match| TextMatch::new(text_match.start(), text_match.end()))
        .collect()
}

pub fn replacement_for_match(
    text: &str,
    text_match: TextMatch,
    query: &str,
    replacement: &str,
    options: SearchOptions,
) -> Result<String, SearchError> {
    Ok(PreparedSearch::new(query, options)?
        .map(|prepared| prepared.replacement_for_match(text, text_match, replacement))
        .unwrap_or_else(|| replacement.to_owned()))
}

fn build_regex(query: &str, options: SearchOptions) -> Result<Regex, SearchError> {
    RegexBuilder::new(query)
        .case_insensitive(!options.case_sensitive)
        .build()
        .map_err(|error| SearchError::InvalidRegex(error.to_string()))
}

fn regex_replacement_for_match(
    text: &str,
    text_match: TextMatch,
    regex: &Regex,
    replacement: &str,
) -> Option<String> {
    if text_match.end > text.len()
        || text_match.start > text_match.end
        || !text.is_char_boundary(text_match.start)
        || !text.is_char_boundary(text_match.end)
    {
        return None;
    }

    let matched = &text[text_match.start..text_match.end];
    let captures = regex.captures(matched)?;
    let full_match = captures.get(0)?;

    if full_match.start() != 0 || full_match.end() != matched.len() {
        return None;
    }

    let mut expanded = String::new();
    captures.expand(replacement, &mut expanded);
    Some(expanded)
}

pub fn replace_current(text: &str, text_match: TextMatch, replacement: &str) -> Option<String> {
    if text_match.end > text.len()
        || text_match.start > text_match.end
        || !text.is_char_boundary(text_match.start)
        || !text.is_char_boundary(text_match.end)
    {
        return None;
    }

    let mut replaced = String::with_capacity(text.len() + replacement.len());
    replaced.push_str(&text[..text_match.start]);
    replaced.push_str(replacement);
    replaced.push_str(&text[text_match.end..]);

    Some(replaced)
}

pub fn replace_all(
    text: &str,
    query: &str,
    replacement: &str,
    case_sensitive: bool,
) -> (String, usize) {
    let mut state = FindState {
        query: query.to_owned(),
        replacement: replacement.to_owned(),
        case_sensitive,
        whole_word: false,
        matches: Vec::new(),
        current_match: None,
    };

    state.replace_all(text)
}

fn is_whole_word_match(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start].chars().next_back();
    let after = text[end..].chars().next();

    !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
}

fn is_word_char(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

fn end_after_chars(text: &str, start: usize, char_count: usize) -> Option<usize> {
    let mut end = start;
    let mut chars_seen = 0;

    for (offset, ch) in text[start..].char_indices() {
        if chars_seen == char_count {
            break;
        }

        end = start + offset + ch.len_utf8();
        chars_seen += 1;
    }

    (chars_seen == char_count).then_some(end)
}

fn next_char_boundary(text: &str, start: usize) -> usize {
    text[start..]
        .chars()
        .next()
        .map(|ch| start + ch.len_utf8())
        .unwrap_or(text.len())
}

fn expand_extended_pattern(value: &str) -> String {
    let mut expanded = String::with_capacity(value.len());
    let mut chars = value.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            expanded.push(ch);
            continue;
        }

        match chars.next() {
            Some('n') => expanded.push('\n'),
            Some('r') => expanded.push('\r'),
            Some('t') => expanded.push('\t'),
            Some('0') => expanded.push('\0'),
            Some('\\') => expanded.push('\\'),
            Some(other) => {
                expanded.push('\\');
                expanded.push(other);
            }
            None => expanded.push('\\'),
        }
    }

    expanded
}
