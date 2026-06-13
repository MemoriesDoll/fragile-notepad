use super::buffer::EditorBuffer;
use super::decoration::{DecorationModel, HiddenLineSpan, IndentGuide, LineDecoration};
use super::layout::{
    EditorLayout, EditorMetrics, byte_column_for, caret_x, row_y, scrolled_text_origin_x,
    visual_column_for, visual_width_with_tab_width, x_for_visual_column,
};
use super::position::{
    EditorPosition, EditorSelection, ProjectedSelectionLine, SelectionRange, SelectionSet,
    SelectionShape,
};
use super::viewport::ViewportModel;
use iced::advanced::text::Highlighter as _;
use iced::{Color, Rectangle, highlighter};
use std::ops::Range;

const DEFAULT_SYNTAX_TOKEN: &str = "txt";
const GUTTER_RIGHT_MARGIN: f32 = 6.0;

#[derive(Debug, Clone, PartialEq)]
pub struct RenderPlan {
    pub rows: Vec<RowRenderPlan>,
    pub selections: Vec<SelectionRenderPlan>,
    pub carets: Vec<CaretRenderPlan>,
    pub caret: Option<CaretRenderPlan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowRenderPlan {
    pub visible_row: usize,
    pub line: usize,
    pub y: f32,
    pub text_x: f32,
    pub text: String,
    pub line_number: Option<usize>,
    pub is_active_line: bool,
    pub fold: Option<FoldRenderPlan>,
    pub hidden_lines: Option<HiddenLineRenderPlan>,
    pub whitespace: Vec<WhitespaceRenderPlan>,
    pub eol: Option<EolRenderPlan>,
    pub indent_guides: Vec<IndentGuideRenderPlan>,
    pub syntax_spans: Vec<SyntaxRenderSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxRenderSpan {
    pub range: Range<usize>,
    pub color: Option<Color>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FoldRenderPlan {
    pub line: usize,
    pub collapsed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HiddenLineRenderPlan {
    pub first_hidden_line: usize,
    pub last_hidden_line: usize,
    pub hidden_line_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhitespaceKind {
    Space,
    Tab,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WhitespaceRenderPlan {
    pub line: usize,
    pub column: usize,
    pub x: f32,
    pub kind: WhitespaceKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EolRenderPlan {
    pub line: usize,
    pub x: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IndentGuideRenderPlan {
    pub line: usize,
    pub depth: usize,
    pub x: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectionRenderPlan {
    pub line: usize,
    pub start_column: usize,
    pub end_column: usize,
    pub start_visual_column: usize,
    pub end_visual_column: usize,
    pub start_virtual_column: Option<usize>,
    pub end_virtual_column: Option<usize>,
    pub y: f32,
    pub x: f32,
    pub width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaretRenderPlan {
    pub position: EditorPosition,
    pub visual_column: Option<usize>,
    pub x: f32,
    pub y: f32,
    pub height: f32,
}

/// Estimates the number of text draw calls needed for editor row text.
///
/// This helper deliberately counts only text primitives. It does not include
/// quads for selections, active-line backgrounds, indentation guides, carets,
/// scrollbars, or decorative visible-space dots.
///
/// When `fast_text` is enabled and every visible row is tab-free ASCII text
/// without syntax spans, the widget can batch all visible rows into a single
/// clipped multiline text item. Highlighted rows are kept separate so syntax
/// colors remain visible during active scrolling.
pub fn planned_text_draws(plan: &RenderPlan, fast_text: bool) -> usize {
    if fast_text
        && plan.rows.iter().all(|row| {
            row.syntax_spans.is_empty()
                && row
                    .text
                    .bytes()
                    .all(|byte| byte.is_ascii() && byte != b'\t')
        })
    {
        return usize::from(!plan.rows.is_empty());
    }

    plan.rows.len()
}

/// Estimates text draw calls after optional whitespace marker rendering.
///
/// Visible space markers are intentionally excluded because they render as tiny
/// solid quads on the software renderer fast path. Tab and end-of-line markers
/// render as raster Heroicons, so they do not add text draw calls.
///
/// The function is used by performance tests and profiling examples as a stable
/// guard against accidentally returning decorative spaces to per-glyph text
/// rendering.
pub fn planned_text_draws_with_markers(plan: &RenderPlan, fast_text: bool) -> usize {
    planned_text_draws(plan, fast_text)
}

/// Returns the left edge used for right-aligned line numbers.
///
/// The editor gutter anchors line numbers to a stable right edge so the text
/// does not jitter as the visible line range changes from one digit count to
/// another. `text_width` should be the measured or monospace-estimated width of
/// the concrete line number.
pub fn line_number_left_x(metrics: EditorMetrics, text_width: f32) -> f32 {
    line_number_text_x(metrics) - text_width
}

/// Returns the right-edge text anchor for line numbers.
///
/// The value is expressed in editor-local coordinates, not absolute widget
/// coordinates. Callers should add the widget bounds origin before drawing.
pub fn line_number_text_x(metrics: EditorMetrics) -> f32 {
    metrics.padding_left + metrics.line_number_width - GUTTER_RIGHT_MARGIN
}

/// Computes the baseline offset used by all editor text draws.
///
/// The editor renders text in a fixed-height row, but iced text positioning is
/// top-based. This offset vertically centers the configured text size inside
/// `metrics.line_height`; callers add it to the row's top before drawing text.
pub fn text_baseline_offset(metrics: EditorMetrics) -> f32 {
    (metrics.line_height - text_size(metrics)) / 2.0
}

/// Computes the editor text size derived from row metrics.
///
/// Keeping this calculation centralized ensures plain text, rich text, line
/// numbers, markers, hit-test measurement, and profiler fixtures all agree on
/// the same font size.
pub fn text_size(metrics: EditorMetrics) -> f32 {
    (metrics.line_height / 1.25).max(8.0)
}

/// Returns the visible visual-column range for marker rendering.
///
/// `text_origin_x` is the absolute x coordinate of visual column zero after
/// horizontal scrolling. `clip_bounds` is the text clip rectangle in absolute
/// widget coordinates. The result is inclusive and intentionally rounded outward
/// so partially visible markers are still drawn.
pub fn visible_marker_columns(
    text_origin_x: f32,
    character_width: f32,
    clip_bounds: Rectangle,
) -> Option<(usize, usize)> {
    if character_width <= 0.0 || clip_bounds.width <= 0.0 {
        return None;
    }

    let first = ((clip_bounds.x - text_origin_x) / character_width)
        .floor()
        .max(0.0) as usize;
    let last = ((clip_bounds.x + clip_bounds.width - text_origin_x) / character_width)
        .ceil()
        .max(0.0) as usize;

    Some((first, last))
}

/// Returns the rectangle for a decorative visible-space dot.
///
/// The rectangle is in absolute widget coordinates. Space markers use this
/// geometry so they can render as tiny solid quads instead of one-character text
/// items. That keeps the common visible-spaces setting on the renderer's solid
/// rectangle fast path while preserving centered dot placement in each
/// monospace cell.
pub fn space_marker_bounds(
    bounds: Rectangle,
    layout: EditorLayout,
    decorations: &DecorationModel,
    row: &RowRenderPlan,
    visual_column: usize,
) -> Rectangle {
    let metrics = layout.metrics;
    let dot_size = space_marker_size(metrics);
    let x = bounds.x
        + scrolled_text_origin_x(layout, decorations)
        + visual_column as f32 * metrics.character_width
        + (metrics.character_width - dot_size) / 2.0;
    let y = bounds.y + row.y + (metrics.line_height - dot_size) / 2.0;

    Rectangle {
        x,
        y,
        width: dot_size,
        height: dot_size,
    }
}

/// Returns the side length of a visible-space dot in physical editor units.
///
/// The value is capped so dots remain visible at small font sizes and do not
/// become visually dominant at larger zoom levels.
pub fn space_marker_size(metrics: EditorMetrics) -> f32 {
    (metrics.character_width / 4.0).clamp(1.0, 2.0)
}

pub fn build_render_plan(
    buffer: &EditorBuffer,
    viewport: &ViewportModel,
    decorations: &DecorationModel,
    selection: EditorSelection,
    layout: EditorLayout,
    syntax_settings: &highlighter::Settings,
) -> RenderPlan {
    let syntax_cache = SyntaxLineCache::rebuild(buffer, syntax_settings);

    build_render_plan_for_selection_set_with_cache(
        buffer,
        viewport,
        decorations,
        SelectionSet::single(selection),
        layout,
        &syntax_cache,
    )
}

pub fn build_render_plan_with_cache(
    buffer: &EditorBuffer,
    viewport: &ViewportModel,
    decorations: &DecorationModel,
    selection: EditorSelection,
    layout: EditorLayout,
    syntax_cache: &SyntaxLineCache,
) -> RenderPlan {
    build_render_plan_for_selection_set_with_cache(
        buffer,
        viewport,
        decorations,
        SelectionSet::single(selection),
        layout,
        syntax_cache,
    )
}

pub fn build_render_plan_for_selection_set_with_cache(
    buffer: &EditorBuffer,
    viewport: &ViewportModel,
    decorations: &DecorationModel,
    selections: SelectionSet,
    layout: EditorLayout,
    syntax_cache: &SyntaxLineCache,
) -> RenderPlan {
    let main_selection = selections.main();
    let active_line = main_selection.cursor.line;
    let mut rows = Vec::new();
    let lookups = RenderDecorationLookups::new(decorations);
    let text_x = scrolled_text_origin_x(layout, decorations);
    let max_row = layout
        .scroll
        .first_visible_row
        .saturating_add(layout.visible_row_capacity());
    for visible_row in layout.scroll.first_visible_row..=max_row {
        let Some(line) = viewport.visible_row_to_document_line(visible_row) else {
            break;
        };
        let text = buffer.line(line).unwrap_or("").to_owned();
        let y = row_y(visible_row, layout);
        let line_decoration = lookups.line_decoration(line);
        let fold = line_decoration
            .filter(|decoration| decoration.has_fold_control)
            .map(|decoration| FoldRenderPlan {
                line,
                collapsed: decoration.is_fold_collapsed,
            });
        let hidden_lines = lookups
            .hidden_line_span(line)
            .map(|span| HiddenLineRenderPlan {
                first_hidden_line: span.first_hidden_line,
                last_hidden_line: span.last_hidden_line,
                hidden_line_count: span.hidden_line_count(),
            });
        let syntax_spans = syntax_cache.spans(line).unwrap_or_default();

        rows.push(RowRenderPlan {
            visible_row,
            line,
            y,
            text_x,
            text: text.clone(),
            line_number: line_decoration.and_then(|decoration| decoration.line_number),
            is_active_line: line == active_line,
            fold,
            hidden_lines,
            whitespace: whitespace_plan(line, &text, decorations, layout),
            eol: eol_plan(line, &text, decorations, layout),
            indent_guides: indent_guide_plan(
                line,
                lookups.indent_guides(line),
                decorations,
                layout,
            ),
            syntax_spans,
        });
    }

    let projected_selection_lines =
        projected_visible_selection_lines(buffer, viewport, decorations, &selections, layout);
    let selection_plans = projected_selection_lines
        .iter()
        .filter_map(|selection| {
            selection_plan_from_projected(viewport, decorations, *selection, layout)
        })
        .collect();
    let projected_caret_lines =
        projected_visible_caret_lines(buffer, viewport, decorations, &selections, layout);
    let carets = projected_caret_lines
        .iter()
        .filter_map(|selection| {
            caret_plan_from_projected(buffer, viewport, decorations, *selection, layout)
        })
        .collect::<Vec<_>>();
    let caret = selections
        .main_range()
        .is_caret()
        .then(|| {
            caret_plan_for_position(
                buffer,
                viewport,
                decorations,
                selections.main_range().cursor,
                selections.main_range().cursor_virtual_column,
                layout,
            )
        })
        .flatten();

    RenderPlan {
        rows,
        selections: selection_plans,
        carets,
        caret,
    }
}

fn projected_visible_selection_lines(
    buffer: &EditorBuffer,
    viewport: &ViewportModel,
    decorations: &DecorationModel,
    selections: &SelectionSet,
    layout: EditorLayout,
) -> Vec<ProjectedSelectionLine> {
    let first = layout.scroll.first_visible_row;
    let last = first.saturating_add(layout.visible_row_capacity());
    let mut projected = Vec::new();

    for visible_row in first..=last {
        let Some(line) = viewport.visible_row_to_document_line(visible_row) else {
            break;
        };

        for selection in selections.ranges() {
            if let Some(line) =
                project_selection_line(*selection, line, buffer, decorations.settings.indent_width)
            {
                projected.push(line);
            }
        }
    }

    projected
}

fn projected_visible_caret_lines(
    buffer: &EditorBuffer,
    viewport: &ViewportModel,
    decorations: &DecorationModel,
    selections: &SelectionSet,
    layout: EditorLayout,
) -> Vec<ProjectedSelectionLine> {
    let first = layout.scroll.first_visible_row;
    let last = first.saturating_add(layout.visible_row_capacity());
    let mut projected = Vec::new();

    for visible_row in first..=last {
        let Some(line) = viewport.visible_row_to_document_line(visible_row) else {
            break;
        };

        for selection in selections.ranges() {
            if !selection.is_caret() && !selection.is_rectangular() {
                continue;
            }

            if let Some(line) =
                project_selection_line(*selection, line, buffer, decorations.settings.indent_width)
            {
                projected.push(line);
            }
        }
    }

    projected
}

fn project_selection_line(
    selection: SelectionRange,
    line: usize,
    buffer: &EditorBuffer,
    tab_width: usize,
) -> Option<ProjectedSelectionLine> {
    match selection.shape {
        SelectionShape::Linear => project_linear_selection_line(selection, line, buffer, tab_width),
        SelectionShape::Rectangular(rectangular) => {
            let anchor = buffer.clamp_position(selection.anchor);
            let cursor = buffer.clamp_position(selection.cursor);
            let (first_line, last_line) = if anchor.line <= cursor.line {
                (anchor.line, cursor.line)
            } else {
                (cursor.line, anchor.line)
            };
            if !(first_line..=last_line).contains(&line) {
                return None;
            }

            let text = buffer.line(line)?;
            let (start_visual_column, end_visual_column) = rectangular.visual_columns();
            let line_visual_width = visual_column_for(text, text.len(), tab_width);
            let start_column = byte_column_for(text, start_visual_column, tab_width);
            let end_column = byte_column_for(text, end_visual_column, tab_width);

            Some(ProjectedSelectionLine {
                line,
                start: EditorPosition::new(line, start_column),
                end: EditorPosition::new(line, end_column),
                start_visual_column,
                end_visual_column,
                start_virtual_column: (start_visual_column > line_visual_width)
                    .then_some(start_visual_column),
                end_virtual_column: (end_visual_column > line_visual_width)
                    .then_some(end_visual_column),
            })
        }
    }
}

fn project_linear_selection_line(
    selection: SelectionRange,
    line: usize,
    buffer: &EditorBuffer,
    tab_width: usize,
) -> Option<ProjectedSelectionLine> {
    let range = buffer.clamp_range(selection.range());
    if !(range.start.line..=range.end.line).contains(&line) {
        return None;
    }

    let text = buffer.line(line)?;
    let start_column = if line == range.start.line {
        range.start.column
    } else {
        0
    };
    let end_column = if line == range.end.line {
        range.end.column
    } else {
        text.len()
    };
    let start = buffer.clamp_position(EditorPosition::new(line, start_column));
    let end = buffer.clamp_position(EditorPosition::new(line, end_column));

    let start_visual_column = selection
        .anchor_virtual_column
        .filter(|_| start == selection.anchor)
        .unwrap_or_else(|| visual_column_for(text, start.column, tab_width));
    let mut end_visual_column = selection
        .cursor_virtual_column
        .filter(|_| end == selection.cursor)
        .unwrap_or_else(|| visual_column_for(text, end.column, tab_width));

    if start_visual_column == end_visual_column && line < range.end.line {
        end_visual_column = end_visual_column.saturating_add(1);
    }

    Some(ProjectedSelectionLine {
        line,
        start,
        end,
        start_visual_column,
        end_visual_column,
        start_virtual_column: selection
            .anchor_virtual_column
            .filter(|_| start == selection.anchor),
        end_virtual_column: selection
            .cursor_virtual_column
            .filter(|_| end == selection.cursor),
    })
}

fn selection_plan_from_projected(
    viewport: &ViewportModel,
    decorations: &DecorationModel,
    selection: ProjectedSelectionLine,
    layout: EditorLayout,
) -> Option<SelectionRenderPlan> {
    if selection.start_visual_column == selection.end_visual_column {
        return None;
    }

    let visible_row = visible_row_in_layout(viewport, selection.line, layout)?;
    let start_visual_column = selection
        .start_visual_column
        .min(selection.end_visual_column);
    let end_visual_column = selection
        .start_visual_column
        .max(selection.end_visual_column);

    Some(SelectionRenderPlan {
        line: selection.line,
        start_column: selection.start.column,
        end_column: selection.end.column,
        start_visual_column,
        end_visual_column,
        start_virtual_column: selection.start_virtual_column,
        end_virtual_column: selection.end_virtual_column,
        y: row_y(visible_row, layout),
        x: x_for_visual_column(start_visual_column, layout, decorations),
        width: end_visual_column.saturating_sub(start_visual_column) as f32
            * layout.metrics.character_width,
    })
}

fn caret_plan_from_projected(
    buffer: &EditorBuffer,
    viewport: &ViewportModel,
    decorations: &DecorationModel,
    selection: ProjectedSelectionLine,
    layout: EditorLayout,
) -> Option<CaretRenderPlan> {
    if selection.start != selection.end
        || selection.start_visual_column != selection.end_visual_column
    {
        return None;
    }

    caret_plan_for_position(
        buffer,
        viewport,
        decorations,
        selection.end,
        selection
            .end_virtual_column
            .or(Some(selection.end_visual_column)),
        layout,
    )
}

fn caret_plan_for_position(
    buffer: &EditorBuffer,
    viewport: &ViewportModel,
    decorations: &DecorationModel,
    position: EditorPosition,
    virtual_column: Option<usize>,
    layout: EditorLayout,
) -> Option<CaretRenderPlan> {
    let visible_row = visible_row_in_layout(viewport, position.line, layout)?;
    let line_text = buffer.line(position.line).unwrap_or("");
    let visual_column = virtual_column.filter(|column| {
        *column
            > visual_column_for(
                line_text,
                position.column,
                decorations.settings.indent_width,
            )
    });

    Some(CaretRenderPlan {
        position,
        visual_column,
        x: visual_column
            .map(|column| x_for_visual_column(column, layout, decorations))
            .unwrap_or_else(|| caret_x(line_text, position.column, layout, decorations)),
        y: row_y(visible_row, layout),
        height: layout.metrics.line_height,
    })
}

#[derive(Debug)]
pub struct SyntaxLineCache {
    settings: Option<highlighter::Settings>,
    lines: Vec<Vec<SyntaxRenderSpan>>,
    highlighter: Option<iced::highlighter::Highlighter>,
}

impl Clone for SyntaxLineCache {
    fn clone(&self) -> Self {
        Self {
            settings: self.settings.clone(),
            lines: self.lines.clone(),
            highlighter: self
                .settings
                .as_ref()
                .map(iced::highlighter::Highlighter::new),
        }
    }
}

impl Default for SyntaxLineCache {
    fn default() -> Self {
        Self {
            settings: None,
            lines: Vec::new(),
            highlighter: None,
        }
    }
}

impl PartialEq for SyntaxLineCache {
    fn eq(&self, other: &Self) -> bool {
        self.settings == other.settings && self.lines == other.lines
    }
}

impl SyntaxLineCache {
    pub fn rebuild(buffer: &EditorBuffer, settings: &highlighter::Settings) -> Self {
        if !uses_syntax_highlighting(settings) {
            return Self::default();
        }

        let mut cache = Self::new(settings);
        let lines = (0..buffer.line_count())
            .map(|line| {
                let text = buffer.line(line).unwrap_or("");
                let highlighter = cache.highlighter.as_mut().expect("syntax highlighter");
                syntax_span_plan(highlighter, text)
            })
            .collect();

        Self {
            settings: Some(settings.clone()),
            lines,
            highlighter: cache.highlighter,
        }
    }

    pub fn new(settings: &highlighter::Settings) -> Self {
        if !uses_syntax_highlighting(settings) {
            return Self::default();
        }

        Self {
            settings: Some(settings.clone()),
            lines: Vec::new(),
            highlighter: Some(iced::highlighter::Highlighter::new(settings)),
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
            self.settings.as_ref() == Some(settings) && self.highlighter.is_some()
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

        let Some(highlighter) = self.highlighter.as_mut() else {
            return;
        };

        let target_last_line = last_line.min(buffer.line_count().saturating_sub(1));
        if target_last_line < self.lines.len() {
            return;
        }

        if highlighter.current_line() != self.lines.len() {
            highlighter.change_line(self.lines.len());
            self.lines
                .truncate(highlighter.current_line().min(self.lines.len()));
        }

        while self.lines.len() <= target_last_line {
            let line = self.lines.len();
            let text = buffer.line(line).unwrap_or("");
            self.lines.push(syntax_span_plan(highlighter, text));
        }
    }

    pub fn invalidate_from(&mut self, line: usize) {
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
    }

    fn spans(&self, line: usize) -> Option<Vec<SyntaxRenderSpan>> {
        self.lines.get(line).cloned()
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

struct RenderDecorationLookups<'a> {
    decorations: &'a DecorationModel,
}

impl<'a> RenderDecorationLookups<'a> {
    fn new(decorations: &'a DecorationModel) -> Self {
        Self { decorations }
    }

    fn line_decoration(&self, line: usize) -> Option<&'a LineDecoration> {
        self.decorations.line_decorations.get(line)
    }

    fn hidden_line_span(&self, line: usize) -> Option<&'a HiddenLineSpan> {
        self.decorations
            .hidden_line_spans
            .iter()
            .find(|span| span.header_line == line)
    }

    fn indent_guides(&self, line: usize) -> impl Iterator<Item = &'a IndentGuide> {
        self.decorations
            .indent_guides
            .iter()
            .filter(move |guide| guide.line == line)
    }
}

fn whitespace_plan(
    line: usize,
    text: &str,
    decorations: &DecorationModel,
    layout: EditorLayout,
) -> Vec<WhitespaceRenderPlan> {
    if !decorations.settings.show_spaces && !decorations.settings.show_tabs {
        return Vec::new();
    }

    let mut visual_column = 0usize;

    text.char_indices()
        .filter_map(|(column, ch)| {
            let kind = match ch {
                ' ' if decorations.settings.show_spaces => WhitespaceKind::Space,
                '\t' if decorations.settings.show_tabs => WhitespaceKind::Tab,
                _ => {
                    visual_column += visual_width_with_tab_width(
                        ch,
                        visual_column,
                        decorations.settings.indent_width,
                    );
                    return None;
                }
            };
            let x = x_for_visual_column(visual_column, layout, decorations);

            visual_column +=
                visual_width_with_tab_width(ch, visual_column, decorations.settings.indent_width);

            Some(WhitespaceRenderPlan {
                line,
                column,
                x,
                kind,
            })
        })
        .collect()
}

fn eol_plan(
    line: usize,
    text: &str,
    decorations: &DecorationModel,
    layout: EditorLayout,
) -> Option<EolRenderPlan> {
    decorations
        .settings
        .show_end_of_line_markers
        .then_some(EolRenderPlan {
            line,
            x: caret_x(text, text.len(), layout, decorations),
        })
}

fn indent_guide_plan<'a>(
    line: usize,
    indent_guides: impl Iterator<Item = &'a IndentGuide>,
    decorations: &DecorationModel,
    layout: EditorLayout,
) -> Vec<IndentGuideRenderPlan> {
    if !decorations.settings.show_indentation_guides {
        return Vec::new();
    }

    indent_guides
        .map(|guide| IndentGuideRenderPlan {
            line,
            depth: guide.depth,
            x: x_for_visual_column(
                guide.depth * decorations.settings.indent_width.max(1),
                layout,
                decorations,
            ),
        })
        .collect()
}

fn visible_row_in_layout(
    viewport: &ViewportModel,
    line: usize,
    layout: EditorLayout,
) -> Option<usize> {
    let visible_row = viewport.document_line_to_visible_row(line)?;
    let first = layout.scroll.first_visible_row;
    let last = first.saturating_add(layout.visible_row_capacity());

    (first..=last).contains(&visible_row).then_some(visible_row)
}
