use iced::advanced::layout;
use iced::advanced::renderer;
use iced::advanced::widget::{self, Widget};
use iced::advanced::{Layout, Shell, image as advanced_image, mouse, text};
#[cfg(test)]
use iced::time::Duration;
#[cfg(test)]
use iced::time::Instant;
use iced::{Background, Element, Event, Font, Length, Rectangle, Size, Theme, highlighter};
#[cfg(test)]
use iced::{Color, Pixels, Point, alignment};
use std::cell::RefCell;
use std::time::Instant as StdInstant;

use crate::core::ShortcutMap;

use super::buffer::EditorBuffer;
use super::decoration::DecorationModel;
#[cfg(test)]
use super::layout::scrolled_text_origin_x;
use super::layout::{EditorLayout, EditorMetrics, ScrollOffset};
use super::position::SelectionSet;
#[cfg(test)]
use super::render::{RowRenderPlan, SelectionRenderPlan};
use super::render::{SyntaxLineCache, build_render_plan_for_selection_set_with_cache};
use super::viewport::ViewportModel;

mod actions;
mod cache;
mod draw;
mod font;
mod interaction;
mod line_cache;
mod markers;
mod rich_text;
mod scrollbar;
mod state;
mod style;

pub use actions::{CaretMotion, EditorAction, key_action};
#[cfg(test)]
use cache::{RichParagraphCache, SyntaxSpanKey};
use draw::{draw_plan, draw_vertical_scrollbar};
pub use font::{EDITOR_FONT, EDITOR_FONT_ROUTE, EDITOR_TEXT_SHAPING, EditorFontRoute};
#[cfg(test)]
use interaction::scroll_delta_lines;
use interaction::{InteractionContext, handle_event};
use line_cache::LineGeometryCache;
#[cfg(test)]
use line_cache::{LineGeometry, byte_to_grapheme_table, measured_selection_x_and_width};
#[cfg(test)]
use rich_text::{first_visible_syntax_span, visible_rich_text_range, visible_styled_text_range};
pub use scrollbar::{
    VerticalScrollbarGeometry, scrollbar_row_for_position, vertical_scrollbar_geometry,
};
pub use state::AdvancedEditorState;
use state::is_scroll_fast_frame;
#[cfg(test)]
use state::{CARET_BLINK_INTERVAL_MS, caret_visible_at};
pub use style::EditorStyle;

pub struct AdvancedEditor<'a, Message> {
    id: Option<widget::Id>,
    buffer: &'a EditorBuffer,
    viewport: &'a ViewportModel,
    decorations: &'a DecorationModel,
    syntax_cache: &'a RefCell<SyntaxLineCache>,
    syntax_settings: highlighter::Settings,
    selections: SelectionSet,
    metrics: EditorMetrics,
    scroll: ScrollOffset,
    scroll_speed: f32,
    shortcuts: &'a ShortcutMap,
    width: Length,
    height: Length,
    on_action: Box<dyn Fn(EditorAction) -> Message + 'a>,
}

impl<'a, Message> AdvancedEditor<'a, Message> {
    pub fn new(
        buffer: &'a EditorBuffer,
        viewport: &'a ViewportModel,
        decorations: &'a DecorationModel,
        syntax_cache: &'a RefCell<SyntaxLineCache>,
        syntax_settings: highlighter::Settings,
        selections: impl Into<SelectionSet>,
        on_action: impl Fn(EditorAction) -> Message + 'a,
    ) -> Self {
        Self {
            id: None,
            buffer,
            viewport,
            decorations,
            syntax_cache,
            syntax_settings,
            selections: selections.into(),
            metrics: EditorMetrics::default(),
            scroll: ScrollOffset::ZERO,
            scroll_speed: 1.5,
            shortcuts: &DEFAULT_SHORTCUTS,
            width: Length::Fill,
            height: Length::Fill,
            on_action: Box::new(on_action),
        }
    }

    pub fn id(mut self, id: impl Into<widget::Id>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn metrics(mut self, metrics: EditorMetrics) -> Self {
        self.metrics = metrics;
        self
    }

    pub fn scroll(mut self, scroll: ScrollOffset) -> Self {
        self.scroll = scroll;
        self
    }

    pub fn scroll_speed(mut self, scroll_speed: f32) -> Self {
        self.scroll_speed = scroll_speed.max(0.0);
        self
    }

    pub fn shortcuts(mut self, shortcuts: &'a ShortcutMap) -> Self {
        self.shortcuts = shortcuts;
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }
}

impl<Message, Renderer> Widget<Message, Theme, Renderer> for AdvancedEditor<'_, Message>
where
    Message: Clone,
    Renderer: iced::advanced::Renderer
        + text::Renderer<Font = Font>
        + advanced_image::Renderer<Handle = advanced_image::Handle>,
{
    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn layout(
        &mut self,
        _tree: &mut widget::Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.resolve(self.width, self.height, Size::new(320.0, 180.0)))
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let trace_enabled = crate::perf_trace::enabled();
        let draw_started = trace_enabled.then(StdInstant::now);
        let bounds = layout.bounds();
        let editor_layout = self.editor_layout(bounds);
        let editor_style = EditorStyle::from_theme(theme);
        let state = tree
            .state
            .downcast_ref::<AdvancedEditorState<Renderer::Paragraph>>();
        let fast_text = is_scroll_fast_frame(state);
        let caret_visible = state.is_caret_visible();
        let syntax_us = if trace_enabled {
            let syntax_started = StdInstant::now();
            prepare_visible_syntax_cache(
                self.syntax_cache,
                self.buffer,
                self.viewport,
                &self.syntax_settings,
                editor_layout,
            );
            syntax_started.elapsed().as_micros()
        } else {
            prepare_visible_syntax_cache(
                self.syntax_cache,
                self.buffer,
                self.viewport,
                &self.syntax_settings,
                editor_layout,
            );
            0
        };
        let syntax_cache = self.syntax_cache.borrow();
        let plan_started = trace_enabled.then(StdInstant::now);
        let plan = build_render_plan_for_selection_set_with_cache(
            self.buffer,
            self.viewport,
            self.decorations,
            self.selections.clone(),
            editor_layout,
            &syntax_cache,
        );
        let plan_us = plan_started.map_or(0, |started| started.elapsed().as_micros());
        let plan_rows = if trace_enabled { plan.rows.len() } else { 0 };
        let plan_spans = if trace_enabled {
            plan.rows
                .iter()
                .map(|row| row.syntax_spans.len())
                .sum::<usize>()
        } else {
            0
        };
        let selection_range_lines = if trace_enabled {
            self.selections
                .projected_lines(self.buffer, self.decorations.settings.indent_width)
                .into_iter()
                .filter(|line| !line.range().is_empty())
                .count()
        } else {
            0
        };
        let visible_selection_lines = if trace_enabled {
            plan.selections.len()
        } else {
            0
        };
        let visible_selection_area = if trace_enabled {
            let height = (editor_layout.metrics.line_height - 2.0).max(1.0);
            plan.selections
                .iter()
                .map(|selection| selection.width.max(1.0) * height)
                .sum::<f32>()
        } else {
            0.0
        };
        let visible_selection_max_width = if trace_enabled {
            plan.selections
                .iter()
                .map(|selection| selection.width.max(1.0))
                .fold(0.0, f32::max)
        } else {
            0.0
        };

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                ..renderer::Quad::default()
            },
            Background::Color(editor_style.surface),
        );

        let frame_id = state.render_frame.get().wrapping_add(1);
        state.render_frame.set(frame_id);
        let mut rich_paragraphs = state.rich_paragraphs.borrow_mut();
        let mut line_geometries = state.line_geometries.borrow_mut();
        let record_started = trace_enabled.then(StdInstant::now);
        renderer.with_layer(bounds, |renderer| {
            draw_plan(
                renderer,
                bounds,
                editor_layout,
                self.decorations,
                &plan,
                editor_style,
                self.viewport.visible_row_count(),
                fast_text,
                caret_visible,
                frame_id,
                &mut rich_paragraphs,
                &mut line_geometries,
            );
            draw_vertical_scrollbar(
                renderer,
                editor_layout,
                self.viewport.visible_row_count(),
                bounds,
                editor_style,
            );
        });
        let record_us = record_started.map_or(0, |started| started.elapsed().as_micros());
        rich_paragraphs.prune(frame_id);

        if let Some(draw_started) = draw_started {
            crate::perf_trace::event(
                "editor_draw",
                format_args!(
                    "total_us={} syntax_us={syntax_us} plan_us={plan_us} record_us={record_us} bounds={:.0}x{:.0} first_row={} rows={plan_rows} spans={plan_spans} selection_range_lines={selection_range_lines} visible_selection_lines={visible_selection_lines} visible_selection_area={visible_selection_area:.1} visible_selection_max_width={visible_selection_max_width:.1} fast_text={fast_text} token={}",
                    draw_started.elapsed().as_micros(),
                    bounds.width,
                    bounds.height,
                    editor_layout.scroll.first_visible_row,
                    self.syntax_settings.token,
                ),
            );
        }
    }

    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<AdvancedEditorState<Renderer::Paragraph>>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(AdvancedEditorState::<Renderer::Paragraph>::default())
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        let state = tree
            .state
            .downcast_mut::<AdvancedEditorState<Renderer::Paragraph>>();

        operation.focusable(self.id.as_ref(), layout.bounds(), state);
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let trace_enabled = crate::perf_trace::enabled();
        let update_started = trace_enabled.then(StdInstant::now);
        let state = tree
            .state
            .downcast_mut::<AdvancedEditorState<Renderer::Paragraph>>();
        let editor_layout = self.editor_layout(layout.bounds());
        let outcome = handle_event(
            InteractionContext {
                buffer: self.buffer,
                viewport: self.viewport,
                decorations: self.decorations,
                selections: &self.selections,
                metrics: self.metrics,
                scroll: self.scroll,
                scroll_speed: self.scroll_speed,
                shortcuts: self.shortcuts,
                on_action: &*self.on_action,
            },
            state,
            event,
            layout.bounds(),
            cursor,
            editor_layout,
            renderer,
            shell,
        );

        if let Some(update_started) = update_started {
            crate::perf_trace::event(
                outcome.perf_event,
                format_args!(
                    "elapsed_us={} capture={} first_row={} bounds={:.0}x{:.0}",
                    update_started.elapsed().as_micros(),
                    outcome.should_capture,
                    editor_layout.scroll.first_visible_row,
                    layout.bounds().width,
                    layout.bounds().height,
                ),
            );
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();
        let editor_layout = self.editor_layout(bounds);

        if let Some(position) = cursor.position_in(bounds)
            && let Some(scrollbar) =
                vertical_scrollbar_geometry(editor_layout, self.viewport.visible_row_count())
            && scrollbar.track.contains(position)
        {
            mouse::Interaction::Pointer
        } else if cursor.is_over(bounds) {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::None
        }
    }
}

static DEFAULT_SHORTCUTS: std::sync::LazyLock<ShortcutMap> =
    std::sync::LazyLock::new(ShortcutMap::default);

impl<'a, Message> AdvancedEditor<'a, Message> {
    fn editor_layout(&self, bounds: Rectangle) -> EditorLayout {
        EditorLayout::new(self.metrics, self.scroll, bounds.width, bounds.height)
    }
}

impl<'a, Message, Renderer> From<AdvancedEditor<'a, Message>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Renderer: iced::advanced::Renderer
        + text::Renderer<Font = Font>
        + advanced_image::Renderer<Handle = advanced_image::Handle>
        + 'a,
{
    fn from(editor: AdvancedEditor<'a, Message>) -> Self {
        Element::new(editor)
    }
}

fn prepare_visible_syntax_cache(
    syntax_cache: &RefCell<SyntaxLineCache>,
    buffer: &EditorBuffer,
    viewport: &ViewportModel,
    syntax_settings: &highlighter::Settings,
    layout: EditorLayout,
) {
    let first_row = layout.scroll.first_visible_row;
    let last_row = first_row.saturating_add(layout.visible_row_capacity());
    let first_line = viewport
        .visible_row_to_document_line(first_row)
        .unwrap_or(0);
    let last_line = viewport
        .visible_row_to_document_line(last_row)
        .unwrap_or_else(|| buffer.line_count().saturating_sub(1));

    syntax_cache
        .borrow_mut()
        .ensure_visible(buffer, syntax_settings, first_line, last_line);
}

#[cfg(test)]
mod tests {
    use super::super::render::SyntaxRenderSpan;
    use super::*;
    use std::cell::Cell;

    fn span_key(line: usize) -> Vec<SyntaxSpanKey> {
        vec![SyntaxSpanKey {
            start: 0,
            end: line.to_string().len(),
            color: Some(Color::from_rgb(0.8, 0.2, 0.1)),
        }]
    }

    #[test]
    fn rich_paragraph_cache_reuses_page_rows_across_wheel_scroll_frames() {
        let mut cache = RichParagraphCache::default();
        let builds = Cell::new(0usize);
        let bounds = Size::new(360.0, 18.0);
        let size = Pixels(14.0);

        for frame in 1..=2 {
            let first_line = frame - 1;
            for line in first_line..first_line + 37 {
                let text = format!("line {line}");
                let syntax_spans = span_key(line);
                cache.get_or_insert_with(
                    line,
                    &text,
                    &syntax_spans,
                    0,
                    bounds,
                    size,
                    18.0,
                    None,
                    frame as u64,
                    || {
                        let build = builds.get() + 1;
                        builds.set(build);
                        build
                    },
                );
            }
        }

        assert_eq!(
            builds.get(),
            38,
            "second scroll frame should reuse 36 of 37 shaped rows"
        );
        assert_eq!(
            cache.probe_count(),
            74,
            "cache lookup should be direct-mapped: one probe per visible row access"
        );
    }

    #[test]
    fn caret_visibility_follows_blink_interval_and_focus() {
        let updated_at = Instant::now();

        assert!(caret_visible_at(true, true, updated_at, updated_at));
        assert!(!caret_visible_at(
            true,
            true,
            updated_at,
            updated_at + Duration::from_millis(CARET_BLINK_INTERVAL_MS as u64)
        ));
        assert!(caret_visible_at(
            true,
            true,
            updated_at,
            updated_at + Duration::from_millis((CARET_BLINK_INTERVAL_MS * 2) as u64)
        ));
        assert!(!caret_visible_at(false, true, updated_at, updated_at));
        assert!(!caret_visible_at(true, false, updated_at, updated_at));
    }

    #[derive(Debug, Default)]
    struct TestParagraph {
        positions: Vec<f32>,
        min_width: f32,
    }

    impl text::Paragraph for TestParagraph {
        type Font = Font;

        fn with_text(_text: text::Text<&str, Self::Font>) -> Self {
            Self::default()
        }

        fn with_spans<Link>(
            _text: text::Text<&[text::Span<'_, Link, Self::Font>], Self::Font>,
        ) -> Self {
            Self::default()
        }

        fn resize(&mut self, _new_bounds: Size) {}

        fn compare(&self, _text: text::Text<(), Self::Font>) -> text::Difference {
            text::Difference::None
        }

        fn size(&self) -> Pixels {
            Pixels(16.0)
        }

        fn hint_factor(&self) -> Option<f32> {
            None
        }

        fn font(&self) -> Font {
            EDITOR_FONT
        }

        fn line_height(&self) -> text::LineHeight {
            text::LineHeight::default()
        }

        fn align_x(&self) -> text::Alignment {
            text::Alignment::Left
        }

        fn align_y(&self) -> alignment::Vertical {
            alignment::Vertical::Top
        }

        fn wrapping(&self) -> text::Wrapping {
            text::Wrapping::None
        }

        fn ellipsis(&self) -> text::Ellipsis {
            text::Ellipsis::None
        }

        fn shaping(&self) -> text::Shaping {
            EDITOR_TEXT_SHAPING
        }

        fn bounds(&self) -> Size {
            Size::new(f32::INFINITY, 18.0)
        }

        fn min_bounds(&self) -> Size {
            Size::new(self.min_width, 18.0)
        }

        fn hit_test(&self, _point: Point) -> Option<text::Hit> {
            None
        }

        fn hit_span(&self, _point: Point) -> Option<usize> {
            None
        }

        fn span_bounds(&self, _index: usize) -> Vec<Rectangle> {
            Vec::new()
        }

        fn grapheme_position(&self, _line: usize, index: usize) -> Option<Point> {
            self.positions.get(index).map(|x| Point::new(*x, 0.0))
        }
    }

    #[test]
    fn measured_selection_bounds_use_unicode_glyph_advances() {
        let text = "a\u{6c49}b";
        let metrics = EditorMetrics {
            character_width: 10.0,
            ..EditorMetrics::default()
        };
        let layout = EditorLayout::new(
            metrics,
            ScrollOffset {
                first_visible_row: 0,
                horizontal_px: 3.0,
            },
            400.0,
            200.0,
        );
        let decorations = DecorationModel::from_folds(
            super::super::decoration::DecorationSettings::default(),
            1,
            &super::super::fold::FoldModel::default(),
            vec![],
        );
        let selection = SelectionRenderPlan {
            line: 0,
            start_column: "a".len(),
            end_column: "a\u{6c49}".len(),
            start_visual_column: 1,
            end_visual_column: 2,
            start_virtual_column: None,
            end_virtual_column: None,
            y: 0.0,
            x: 999.0,
            width: 999.0,
        };
        let line_geometry = LineGeometry::Measured {
            text: text.to_owned(),
            paragraph: TestParagraph {
                positions: vec![0.0, 10.0, 27.0, 37.0],
                min_width: 37.0,
            },
            byte_to_grapheme: byte_to_grapheme_table(text),
            fallback_character_width: metrics.character_width,
        };

        let (x, width) =
            measured_selection_x_and_width(&selection, &line_geometry, layout, &decorations);

        assert_eq!(x, scrolled_text_origin_x(layout, &decorations) + 10.0);
        assert_eq!(width, 17.0);
    }

    #[test]
    fn wheel_line_delta_scrolls_a_little_faster_than_raw_delta() {
        assert_eq!(
            scroll_delta_lines(mouse::ScrollDelta::Lines { x: 0.0, y: -2.0 }, 1.5),
            3.0
        );
    }

    #[test]
    fn wheel_pixel_delta_keeps_fractional_scroll_accumulation() {
        assert_eq!(
            scroll_delta_lines(mouse::ScrollDelta::Pixels { x: 0.0, y: -8.0 }, 1.5),
            0.75
        );
    }

    #[test]
    fn wheel_delta_uses_configured_scroll_speed() {
        assert_eq!(
            scroll_delta_lines(mouse::ScrollDelta::Lines { x: 0.0, y: -2.0 }, 0.5),
            1.0
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
            text: format!("{}婵{}", "a".repeat(40), "b".repeat(40)),
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
