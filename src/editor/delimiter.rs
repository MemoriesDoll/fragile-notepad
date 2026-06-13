#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DelimiterMatch {
    pub delimiter: usize,
    pub matching_delimiter: usize,
}

pub fn matching_delimiter_near_caret(text: &str, caret_offset: usize) -> Option<DelimiterMatch> {
    if caret_offset > text.len() || !text.is_char_boundary(caret_offset) {
        return None;
    }

    if let Some(delimiter_match) = previous_char_start(text, caret_offset)
        .filter(|offset| delimiter_kind(text.as_bytes()[*offset]).is_some())
        .and_then(|offset| matching_delimiter_at(text, offset))
    {
        return Some(delimiter_match);
    }

    text.get(caret_offset..)
        .and_then(|tail| tail.chars().next())
        .filter(|ch| ch.is_ascii() && delimiter_kind(*ch as u8).is_some())
        .map(|_| caret_offset)
        .and_then(|offset| matching_delimiter_at(text, offset))
}

pub fn matching_delimiter_at(text: &str, delimiter_offset: usize) -> Option<DelimiterMatch> {
    if delimiter_offset >= text.len() || !text.is_char_boundary(delimiter_offset) {
        return None;
    }

    let delimiter = text.as_bytes()[delimiter_offset];
    let kind = delimiter_kind(delimiter)?;
    let matching_delimiter = if kind.is_open {
        scan_forward(text, delimiter_offset, kind.open, kind.close)?
    } else {
        scan_backward(text, delimiter_offset, kind.open, kind.close)?
    };

    Some(DelimiterMatch {
        delimiter: delimiter_offset,
        matching_delimiter,
    })
}

fn scan_forward(text: &str, delimiter_offset: usize, open: u8, close: u8) -> Option<usize> {
    let mut depth = 0usize;

    for (relative_offset, ch) in text.get(delimiter_offset..)?.char_indices() {
        let offset = delimiter_offset + relative_offset;
        if !ch.is_ascii() {
            continue;
        }

        match ch as u8 {
            byte if byte == open => depth += 1,
            byte if byte == close => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(offset);
                }
            }
            _ => {}
        }
    }

    None
}

fn scan_backward(text: &str, delimiter_offset: usize, open: u8, close: u8) -> Option<usize> {
    let mut depth = 0usize;

    for (offset, ch) in text.get(..=delimiter_offset)?.char_indices().rev() {
        if !ch.is_ascii() {
            continue;
        }

        match ch as u8 {
            byte if byte == close => depth += 1,
            byte if byte == open => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(offset);
                }
            }
            _ => {}
        }
    }

    None
}

fn previous_char_start(text: &str, caret_offset: usize) -> Option<usize> {
    text.get(..caret_offset)?
        .char_indices()
        .next_back()
        .map(|(offset, _)| offset)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DelimiterKind {
    open: u8,
    close: u8,
    is_open: bool,
}

fn delimiter_kind(byte: u8) -> Option<DelimiterKind> {
    match byte {
        b'(' => Some(DelimiterKind {
            open: b'(',
            close: b')',
            is_open: true,
        }),
        b')' => Some(DelimiterKind {
            open: b'(',
            close: b')',
            is_open: false,
        }),
        b'[' => Some(DelimiterKind {
            open: b'[',
            close: b']',
            is_open: true,
        }),
        b']' => Some(DelimiterKind {
            open: b'[',
            close: b']',
            is_open: false,
        }),
        b'{' => Some(DelimiterKind {
            open: b'{',
            close: b'}',
            is_open: true,
        }),
        b'}' => Some(DelimiterKind {
            open: b'{',
            close: b'}',
            is_open: false,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{DelimiterMatch, matching_delimiter_at, matching_delimiter_near_caret};

    #[test]
    fn matches_nested_pairs_by_delimiter_kind() {
        assert_eq!(
            matching_delimiter_at("a({[]})z", 1),
            Some(DelimiterMatch {
                delimiter: 1,
                matching_delimiter: 6,
            })
        );
        assert_eq!(
            matching_delimiter_at("a({[]})z", 3),
            Some(DelimiterMatch {
                delimiter: 3,
                matching_delimiter: 4,
            })
        );
    }

    #[test]
    fn matches_backward_from_closing_delimiter() {
        assert_eq!(
            matching_delimiter_at("a({[]})z", 6),
            Some(DelimiterMatch {
                delimiter: 6,
                matching_delimiter: 1,
            })
        );
    }

    #[test]
    fn prefers_delimiter_before_caret() {
        assert_eq!(
            matching_delimiter_near_caret("()", 1),
            Some(DelimiterMatch {
                delimiter: 0,
                matching_delimiter: 1,
            })
        );
    }

    #[test]
    fn falls_back_to_delimiter_at_caret() {
        assert_eq!(
            matching_delimiter_near_caret("x()", 1),
            Some(DelimiterMatch {
                delimiter: 1,
                matching_delimiter: 2,
            })
        );
    }

    #[test]
    fn falls_back_to_delimiter_at_caret_when_previous_is_unmatched() {
        assert_eq!(
            matching_delimiter_near_caret("{()", 1),
            Some(DelimiterMatch {
                delimiter: 1,
                matching_delimiter: 2,
            })
        );
    }

    #[test]
    fn rejects_unmatched_or_non_boundary_offsets() {
        assert_eq!(matching_delimiter_at("(()", 0), None);
        assert_eq!(matching_delimiter_near_caret("a\u{597d}()", 2), None);
    }
}
