use iced::advanced::text::{self, Paragraph};
use iced::{Color, Font, Pixels, Point, Rectangle, Size, alignment};
use std::ops::Range;

use crate::editor::decoration::DecorationModel;
use crate::editor::layout::{EditorMetrics, visual_width_with_tab_width};
use crate::editor::render::RowRenderPlan;

use super::cache::{RichParagraphCache, SyntaxSpanKey};
use super::font::{EDITOR_FONT, EDITOR_TEXT_SHAPING};
use super::line_cache::LineGeometry;
use super::style::EditorStyle;

const MAX_SYNTAX_SPANS_PER_ROW: usize = 256;
const LONG_LINE_VISIBLE_MARGIN_COLUMNS: usize = 8;
const LONG_STYLE_RUN_SUBDIVISION_COLUMNS: usize = 100;

pub(super) fn can_batch_fast_text(text: &str) -> bool {
    text.bytes().all(|byte| byte.is_ascii() && byte != b'\t')
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_row_text<Renderer>(
    renderer: &mut Renderer,
    row: &RowRenderPlan,
    text_origin_x: f32,
    baseline_y: f32,
    metrics: EditorMetrics,
    decorations: &DecorationModel,
    line_geometry: &LineGeometry<Renderer::Paragraph>,
    style: EditorStyle,
    clip_bounds: Rectangle,
    frame_id: u64,
    rich_paragraphs: &mut RichParagraphCache<Renderer::Paragraph>,
    draw_plain_text: impl FnOnce(
        &mut Renderer,
        String,
        Point,
        Size,
        Color,
        text::Alignment,
        EditorMetrics,
        Rectangle,
    ),
) where
    Renderer: text::Renderer<Font = Font>,
{
    if let Some(expanded) = expand_tabs_for_rendering(&row.text, decorations.settings.indent_width)
    {
        let text_bounds = Size::new(
            line_geometry.width(decorations.settings.indent_width),
            metrics.line_height,
        );

        if row.syntax_spans.is_empty() || row.syntax_spans.len() > MAX_SYNTAX_SPANS_PER_ROW {
            draw_plain_text(
                renderer,
                expanded.text,
                Point::new(text_origin_x, baseline_y),
                text_bounds,
                style.syntax_fallback_text,
                text::Alignment::Left,
                metrics,
                clip_bounds,
            );
            return;
        }

        let expanded_row = RowRenderPlan {
            text: expanded.text,
            syntax_spans: remap_syntax_spans(&row.syntax_spans, &expanded.byte_offsets),
            ..row.clone()
        };

        if expanded_row.syntax_spans.is_empty() || !syntax_spans_are_valid(&expanded_row) {
            draw_plain_text(
                renderer,
                expanded_row.text,
                Point::new(text_origin_x, baseline_y),
                text_bounds,
                style.syntax_fallback_text,
                text::Alignment::Left,
                metrics,
                clip_bounds,
            );
            return;
        }

        draw_rich_row_text(
            renderer,
            &expanded_row,
            Point::new(text_origin_x, baseline_y),
            metrics.line_height,
            metrics,
            style,
            clip_bounds,
            frame_id,
            rich_paragraphs,
            draw_plain_text,
        );
        return;
    }

    if row.syntax_spans.is_empty() || row.syntax_spans.len() > MAX_SYNTAX_SPANS_PER_ROW {
        let text_bounds = Size::new(
            line_geometry.width(decorations.settings.indent_width),
            metrics.line_height,
        );

        draw_plain_text(
            renderer,
            row.text.clone(),
            Point::new(text_origin_x, baseline_y),
            text_bounds,
            style.syntax_fallback_text,
            text::Alignment::Left,
            metrics,
            clip_bounds,
        );
        return;
    }

    draw_rich_row_text(
        renderer,
        row,
        Point::new(text_origin_x, baseline_y),
        metrics.line_height,
        metrics,
        style,
        clip_bounds,
        frame_id,
        rich_paragraphs,
        draw_plain_text,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_rich_row_text<Renderer>(
    renderer: &mut Renderer,
    row: &RowRenderPlan,
    position: Point,
    height: f32,
    metrics: EditorMetrics,
    style: EditorStyle,
    clip_bounds: Rectangle,
    frame_id: u64,
    rich_paragraphs: &mut RichParagraphCache<Renderer::Paragraph>,
    draw_plain_text: impl FnOnce(
        &mut Renderer,
        String,
        Point,
        Size,
        Color,
        text::Alignment,
        EditorMetrics,
        Rectangle,
    ),
) where
    Renderer: text::Renderer<Font = Font>,
{
    let visible_range = visible_rich_text_range(row, position.x, metrics, clip_bounds);
    let visible_text = &row.text[visible_range.clone()];
    let visible_position = Point::new(
        position.x + visible_range.start as f32 * metrics.character_width,
        position.y,
    );
    let visible_bounds = Size::new(
        (visible_range.end - visible_range.start) as f32 * metrics.character_width,
        height,
    );
    let mut span_keys = Vec::new();
    let mut cursor = visible_range.start;
    let first_span = first_visible_syntax_span(row, visible_range.start);
    let fallback_color = style.syntax_fallback_text;

    for syntax_span in &row.syntax_spans[first_span..] {
        if syntax_span.range.start >= visible_range.end {
            break;
        }

        let start = syntax_span.range.start.max(visible_range.start);
        let end = syntax_span.range.end.min(visible_range.end);

        if start >= end
            || end > row.text.len()
            || !row.text.is_char_boundary(start)
            || !row.text.is_char_boundary(end)
        {
            continue;
        }

        if cursor < start {
            let local_start = cursor - visible_range.start;
            let local_end = start - visible_range.start;

            push_syntax_span_key(
                &mut span_keys,
                SyntaxSpanKey {
                    start: local_start,
                    end: local_end,
                    color: Some(fallback_color),
                },
            );
        }

        push_syntax_span_key(
            &mut span_keys,
            SyntaxSpanKey {
                start: start - visible_range.start,
                end: end - visible_range.start,
                color: syntax_span.color.or(Some(fallback_color)),
            },
        );
        cursor = end;
    }

    if cursor < visible_range.end {
        push_syntax_span_key(
            &mut span_keys,
            SyntaxSpanKey {
                start: cursor - visible_range.start,
                end: visible_range.end - visible_range.start,
                color: Some(fallback_color),
            },
        );
    }

    if span_keys.is_empty() {
        draw_plain_text(
            renderer,
            visible_text.to_owned(),
            visible_position,
            visible_bounds,
            style.syntax_fallback_text,
            text::Alignment::Left,
            metrics,
            clip_bounds,
        );
        return;
    }

    let size = Pixels((metrics.line_height / 1.25).max(8.0));
    let scale_factor = renderer.scale_factor();
    let paragraph = rich_paragraphs.get_or_insert_with(
        row.line,
        visible_text,
        &span_keys,
        visible_range.start,
        visible_bounds,
        size,
        metrics.line_height,
        scale_factor,
        frame_id,
        || {
            let mut spans: Vec<text::Span<'_, (), Font>> = Vec::with_capacity(span_keys.len());
            for key in &span_keys {
                spans.push(
                    text::Span::new(&visible_text[key.start..key.end]).color_maybe(key.color),
                );
            }

            Renderer::Paragraph::with_spans(text::Text {
                content: spans.as_slice(),
                bounds: visible_bounds,
                size,
                line_height: text::LineHeight::Absolute(Pixels(metrics.line_height)),
                font: EDITOR_FONT,
                align_x: text::Alignment::Left,
                align_y: alignment::Vertical::Top,
                shaping: EDITOR_TEXT_SHAPING,
                wrapping: text::Wrapping::None,
                ellipsis: text::Ellipsis::None,
                hint_factor: scale_factor,
            })
        },
    );

    renderer.fill_paragraph(
        paragraph,
        visible_position,
        style.syntax_fallback_text,
        clip_bounds,
    );
}

pub(super) fn visible_rich_text_range(
    row: &RowRenderPlan,
    text_origin_x: f32,
    metrics: EditorMetrics,
    clip_bounds: Rectangle,
) -> Range<usize> {
    visible_styled_text_range(row, text_origin_x, metrics, clip_bounds)
}

pub(super) fn push_syntax_span_key(span_keys: &mut Vec<SyntaxSpanKey>, key: SyntaxSpanKey) {
    if key.start >= key.end {
        return;
    }

    if let Some(previous) = span_keys.last_mut()
        && previous.end == key.start
        && previous.color == key.color
    {
        previous.end = key.end;
        return;
    }

    span_keys.push(key);
}

pub(super) fn visible_styled_text_range(
    row: &RowRenderPlan,
    text_origin_x: f32,
    metrics: EditorMetrics,
    clip_bounds: Rectangle,
) -> Range<usize> {
    if row.text.is_empty() || !can_batch_fast_text(&row.text) || metrics.character_width <= 0.0 {
        return 0..row.text.len();
    }

    if !syntax_spans_are_valid(row) {
        return 0..row.text.len();
    }

    let first = ((clip_bounds.x - text_origin_x) / metrics.character_width)
        .floor()
        .max(0.0) as usize;
    let last = ((clip_bounds.x + clip_bounds.width - text_origin_x) / metrics.character_width)
        .ceil()
        .max(0.0) as usize;
    let start = first.saturating_sub(LONG_LINE_VISIBLE_MARGIN_COLUMNS);
    let end = last
        .saturating_add(LONG_LINE_VISIBLE_MARGIN_COLUMNS)
        .min(row.text.len());
    let start = style_boundary_start(row, start.min(row.text.len()));

    start..end
}

fn syntax_spans_are_valid(row: &RowRenderPlan) -> bool {
    let mut previous_end = 0;

    row.syntax_spans.iter().all(|span| {
        let valid = previous_end <= span.range.start
            && span.range.start < span.range.end
            && span.range.end <= row.text.len()
            && row.text.is_char_boundary(span.range.start)
            && row.text.is_char_boundary(span.range.end);

        previous_end = span.range.end;
        valid
    })
}

fn style_boundary_start(row: &RowRenderPlan, start: usize) -> usize {
    let boundary = row
        .syntax_spans
        .iter()
        .filter(|span| span.range.start < start && start < span.range.end)
        .map(|span| span.range.start)
        .max()
        .unwrap_or(start);

    if start.saturating_sub(boundary) > LONG_STYLE_RUN_SUBDIVISION_COLUMNS {
        start - (start - boundary) % LONG_STYLE_RUN_SUBDIVISION_COLUMNS
    } else {
        boundary
    }
}

pub(super) fn first_visible_syntax_span(row: &RowRenderPlan, visible_start: usize) -> usize {
    row.syntax_spans
        .partition_point(|span| span.range.end <= visible_start)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpandedTabs {
    text: String,
    byte_offsets: Vec<usize>,
}

fn expand_tabs_for_rendering(text: &str, tab_width: usize) -> Option<ExpandedTabs> {
    if !text.contains('\t') {
        return None;
    }

    let mut expanded = String::with_capacity(text.len());
    let mut byte_offsets = vec![0; text.len() + 1];
    let mut visual_column = 0usize;
    let mut expanded_offset = 0usize;

    for (offset, ch) in text.char_indices() {
        byte_offsets[offset] = expanded_offset;

        if ch == '\t' {
            let spaces = visual_width_with_tab_width(ch, visual_column, tab_width);
            expanded.extend(std::iter::repeat_n(' ', spaces));
            visual_column += spaces;
            expanded_offset += spaces;
        } else {
            expanded.push(ch);
            visual_column += visual_width_with_tab_width(ch, visual_column, tab_width);
            expanded_offset += ch.len_utf8();
        }

        byte_offsets[offset + ch.len_utf8()] = expanded_offset;
    }

    byte_offsets[text.len()] = expanded_offset;

    Some(ExpandedTabs {
        text: expanded,
        byte_offsets,
    })
}

fn remap_syntax_spans(
    spans: &[crate::editor::render::SyntaxRenderSpan],
    byte_offsets: &[usize],
) -> Vec<crate::editor::render::SyntaxRenderSpan> {
    spans
        .iter()
        .filter_map(|span| {
            let start = *byte_offsets.get(span.range.start)?;
            let end = *byte_offsets.get(span.range.end)?;

            (start < end).then_some(crate::editor::render::SyntaxRenderSpan {
                range: start..end,
                color: span.color,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::render::SyntaxRenderSpan;

    #[test]
    fn syntax_span_keys_merge_adjacent_equal_styles() {
        let color = Some(Color::from_rgb(0.8, 0.2, 0.1));
        let mut keys = Vec::new();

        push_syntax_span_key(
            &mut keys,
            SyntaxSpanKey {
                start: 0,
                end: 3,
                color,
            },
        );
        push_syntax_span_key(
            &mut keys,
            SyntaxSpanKey {
                start: 3,
                end: 7,
                color,
            },
        );
        push_syntax_span_key(
            &mut keys,
            SyntaxSpanKey {
                start: 7,
                end: 9,
                color: Some(Color::from_rgb(0.2, 0.4, 0.6)),
            },
        );

        assert_eq!(
            keys,
            vec![
                SyntaxSpanKey {
                    start: 0,
                    end: 7,
                    color,
                },
                SyntaxSpanKey {
                    start: 7,
                    end: 9,
                    color: Some(Color::from_rgb(0.2, 0.4, 0.6)),
                },
            ]
        );
    }

    #[test]
    fn tab_expansion_uses_configured_tab_stops_for_render_text() {
        let expanded = expand_tabs_for_rendering("a\tb", 8).expect("tabbed text");

        assert_eq!(expanded.text, "a       b");
        assert_eq!(expanded.byte_offsets[0], 0);
        assert_eq!(expanded.byte_offsets[1], 1);
        assert_eq!(expanded.byte_offsets[2], 8);
        assert_eq!(expanded.byte_offsets[3], 9);
    }

    #[test]
    fn syntax_spans_remap_to_expanded_tab_text() {
        let expanded = expand_tabs_for_rendering("a\tbc", 4).expect("tabbed text");
        let spans = vec![
            SyntaxRenderSpan {
                range: 0..1,
                color: Some(Color::from_rgb(1.0, 0.0, 0.0)),
            },
            SyntaxRenderSpan {
                range: 1..4,
                color: Some(Color::from_rgb(0.0, 1.0, 0.0)),
            },
        ];

        assert_eq!(
            remap_syntax_spans(&spans, &expanded.byte_offsets),
            vec![
                SyntaxRenderSpan {
                    range: 0..1,
                    color: Some(Color::from_rgb(1.0, 0.0, 0.0)),
                },
                SyntaxRenderSpan {
                    range: 1..6,
                    color: Some(Color::from_rgb(0.0, 1.0, 0.0)),
                },
            ]
        );
    }

    #[test]
    fn rich_text_visible_range_limits_long_ascii_lines_to_clip_columns() {
        let row = RowRenderPlan {
            visible_row: 0,
            line: 0,
            y: 0.0,
            text_x: 0.0,
            text: "x".repeat(1_000),
            line_number: None,
            is_active_line: false,
            fold: None,
            hidden_lines: None,
            whitespace: Vec::new(),
            eol: None,
            indent_guides: Vec::new(),
            syntax_spans: Vec::new(),
        };
        let metrics = EditorMetrics {
            character_width: 10.0,
            ..EditorMetrics::default()
        };
        let range = visible_rich_text_range(
            &row,
            0.0,
            metrics,
            Rectangle {
                x: 100.0,
                y: 0.0,
                width: 50.0,
                height: 20.0,
            },
        );

        assert_eq!(range, 2..23);
    }

    #[test]
    fn rich_text_visible_range_backs_up_to_syntax_boundary() {
        let row = RowRenderPlan {
            visible_row: 0,
            line: 0,
            y: 0.0,
            text_x: 0.0,
            text: "a".repeat(120),
            line_number: None,
            is_active_line: false,
            fold: None,
            hidden_lines: None,
            whitespace: Vec::new(),
            eol: None,
            indent_guides: Vec::new(),
            syntax_spans: vec![
                SyntaxRenderSpan {
                    range: 0..16,
                    color: Some(Color::from_rgb(1.0, 0.0, 0.0)),
                },
                SyntaxRenderSpan {
                    range: 16..80,
                    color: Some(Color::from_rgb(0.0, 1.0, 0.0)),
                },
                SyntaxRenderSpan {
                    range: 80..120,
                    color: Some(Color::from_rgb(0.0, 0.0, 1.0)),
                },
            ],
        };
        let metrics = EditorMetrics {
            character_width: 10.0,
            ..EditorMetrics::default()
        };
        let range = visible_styled_text_range(
            &row,
            0.0,
            metrics,
            Rectangle {
                x: 330.0,
                y: 0.0,
                width: 60.0,
                height: 20.0,
            },
        );

        assert_eq!(range, 16..47);
    }

    #[test]
    fn rich_text_visible_range_subdivides_long_syntax_runs() {
        let row = RowRenderPlan {
            visible_row: 0,
            line: 0,
            y: 0.0,
            text_x: 0.0,
            text: "a".repeat(1_000),
            line_number: None,
            is_active_line: false,
            fold: None,
            hidden_lines: None,
            whitespace: Vec::new(),
            eol: None,
            indent_guides: Vec::new(),
            syntax_spans: vec![SyntaxRenderSpan {
                range: 0..1_000,
                color: Some(Color::from_rgb(1.0, 0.0, 0.0)),
            }],
        };
        let metrics = EditorMetrics {
            character_width: 10.0,
            ..EditorMetrics::default()
        };
        let range = visible_styled_text_range(
            &row,
            0.0,
            metrics,
            Rectangle {
                x: 3_330.0,
                y: 0.0,
                width: 60.0,
                height: 20.0,
            },
        );

        assert_eq!(range, 300..347);
    }

    #[test]
    fn first_visible_syntax_span_skips_offscreen_spans() {
        let syntax_spans = (0..1_000)
            .map(|index| SyntaxRenderSpan {
                range: index * 8..index * 8 + 8,
                color: Some(Color::from_rgb(1.0, 0.0, 0.0)),
            })
            .collect();
        let row = RowRenderPlan {
            visible_row: 0,
            line: 0,
            y: 0.0,
            text_x: 0.0,
            text: "a".repeat(8_000),
            line_number: None,
            is_active_line: false,
            fold: None,
            hidden_lines: None,
            whitespace: Vec::new(),
            eol: None,
            indent_guides: Vec::new(),
            syntax_spans,
        };

        assert_eq!(first_visible_syntax_span(&row, 3_200), 400);
    }

    #[test]
    fn rich_text_visible_range_keeps_tabs_on_full_row_fallback() {
        let row = RowRenderPlan {
            visible_row: 0,
            line: 0,
            y: 0.0,
            text_x: 0.0,
            text: format!("{}\t{}", "a".repeat(40), "b".repeat(40)),
            line_number: None,
            is_active_line: false,
            fold: None,
            hidden_lines: None,
            whitespace: Vec::new(),
            eol: None,
            indent_guides: Vec::new(),
            syntax_spans: Vec::new(),
        };
        let metrics = EditorMetrics {
            character_width: 10.0,
            ..EditorMetrics::default()
        };

        assert_eq!(
            visible_styled_text_range(
                &row,
                0.0,
                metrics,
                Rectangle {
                    x: 100.0,
                    y: 0.0,
                    width: 50.0,
                    height: 20.0,
                },
            ),
            0..row.text.len()
        );
    }

    #[test]
    fn rich_text_visible_range_keeps_non_ascii_on_full_row_fallback() {
        let row = RowRenderPlan {
            visible_row: 0,
            line: 0,
            y: 0.0,
            text_x: 0.0,
            text: format!("{}\u{6f20}\u{7958}{}", "a".repeat(40), "b".repeat(40)),
            line_number: None,
            is_active_line: false,
            fold: None,
            hidden_lines: None,
            whitespace: Vec::new(),
            eol: None,
            indent_guides: Vec::new(),
            syntax_spans: Vec::new(),
        };
        let metrics = EditorMetrics {
            character_width: 10.0,
            ..EditorMetrics::default()
        };

        assert_eq!(
            visible_styled_text_range(
                &row,
                0.0,
                metrics,
                Rectangle {
                    x: 100.0,
                    y: 0.0,
                    width: 50.0,
                    height: 20.0,
                },
            ),
            0..row.text.len()
        );
    }

    #[test]
    fn rich_text_visible_range_falls_back_for_invalid_syntax_spans() {
        let row = RowRenderPlan {
            visible_row: 0,
            line: 0,
            y: 0.0,
            text_x: 0.0,
            text: "a".repeat(120),
            line_number: None,
            is_active_line: false,
            fold: None,
            hidden_lines: None,
            whitespace: Vec::new(),
            eol: None,
            indent_guides: Vec::new(),
            syntax_spans: vec![SyntaxRenderSpan {
                range: 12..128,
                color: Some(Color::from_rgb(1.0, 0.0, 0.0)),
            }],
        };
        let metrics = EditorMetrics {
            character_width: 10.0,
            ..EditorMetrics::default()
        };

        assert_eq!(
            visible_styled_text_range(
                &row,
                0.0,
                metrics,
                Rectangle {
                    x: 100.0,
                    y: 0.0,
                    width: 50.0,
                    height: 20.0,
                },
            ),
            0..row.text.len()
        );
    }
}
