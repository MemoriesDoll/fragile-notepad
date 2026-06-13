use super::fsm::ByteRange;
use super::lexical::OutlineCodeMask;
use super::scan::next_code_token;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CallableStatement {
    pub range: ByteRange,
    pub terminator: CallableStatementTerminator,
    pub is_expression_context: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CallableStatementTerminator {
    Body,
    Semicolon,
    Line,
}

pub(super) fn callable_statements(
    text: &str,
    mask: &OutlineCodeMask,
    operator_tokens: &[String],
    control_headers: &[String],
) -> Vec<CallableStatement> {
    CallableStatementScanner::new(text, mask, operator_tokens, control_headers).scan()
}

struct CallableStatementScanner<'a> {
    text: &'a str,
    mask: &'a OutlineCodeMask,
    operator_tokens: &'a [String],
    control_headers: &'a [String],
    statements: Vec<CallableStatement>,
    start: usize,
    cursor: usize,
    paren_depth: usize,
    bracket_depth: usize,
    expression_context: bool,
}

impl<'a> CallableStatementScanner<'a> {
    fn new(
        text: &'a str,
        mask: &'a OutlineCodeMask,
        operator_tokens: &'a [String],
        control_headers: &'a [String],
    ) -> Self {
        Self {
            text,
            mask,
            operator_tokens,
            control_headers,
            statements: Vec::new(),
            start: 0,
            cursor: 0,
            paren_depth: 0,
            bracket_depth: 0,
            expression_context: false,
        }
    }

    fn scan(mut self) -> Vec<CallableStatement> {
        while self.cursor < self.text.len() {
            let Some(ch) = self.current_char() else {
                break;
            };
            let len = ch.len_utf8();

            if self.mask.is_code(self.cursor) {
                self.visit_code_char(ch, len);
            }

            self.cursor += len;
        }

        self.finish_line_statement(self.text.len());
        self.statements
    }

    fn current_char(&self) -> Option<char> {
        self.text[self.cursor..].chars().next()
    }

    fn visit_code_char(&mut self, ch: char, len: usize) {
        match ch {
            '(' => self.paren_depth += 1,
            ')' => self.paren_depth = self.paren_depth.saturating_sub(1),
            '[' => self.bracket_depth += 1,
            ']' => self.bracket_depth = self.bracket_depth.saturating_sub(1),
            '{' if self.at_statement_boundary() => {
                self.finish_statement(self.cursor + len, CallableStatementTerminator::Body);
            }
            '}' if self.at_statement_boundary() => {
                self.finish_line_statement(self.cursor);
                self.reset_after(self.cursor + len);
            }
            ';' if self.at_statement_boundary() => {
                self.finish_statement(self.cursor + len, CallableStatementTerminator::Semicolon);
            }
            '=' if self.is_assignment_marker() => {
                self.expression_context = true;
            }
            _ => {}
        }
    }

    fn at_statement_boundary(&self) -> bool {
        self.paren_depth == 0 && self.bracket_depth == 0
    }

    fn is_assignment_marker(&self) -> bool {
        self.at_statement_boundary()
            && !code_at_starts_with_any(self.text, self.cursor, self.operator_tokens)
    }

    fn finish_statement(&mut self, end: usize, terminator: CallableStatementTerminator) {
        let start = statement_start(self.text, self.mask, self.start);
        if start < end {
            self.statements.push(CallableStatement {
                range: ByteRange::new(start, end),
                terminator,
                is_expression_context: self.expression_context
                    || statement_starts_with_control_header(
                        self.text,
                        self.mask,
                        start,
                        self.control_headers,
                    ),
            });
        }
        self.reset_after(end);
    }

    fn finish_line_statement(&mut self, end: usize) {
        self.finish_statement(end, CallableStatementTerminator::Line);
    }

    fn reset_after(&mut self, end: usize) {
        self.start = end;
        self.expression_context = false;
    }
}

fn statement_start(text: &str, mask: &OutlineCodeMask, mut start: usize) -> usize {
    while start < text.len() {
        let Some(ch) = text[start..].chars().next() else {
            break;
        };
        if mask.is_code(start) && !ch.is_whitespace() {
            break;
        }
        start += ch.len_utf8();
    }

    start
}

fn statement_starts_with_control_header(
    text: &str,
    mask: &OutlineCodeMask,
    start: usize,
    control_headers: &[String],
) -> bool {
    control_headers
        .iter()
        .any(|header| statement_starts_with_token_sequence(text, mask, start, header))
}

fn statement_starts_with_token_sequence(
    text: &str,
    mask: &OutlineCodeMask,
    start: usize,
    sequence: &str,
) -> bool {
    let mut cursor = start;

    for expected in sequence.split_whitespace() {
        let Some(token) = next_code_token(text, mask, cursor) else {
            return false;
        };
        if token.text(text) != expected {
            return false;
        }
        cursor = token.end;
    }

    true
}

fn code_at_starts_with_any(text: &str, offset: usize, candidates: &[String]) -> bool {
    let Some(rest) = text.get(offset..) else {
        return false;
    };

    candidates
        .iter()
        .any(|candidate| rest.starts_with(candidate.as_str()))
}
