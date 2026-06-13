use iced::advanced::{InputMethod, Shell, input_method, mouse, text};
use iced::keyboard;
use iced::time::Duration;
use iced::{Event, Font, Pixels, Rectangle, window};

use crate::core::ShortcutMap;
use crate::editor::buffer::EditorBuffer;
use crate::editor::decoration::DecorationModel;
use crate::editor::layout::{EditorLayout, HitTarget, ScrollOffset, hit_test};
use crate::editor::position::{EditorPosition, EditorSelection, SelectionSet};
use crate::editor::render::text_size;
use crate::editor::viewport::ViewportModel;

use super::actions::{EditorAction, key_action};
use super::line_cache::{LineGeometry, measured_caret_x, measured_text_hit_target};
use super::scrollbar::{scrollbar_row_for_position, vertical_scrollbar_geometry};
use super::state::{AdvancedEditorState, CARET_BLINK_INTERVAL_MS};

pub(super) struct InteractionContext<'a, Message> {
    pub(super) buffer: &'a EditorBuffer,
    pub(super) viewport: &'a ViewportModel,
    pub(super) decorations: &'a DecorationModel,
    pub(super) selections: &'a SelectionSet,
    pub(super) metrics: crate::editor::layout::EditorMetrics,
    pub(super) scroll: ScrollOffset,
    pub(super) scroll_speed: f32,
    pub(super) shortcuts: &'a ShortcutMap,
    pub(super) on_action: &'a dyn Fn(EditorAction) -> Message,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct UpdateOutcome {
    pub(super) perf_event: &'static str,
    pub(super) should_capture: bool,
}

impl Default for UpdateOutcome {
    fn default() -> Self {
        Self {
            perf_event: "editor_update",
            should_capture: false,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_event<Message, Renderer>(
    context: InteractionContext<'_, Message>,
    state: &mut AdvancedEditorState<Renderer::Paragraph>,
    event: &Event,
    bounds: Rectangle,
    cursor: mouse::Cursor,
    editor_layout: EditorLayout,
    renderer: &Renderer,
    shell: &mut Shell<'_, Message>,
) -> UpdateOutcome
where
    Message: Clone,
    Renderer: iced::advanced::Renderer + text::Renderer<Font = Font>,
{
    let mut outcome = UpdateOutcome::default();

    match event {
        Event::Window(window::Event::Unfocused) => {
            state.is_window_focused = false;
            shell.request_redraw();
        }
        Event::Window(window::Event::Focused) => {
            state.is_window_focused = true;
            state.reset_caret_blink();
            shell.request_redraw();
        }
        Event::Window(window::Event::RedrawRequested(now)) => {
            state.caret_now.set(*now);
            request_caret_blink_frame(state, shell);
        }
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
            let Some(position) = cursor.position_in(bounds) else {
                state.is_focused = false;
                state.drag_anchor = None;
                state.scrollbar_grab_offset_y = None;
                state.preedit = None;
                shell.request_redraw();
                shell.request_input_method(&input_method(
                    &context,
                    state,
                    editor_layout,
                    bounds,
                    renderer,
                ));
                return outcome;
            };

            if let Some(scrollbar) =
                vertical_scrollbar_geometry(editor_layout, context.viewport.visible_row_count())
                && scrollbar.track.contains(position)
            {
                if scrollbar.thumb.contains(position) {
                    state.scrollbar_grab_offset_y = Some(position.y - scrollbar.thumb.y);
                } else {
                    let target_row = scrollbar_row_for_position(
                        position.y,
                        0.0,
                        editor_layout,
                        context.viewport.visible_row_count(),
                    );
                    shell.publish((context.on_action)(EditorAction::ScrollToRow(target_row)));
                    state.scrollbar_grab_offset_y =
                        Some((scrollbar.thumb.height / 2.0).min(position.y));
                }

                state.is_focused = true;
                state.reset_caret_blink();
                state.drag_anchor = None;
                shell.publish((context.on_action)(EditorAction::Focus));
                shell.request_redraw();
                outcome.should_capture = true;
                shell.capture_event();
                return outcome;
            }

            let hit = hit_test(
                position.x,
                position.y,
                editor_layout,
                context.buffer,
                context.viewport,
                context.decorations,
            );
            let hit = match hit {
                HitTarget::Text(_) => measured_text_hit_target(
                    position,
                    editor_layout,
                    context.buffer,
                    context.viewport,
                    context.decorations,
                    renderer,
                ),
                other => other,
            };

            match hit {
                HitTarget::FoldControl { range, .. } => {
                    state.is_focused = true;
                    state.reset_caret_blink();
                    state.clear_text_click();
                    shell.publish((context.on_action)(EditorAction::Focus));
                    shell.publish((context.on_action)(EditorAction::ToggleFold(range)));
                    shell.request_redraw();
                }
                HitTarget::Text(position) => {
                    state.is_focused = true;
                    state.reset_caret_blink();
                    let is_double_click = state.record_text_click(position, state.caret_now.get());
                    state.drag_anchor = (!is_double_click).then_some(position);
                    shell.publish((context.on_action)(EditorAction::Focus));
                    shell.publish((context.on_action)(EditorAction::PlaceCaret(position)));
                    if is_double_click {
                        shell.publish((context.on_action)(EditorAction::SelectWordAt(position)));
                    }
                    shell.request_redraw();
                }
                HitTarget::GutterLine { line } | HitTarget::HiddenLineIndicator { line } => {
                    state.is_focused = true;
                    state.reset_caret_blink();
                    state.clear_text_click();
                    shell.publish((context.on_action)(EditorAction::Focus));
                    shell.publish((context.on_action)(EditorAction::PlaceCaret(
                        EditorPosition::new(line, 0),
                    )));
                    shell.request_redraw();
                }
                HitTarget::Outside => {
                    state.is_focused = true;
                    state.reset_caret_blink();
                    state.clear_text_click();
                    state.drag_anchor = None;
                    shell.publish((context.on_action)(EditorAction::Focus));
                    shell.publish((context.on_action)(EditorAction::PlaceCaret(
                        last_line_end_position(context.buffer),
                    )));
                    shell.request_redraw();
                }
            }

            outcome.should_capture = true;
        }
        Event::Mouse(mouse::Event::CursorMoved { .. }) => {
            if let Some(grab_offset_y) = state.scrollbar_grab_offset_y {
                if let Some(position) = cursor.position_in(bounds) {
                    let target_row = scrollbar_row_for_position(
                        position.y,
                        grab_offset_y,
                        editor_layout,
                        context.viewport.visible_row_count(),
                    );
                    shell.publish((context.on_action)(EditorAction::ScrollToRow(target_row)));
                    shell.request_redraw();
                    outcome.should_capture = true;
                }
            } else if let Some(anchor) = state.drag_anchor
                && let Some(screen_position) = cursor.position()
            {
                outcome.perf_event = "editor_drag_select";
                let position = iced::Point::new(
                    screen_position.x.clamp(bounds.x, bounds.x + bounds.width),
                    screen_position.y.clamp(bounds.y, bounds.y + bounds.height),
                );
                let local_position = iced::Point::new(position.x - bounds.x, position.y - bounds.y);
                let hit = measured_text_hit_target(
                    local_position,
                    editor_layout,
                    context.buffer,
                    context.viewport,
                    context.decorations,
                    renderer,
                );

                if let HitTarget::Text(cursor) = hit {
                    state.reset_caret_blink();
                    if screen_position.y < bounds.y {
                        shell.publish((context.on_action)(EditorAction::ScrollLines(-1)));
                    } else if screen_position.y > bounds.y + bounds.height {
                        shell.publish((context.on_action)(EditorAction::ScrollLines(1)));
                    }
                    shell.publish((context.on_action)(EditorAction::SelectRegion(
                        EditorSelection::new(anchor, cursor),
                    )));
                    shell.request_redraw();
                    outcome.should_capture = true;
                }
            }
        }
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
            state.drag_anchor = None;
            state.scrollbar_grab_offset_y = None;
        }
        Event::Mouse(mouse::Event::WheelScrolled { delta }) if cursor.is_over(bounds) => {
            outcome.perf_event = "editor_wheel";
            let lines =
                scroll_delta_lines(*delta, context.scroll_speed) + state.partial_scroll_lines;
            let whole_lines = lines.trunc() as i32;
            state.partial_scroll_lines = lines.fract();

            if whole_lines != 0 {
                shell.publish((context.on_action)(EditorAction::ScrollLines(whole_lines)));
                outcome.should_capture = true;
            }
        }
        Event::InputMethod(input_method::Event::Opened) if state.is_focused => {
            state.preedit = Some(input_method::Preedit::new());
            state.reset_caret_blink();
            shell.request_redraw();
            outcome.should_capture = true;
        }
        Event::InputMethod(input_method::Event::Closed) if state.is_focused => {
            state.preedit = None;
            state.reset_caret_blink();
            shell.request_redraw();
            outcome.should_capture = true;
        }
        Event::InputMethod(input_method::Event::Preedit(content, selection))
            if state.is_focused =>
        {
            state.reset_caret_blink();
            state.preedit = Some(input_method::Preedit {
                content: content.clone(),
                selection: selection.clone(),
                text_size: Some(Pixels(text_size(context.metrics))),
            });
            shell.request_redraw();
            outcome.should_capture = true;
        }
        Event::InputMethod(input_method::Event::Commit(content)) if state.is_focused => {
            state.preedit = None;
            if !content.is_empty() {
                state.reset_caret_blink();
                shell.publish((context.on_action)(EditorAction::InsertText(
                    content.clone(),
                )));
            }
            shell.request_redraw();
            outcome.should_capture = true;
        }
        Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            modified_key,
            modifiers,
            text,
            ..
        }) if state.is_focused => {
            if let Some(action) = key_action(
                key,
                modified_key,
                *modifiers,
                text.as_deref(),
                context.shortcuts,
            ) {
                state.reset_caret_blink();
                shell.publish((context.on_action)(action));
                shell.request_redraw();
                outcome.should_capture = true;
            }
        }
        _ => {}
    }

    if outcome.should_capture {
        shell.capture_event();
    }

    shell.request_input_method(&input_method(
        &context,
        state,
        editor_layout,
        bounds,
        renderer,
    ));

    outcome
}

fn last_line_end_position(buffer: &EditorBuffer) -> EditorPosition {
    let line = buffer.line_count().saturating_sub(1);
    let column = buffer.line(line).map(str::len).unwrap_or(0);

    buffer.clamp_position(EditorPosition::new(line, column))
}

fn request_caret_blink_frame<Message, Paragraph>(
    state: &AdvancedEditorState<Paragraph>,
    shell: &mut Shell<'_, Message>,
) {
    if !state.is_focused || !state.is_window_focused {
        return;
    }

    let now = state.caret_now.get();
    let elapsed = (now - state.caret_updated_at).as_millis();
    let millis_until_redraw = CARET_BLINK_INTERVAL_MS - elapsed % CARET_BLINK_INTERVAL_MS;

    shell.request_redraw_at(now + Duration::from_millis(millis_until_redraw as u64));
}

fn input_method<'a, Message, Renderer>(
    context: &InteractionContext<'_, Message>,
    state: &'a AdvancedEditorState<Renderer::Paragraph>,
    editor_layout: EditorLayout,
    bounds: Rectangle,
    renderer: &Renderer,
) -> InputMethod<&'a str>
where
    Renderer: text::Renderer<Font = Font>,
{
    if !state.is_focused {
        return InputMethod::Disabled;
    }

    let cursor = context
        .buffer
        .clamp_position(context.selections.main().cursor);
    let visible_row = context
        .viewport
        .document_line_to_visible_row(cursor.line)
        .unwrap_or(context.scroll.first_visible_row);
    let line_text = context.buffer.line(cursor.line).unwrap_or("");
    let line_geometry = LineGeometry::new(line_text, editor_layout.metrics, renderer);
    let x = bounds.x
        + measured_caret_x(
            &line_geometry,
            cursor.column,
            editor_layout,
            context.decorations,
        );
    let y = bounds.y
        + context.metrics.padding_top
        + visible_row.saturating_sub(context.scroll.first_visible_row) as f32
            * context.metrics.line_height;

    InputMethod::Enabled {
        cursor: Rectangle {
            x,
            y,
            width: 1.0,
            height: context.metrics.line_height,
        },
        purpose: input_method::Purpose::Normal,
        preedit: state.preedit.as_ref().map(input_method::Preedit::as_ref),
    }
}

pub(super) fn scroll_delta_lines(delta: mouse::ScrollDelta, scroll_speed: f32) -> f32 {
    let lines = match delta {
        mouse::ScrollDelta::Lines { y, .. } => -y,
        mouse::ScrollDelta::Pixels { y, .. } => -y / 16.0,
    };

    lines * scroll_speed.max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_editor_area_targets_end_of_last_line() {
        let buffer = EditorBuffer::from_text("alpha\nbeta");

        assert_eq!(
            last_line_end_position(&buffer),
            EditorPosition::new(1, "beta".len())
        );
    }

    #[test]
    fn blank_editor_area_targets_empty_trailing_line() {
        let buffer = EditorBuffer::from_text("alpha\n");

        assert_eq!(last_line_end_position(&buffer), EditorPosition::new(1, 0));
    }
}
