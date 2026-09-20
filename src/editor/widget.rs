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
use super::fold::FoldRange;
#[cfg(test)]
use super::layout::scrolled_text_origin_x;
use super::layout::{
    EditorLayout, EditorMetrics, HitTarget, ScrollOffset, hit_test, hit_visible_row, row_y,
};
use super::position::{EditorPosition, SelectionSet};
#[cfg(test)]
use super::render::{
    RowRenderPlan, SelectionRenderPlan,
    build_render_plan_for_selection_set_with_cache_and_caret_row,
};
use super::render::{
    SyntaxLineCache, build_render_plan_for_selection_set_with_cache_and_caret_rows,
    collapsed_fold_indicator_bounds,
};
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
use interaction::{InteractionContext, UpdateOutcome, handle_event};
pub(crate) use line_cache::measured_position_point;
use line_cache::{LineGeometry, LineGeometryCache, measured_caret_x};
#[cfg(test)]
use line_cache::{byte_to_grapheme_table, measured_selection_x_and_width};
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
    caret_row: Option<usize>,
    caret_rows: &'a [(EditorPosition, usize)],
    scroll_speed: f32,
    viewport_key: u64,
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
            caret_row: None,
            caret_rows: &[],
            scroll_speed: 1.5,
            viewport_key: 0,
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

    pub fn viewport_key(mut self, key: u64) -> Self {
        self.viewport_key = key;
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

    pub fn caret_row(mut self, caret_row: Option<usize>) -> Self {
        self.caret_row = caret_row;
        self
    }

    pub fn caret_rows(mut self, caret_rows: &'a [(EditorPosition, usize)]) -> Self {
        self.caret_rows = caret_rows;
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
        let main_caret = self
            .caret_row
            .map(|row| (self.selections.main().cursor, row));
        let caret_rows = if self.caret_rows.is_empty() {
            main_caret.as_slice()
        } else {
            self.caret_rows
        };
        let plan = build_render_plan_for_selection_set_with_cache_and_caret_rows(
            self.buffer,
            self.viewport,
            self.decorations,
            self.selections.clone(),
            editor_layout,
            &syntax_cache,
            caret_rows,
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
        let outcome = if matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        ) && let Some(range) =
            self.hit_collapsed_indicator(layout, cursor, renderer)
        {
            state.is_focused = true;
            state.reset_caret_blink();
            state.cancel_pointer_drag();
            state.clear_text_click();
            state.preedit = None;
            shell.publish((self.on_action)(EditorAction::Focus));
            shell.publish((self.on_action)(EditorAction::ToggleFold(range)));
            shell.capture_event();
            shell.request_redraw();

            UpdateOutcome {
                perf_event: "editor_fold_toggle",
                should_capture: true,
            }
        } else {
            handle_event(
                InteractionContext {
                    buffer: self.buffer,
                    viewport: self.viewport,
                    decorations: self.decorations,
                    selections: &self.selections,
                    metrics: self.metrics,
                    caret_row: self.caret_row,
                    scroll_speed: self.scroll_speed,
                    viewport_key: self.viewport_key,
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
            )
        };

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
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();
        let editor_layout = self.editor_layout(bounds);
        let Some(position) = cursor.position_in(bounds) else {
            return mouse::Interaction::None;
        };
        let over_scrollbar =
            vertical_scrollbar_geometry(editor_layout, self.viewport.visible_row_count())
                .is_some_and(|scrollbar| scrollbar.track.contains(position));
        let over_fold_control = position.x < self.metrics.text_origin_x(self.decorations)
            && matches!(
                hit_test(
                    position.x,
                    position.y,
                    editor_layout,
                    self.buffer,
                    self.viewport,
                    self.decorations,
                ),
                HitTarget::FoldControl { .. }
            );

        if over_scrollbar
            || over_fold_control
            || self
                .hit_collapsed_indicator(layout, cursor, renderer)
                .is_some()
        {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::Text
        }
    }
}

static DEFAULT_SHORTCUTS: std::sync::LazyLock<ShortcutMap> =
    std::sync::LazyLock::new(ShortcutMap::default);

impl<'a, Message> AdvancedEditor<'a, Message> {
    fn editor_layout(&self, bounds: Rectangle) -> EditorLayout {
        let mut scroll = self.scroll;
        if self.viewport.wrap_columns().is_some() {
            scroll.horizontal_px = 0.0;
        }
        EditorLayout::new(self.metrics, scroll, bounds.width, bounds.height)
    }

    fn hit_collapsed_indicator<Renderer>(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> Option<FoldRange>
    where
        Renderer: text::Renderer<Font = Font>,
    {
        let bounds = layout.bounds();
        let position = cursor.position_in(bounds)?;
        let editor_layout = self.editor_layout(bounds);
        let text_right =
            vertical_scrollbar_geometry(editor_layout, self.viewport.visible_row_count())
                .map_or(bounds.width, |scrollbar| scrollbar.track.x);

        if position.x < self.metrics.text_origin_x(self.decorations) || position.x >= text_right {
            return None;
        }

        let (visible_row, line) = hit_visible_row(position.y, editor_layout, self.viewport)?;
        let segment = self.viewport.row_segment(visible_row, self.buffer)?;
        if !segment.is_last {
            return None;
        }
        self.decorations.line_decorations.get(line)?.fold_range?;
        // Brace and indentation folds can share a header. Match the longest
        // collapsed span, which is the range the viewport currently hides.
        let hidden = self
            .decorations
            .hidden_line_spans
            .iter()
            .filter(|span| span.header_line == line)
            .max_by_key(|span| span.last_hidden_line)?;
        let line_text = self.buffer.line(line)?;
        let fragment = &line_text[segment.start_column..segment.end_column];
        let line_geometry = LineGeometry::new_with_visual_offset(
            fragment,
            self.metrics,
            renderer,
            segment.start_visual_column,
        );
        let text_end_x = measured_caret_x(
            &line_geometry,
            fragment.len(),
            editor_layout,
            self.decorations,
        );
        let indicator = collapsed_fold_indicator_bounds(
            self.metrics,
            row_y(visible_row, editor_layout),
            text_end_x,
            self.decorations.settings.show_end_of_line_markers,
        );

        indicator
            .contains(position)
            .then_some(FoldRange::new(line, hidden.last_hidden_line))
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
    use crate::editor::{DecorationSettings, EditorPosition, EditorSelection, FoldModel};
    use iced::advanced::graphics::core::shell::Waker;
    use std::cell::Cell;

    struct FoldPointerFixture {
        buffer: EditorBuffer,
        viewport: ViewportModel,
        decorations: DecorationModel,
        syntax_cache: RefCell<SyntaxLineCache>,
    }

    impl FoldPointerFixture {
        fn new(text: &str, folds: FoldModel) -> Self {
            let buffer = EditorBuffer::from_text(text);
            let viewport = ViewportModel::new(buffer.line_count(), &folds);
            let decorations = DecorationModel::from_folds(
                DecorationSettings::default(),
                buffer.line_count(),
                &folds,
                vec![],
            );

            Self {
                buffer,
                viewport,
                decorations,
                syntax_cache: RefCell::new(SyntaxLineCache::default()),
            }
        }

        fn editor(&self) -> AdvancedEditor<'_, EditorAction> {
            AdvancedEditor::new(
                &self.buffer,
                &self.viewport,
                &self.decorations,
                &self.syntax_cache,
                highlighter::Settings {
                    token: "txt".to_owned(),
                    theme: highlighter::Theme::InspiredGitHub,
                },
                EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(0, 1)),
                std::convert::identity,
            )
        }
    }

    fn fold_test_node() -> layout::Node {
        layout::Node::new(Size::new(300.0, 94.0)).move_to(Point::new(10.0, 20.0))
    }

    fn first_fold_indicator_center(
        editor: &AdvancedEditor<'_, EditorAction>,
        node: &layout::Node,
    ) -> Point {
        let line = editor.buffer.line(0).expect("header");
        let editor_layout = editor.editor_layout(node.bounds());
        let end =
            super::super::layout::caret_x(&line, line.len(), editor_layout, editor.decorations);
        let indicator = collapsed_fold_indicator_bounds(
            editor.metrics,
            editor.metrics.padding_top,
            end,
            editor.decorations.settings.show_end_of_line_markers,
        );

        Point::new(
            node.bounds().x + indicator.center_x(),
            node.bounds().y + indicator.center_y(),
        )
    }

    fn press_fold_test_editor(
        editor: &mut AdvancedEditor<'_, EditorAction>,
        tree: &mut widget::Tree,
        node: &layout::Node,
        point: Point,
    ) -> (Vec<EditorAction>, bool) {
        let mut messages = Vec::new();
        let mut shell = Shell::new(&iced::window::Headless, Waker::noop(), &mut messages);
        Widget::<EditorAction, Theme, ()>::update(
            editor,
            tree,
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Layout::new(node),
            mouse::Cursor::Available(point),
            &(),
            &mut shell,
            &node.bounds(),
        );
        let captured = shell.is_event_captured();

        (messages, captured)
    }

    #[test]
    fn collapsed_indicator_click_expands_fold_without_changing_selection() {
        let range = FoldRange::new(0, 2);
        let mut folds = FoldModel::new(vec![range]);
        folds.set_collapsed(range, true);
        let fixture = FoldPointerFixture::new("{\n    child\n}\nafter", folds);
        let mut editor = fixture.editor();
        let original_selection = editor.selections.clone();
        let node = fold_test_node();
        let point = first_fold_indicator_center(&editor, &node);
        let mut tree = widget::Tree::new(&editor as &dyn Widget<EditorAction, Theme, ()>);
        let state = tree.state.downcast_mut::<AdvancedEditorState<()>>();
        state.drag_anchor = Some(EditorPosition::new(0, 0));
        state.drag_position = Some(point);
        state.drag_scroll_at = Some(Instant::now());
        state.scrollbar_grab_offset_y = Some(4.0);
        state.record_text_click(EditorPosition::new(0, 0), Instant::now());

        let (messages, captured) = press_fold_test_editor(&mut editor, &mut tree, &node, point);

        assert_eq!(
            messages,
            [EditorAction::Focus, EditorAction::ToggleFold(range)]
        );
        assert!(captured);
        assert_eq!(editor.selections, original_selection);
        let state = tree.state.downcast_ref::<AdvancedEditorState<()>>();
        assert!(state.is_focused);
        assert!(state.is_caret_visible());
        assert!(state.drag_anchor.is_none());
        assert!(state.drag_position.is_none());
        assert!(state.drag_scroll_at.is_none());
        assert!(state.scrollbar_grab_offset_y.is_none());
        assert!(state.last_text_click.is_none());
        assert_eq!(
            Widget::<EditorAction, Theme, ()>::mouse_interaction(
                &editor,
                &tree,
                Layout::new(&node),
                mouse::Cursor::Available(point),
                &node.bounds(),
                &(),
            ),
            mouse::Interaction::Pointer,
        );
    }

    #[test]
    fn wrapped_fold_indicator_hits_only_the_final_header_fragment() {
        let range = FoldRange::new(0, 2);
        let mut folds = FoldModel::new(vec![range]);
        folds.set_collapsed(range, true);
        let mut fixture = FoldPointerFixture::new("abcdefghij\nchild\n}\nafter", folds.clone());
        fixture.viewport = ViewportModel::new_wrapped(&fixture.buffer, &folds, 8, 4);
        let editor = fixture.editor();
        let node = fold_test_node();
        let editor_layout = editor.editor_layout(node.bounds());
        let last_row = 2;
        let indicator = collapsed_fold_indicator_bounds(
            editor.metrics,
            row_y(last_row, editor_layout),
            editor.metrics.text_origin_x(editor.decorations) + 2.0 * editor.metrics.character_width,
            false,
        );
        let last_point = indicator.center() + iced::Vector::new(node.bounds().x, node.bounds().y);
        assert_eq!(
            editor.hit_collapsed_indicator(
                Layout::new(&node),
                mouse::Cursor::Available(last_point),
                &()
            ),
            Some(range)
        );
        let first_point = Point::new(
            last_point.x,
            last_point.y - last_row as f32 * editor.metrics.line_height,
        );
        assert_eq!(
            editor.hit_collapsed_indicator(
                Layout::new(&node),
                mouse::Cursor::Available(first_point),
                &()
            ),
            None
        );
        let fold_x = editor.metrics.padding_left + editor.metrics.line_number_width + 1.0;
        assert_eq!(
            hit_test(
                fold_x,
                row_y(1, editor_layout) + 1.0,
                editor_layout,
                editor.buffer,
                editor.viewport,
                editor.decorations
            ),
            HitTarget::GutterLine { line: 0 }
        );
    }

    #[test]
    fn wrapped_fragments_render_distinct_geometry_with_software_renderer() {
        use iced::advanced::renderer::Headless;
        let mut renderer = futures::executor::block_on(<iced::Renderer as Headless>::new(
            renderer::Settings::default(),
            Some("tiny-skia"),
        ))
        .expect("software renderer");
        let buffer = EditorBuffer::from_text("éiii界WWWabcdefgh");
        let folds = FoldModel::default();
        let viewport = ViewportModel::new_wrapped(&buffer, &folds, 4, 4);
        let decorations = DecorationModel::from_folds(
            DecorationSettings {
                show_line_numbers: false,
                show_folding_controls: false,
                ..DecorationSettings::default()
            },
            buffer.line_count(),
            &folds,
            vec![],
        );
        let metrics = EditorMetrics::default();
        let bounds = Rectangle::with_size(Size::new(220.0, 140.0));
        let layout = EditorLayout::new(metrics, ScrollOffset::ZERO, bounds.width, bounds.height);
        let plan = build_render_plan_for_selection_set_with_cache_and_caret_row(
            &buffer,
            &viewport,
            &decorations,
            EditorSelection::new(
                EditorPosition::new(0, 0),
                EditorPosition::new(0, buffer.len_bytes()),
            )
            .into(),
            layout,
            &SyntaxLineCache::default(),
            None,
        );
        assert!(plan.rows.len() >= 4);
        let mut separated = plan.clone();
        for (index, row) in separated.rows.iter_mut().enumerate() {
            row.line = index;
            for selection in &mut separated.selections {
                if selection.y == row.y {
                    selection.line = index;
                }
            }
        }
        let mut render = |plan: &super::super::render::RenderPlan| {
            renderer::Renderer::reset(&mut renderer, bounds);
            draw_plan(
                &mut renderer,
                bounds,
                layout,
                &decorations,
                plan,
                EditorStyle::from_theme(&Theme::Light),
                viewport.visible_row_count(),
                false,
                false,
                1,
                &mut RichParagraphCache::default(),
                &mut LineGeometryCache::default(),
            );
            renderer.screenshot(Size::new(220, 140), 1.0, Color::WHITE)
        };
        let wrapped_pixels = render(&plan);
        let separated_pixels = render(&separated);
        assert!(
            wrapped_pixels
                .chunks_exact(4)
                .any(|pixel| pixel[..3] != [255, 255, 255])
        );
        assert_eq!(
            wrapped_pixels, separated_pixels,
            "wrapped fragments of one line must retain each row's measured geometry"
        );
    }

    #[test]
    fn expanded_or_absent_fold_keeps_normal_text_click_behavior() {
        for folds in [
            FoldModel::default(),
            FoldModel::new(vec![FoldRange::new(0, 2)]),
        ] {
            let fixture = FoldPointerFixture::new("{\n    child\n}\nafter", folds);
            let mut editor = fixture.editor();
            let node = fold_test_node();
            let point = first_fold_indicator_center(&editor, &node);
            let mut tree = widget::Tree::new(&editor as &dyn Widget<EditorAction, Theme, ()>);

            let (messages, captured) = press_fold_test_editor(&mut editor, &mut tree, &node, point);

            assert!(captured);
            assert!(messages.contains(&EditorAction::PlaceCaret(EditorPosition::new(0, 1))));
            assert!(
                !messages
                    .iter()
                    .any(|action| matches!(action, EditorAction::ToggleFold(_)))
            );
            assert_eq!(
                Widget::<EditorAction, Theme, ()>::mouse_interaction(
                    &editor,
                    &tree,
                    Layout::new(&node),
                    mouse::Cursor::Available(point),
                    &node.bounds(),
                    &(),
                ),
                mouse::Interaction::Text,
            );
        }
    }

    #[test]
    fn collapsed_indicator_hit_follows_scroll_and_excludes_gutter_and_scrollbar() {
        let range = FoldRange::new(0, 2);
        let mut folds = FoldModel::new(vec![range]);
        folds.set_collapsed(range, true);
        let fixture = FoldPointerFixture::new(
            "header\nchild\n}\nafter\nafter\nafter\nafter\nafter\nafter",
            folds,
        );
        let node = fold_test_node();
        let mut editor = fixture.editor();
        let original_point = first_fold_indicator_center(&editor, &node);
        editor.scroll.horizontal_px = 32.0;
        let scrolled_point = first_fold_indicator_center(&editor, &node);

        assert_eq!(original_point.x - scrolled_point.x, 32.0);
        assert_eq!(
            editor.hit_collapsed_indicator(
                Layout::new(&node),
                mouse::Cursor::Available(scrolled_point),
                &()
            ),
            Some(range),
        );
        assert_eq!(
            editor.hit_collapsed_indicator(
                Layout::new(&node),
                mouse::Cursor::Available(original_point),
                &()
            ),
            None,
        );

        editor.scroll.horizontal_px = 80.0;
        let hidden_in_gutter = first_fold_indicator_center(&editor, &node);
        assert!(
            hidden_in_gutter.x < node.bounds().x + editor.metrics.text_origin_x(editor.decorations)
        );
        assert_eq!(
            editor.hit_collapsed_indicator(
                Layout::new(&node),
                mouse::Cursor::Available(hidden_in_gutter),
                &()
            ),
            None,
        );

        editor.scroll.horizontal_px = 0.0;
        let narrow_node =
            layout::Node::new(Size::new(150.0, 94.0)).move_to(node.bounds().position());
        let over_scrollbar = first_fold_indicator_center(&editor, &narrow_node);
        assert!(over_scrollbar.x > narrow_node.bounds().x + 138.0);
        assert_eq!(
            editor.hit_collapsed_indicator(
                Layout::new(&narrow_node),
                mouse::Cursor::Available(over_scrollbar),
                &()
            ),
            None,
        );
    }

    #[test]
    fn collapsed_indicator_uses_outer_span_when_folds_share_a_header() {
        let inner = FoldRange::new(0, 1);
        let outer = FoldRange::new(0, 2);
        let mut folds = FoldModel::new(vec![inner, outer]);
        folds.set_all_collapsed(true);
        let mut fixture = FoldPointerFixture::new("header\nchild\n}\nafter", folds);
        fixture.decorations.settings.show_folding_controls = false;
        let editor = fixture.editor();
        let node = fold_test_node();
        let point = first_fold_indicator_center(&editor, &node);

        assert_eq!(
            editor.hit_collapsed_indicator(
                Layout::new(&node),
                mouse::Cursor::Available(point),
                &()
            ),
            Some(outer),
        );
    }

    #[test]
    fn fold_gutter_control_uses_pointer_cursor() {
        let fixture =
            FoldPointerFixture::new("{\nchild\n}", FoldModel::new(vec![FoldRange::new(0, 2)]));
        let editor = fixture.editor();
        let node = fold_test_node();
        let tree = widget::Tree::new(&editor as &dyn Widget<EditorAction, Theme, ()>);
        let point = Point::new(
            node.bounds().x + editor.metrics.padding_left + editor.metrics.line_number_width + 4.0,
            node.bounds().y + editor.metrics.padding_top + 4.0,
        );

        assert_eq!(
            Widget::<EditorAction, Theme, ()>::mouse_interaction(
                &editor,
                &tree,
                Layout::new(&node),
                mouse::Cursor::Available(point),
                &node.bounds(),
                &(),
            ),
            mouse::Interaction::Pointer,
        );
    }

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
            start_visual_column: 0,
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
            start_column: 0,
            start_visual_column: 0,
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
            start_column: 0,
            start_visual_column: 0,
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
            start_column: 0,
            start_visual_column: 0,
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
            start_column: 0,
            start_visual_column: 0,
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
    fn rich_text_visible_range_clips_tabbed_lines_to_visible_columns() {
        let row = RowRenderPlan {
            visible_row: 0,
            line: 0,
            start_column: 0,
            start_visual_column: 0,
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
            2..23
        );
    }

    #[test]
    fn rich_text_visible_range_clips_non_ascii_lines_on_utf8_boundaries() {
        let row = RowRenderPlan {
            visible_row: 0,
            line: 0,
            start_column: 0,
            start_visual_column: 0,
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
            2..23
        );
    }

    #[test]
    fn rich_text_visible_range_falls_back_for_invalid_syntax_spans() {
        let row = RowRenderPlan {
            visible_row: 0,
            line: 0,
            start_column: 0,
            start_visual_column: 0,
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
