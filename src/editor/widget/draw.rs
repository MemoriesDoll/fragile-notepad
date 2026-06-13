use iced::advanced::{image as advanced_image, renderer, text};
use iced::{Background, Color, Font, Pixels, Point, Rectangle, Size, alignment};

use crate::editor::decoration::DecorationModel;
use crate::editor::layout::{
    EditorLayout, EditorMetrics, scrolled_text_origin_x, text_area_bounds,
};
use crate::editor::render::{
    RenderPlan, line_number_left_x, text_baseline_offset, visible_marker_columns,
};
use crate::ui::icons::hero::{self, HeroIcon};

use super::cache::RichParagraphCache;
use super::font::{EDITOR_FONT, EDITOR_TEXT_SHAPING};
use super::line_cache::{
    LineGeometryCache, RowGeometries, measured_selection_x_and_width, measured_virtual_caret_x,
};
use super::markers::draw_row_markers;
use super::rich_text::{can_batch_fast_text, draw_row_text};
use super::scrollbar::vertical_scrollbar_geometry;
use super::style::EditorStyle;

const LONG_LINE_VISIBLE_MARGIN_COLUMNS: usize = 8;

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_plan<Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    layout: EditorLayout,
    decorations: &DecorationModel,
    plan: &RenderPlan,
    style: EditorStyle,
    total_visible_rows: usize,
    fast_text: bool,
    caret_visible: bool,
    frame_id: u64,
    rich_paragraphs: &mut RichParagraphCache<Renderer::Paragraph>,
    line_geometries: &mut LineGeometryCache<Renderer::Paragraph>,
) where
    Renderer: iced::advanced::Renderer
        + text::Renderer<Font = Font>
        + advanced_image::Renderer<Handle = advanced_image::Handle>,
{
    let metrics = layout.metrics;
    let gutter_width = metrics.gutter_width(decorations);
    let text_clip_bounds = text_area_bounds(bounds, layout, decorations);
    let scroll_text_clip_bounds =
        scroll_text_area_bounds(bounds, layout, decorations, total_visible_rows);
    let batch_text = fast_text
        && plan
            .rows
            .iter()
            .all(|row| row.syntax_spans.is_empty() && can_batch_fast_text(&row.text));
    let gutter_bounds = Rectangle {
        x: bounds.x,
        y: bounds.y,
        width: gutter_width + metrics.padding_left,
        height: bounds.height,
    };

    renderer.fill_quad(
        renderer::Quad {
            bounds: gutter_bounds,
            ..renderer::Quad::default()
        },
        Background::Color(style.gutter),
    );

    for row in &plan.rows {
        let row_y = bounds.y + row.y;

        if row.is_active_line {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: bounds.x + gutter_bounds.width,
                        y: row_y,
                        width: (bounds.width - gutter_bounds.width).max(0.0),
                        height: metrics.line_height,
                    },
                    ..renderer::Quad::default()
                },
                Background::Color(style.active_line),
            );
        }
    }

    let row_geometries = RowGeometries::new(&plan.rows, layout.metrics, line_geometries, renderer);

    renderer.with_layer(text_clip_bounds, |renderer| {
        for selection in &plan.selections {
            let (x, width) = row_geometries
                .get_optional(selection.line)
                .map(|line_geometry| {
                    measured_selection_x_and_width(selection, line_geometry, layout, decorations)
                })
                .unwrap_or((selection.x, selection.width));

            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: bounds.x + x,
                        y: bounds.y + selection.y + 1.0,
                        width: width.max(1.0),
                        height: metrics.line_height - 2.0,
                    },
                    ..renderer::Quad::default()
                },
                Background::Color(style.selection),
            );
        }
    });

    renderer.with_layer(gutter_bounds, |renderer| {
        for row in &plan.rows {
            let row_y = bounds.y + row.y;

            if let Some(line_number) = row.line_number {
                draw_line_number(renderer, line_number, bounds, row_y, metrics, style);
            }
        }
    });

    for row in &plan.rows {
        let row_y = bounds.y + row.y;

        if let Some(fold) = row.fold {
            draw_fold_control(renderer, bounds, row_y, fold.collapsed, metrics, style);
        }

        if let Some(hidden_lines) = row.hidden_lines {
            draw_hidden_line_hint(
                renderer,
                bounds,
                row_y,
                hidden_lines.hidden_line_count,
                metrics,
                style,
            );
        }
    }

    renderer.with_layer(scroll_text_clip_bounds, |renderer| {
        if batch_text {
            draw_batched_row_text(
                renderer,
                bounds,
                layout,
                decorations,
                plan,
                style,
                scroll_text_clip_bounds,
            );
        } else {
            for (row_index, row) in plan.rows.iter().enumerate() {
                let row_geometry = row_geometries.get_by_row_index(row_index);

                draw_row_text(
                    renderer,
                    row,
                    bounds.x + row.text_x,
                    bounds.y + row.y + text_baseline_offset(metrics),
                    metrics,
                    decorations,
                    row_geometry,
                    style,
                    scroll_text_clip_bounds,
                    frame_id,
                    rich_paragraphs,
                    |renderer, content, position, bounds, color, align_x, metrics, clip_bounds| {
                        draw_text(
                            renderer,
                            content,
                            position,
                            bounds,
                            color,
                            align_x,
                            metrics,
                            clip_bounds,
                        );
                    },
                );
            }
        }
    });

    renderer.with_layer(text_clip_bounds, |renderer| {
        for (row_index, row) in plan.rows.iter().enumerate() {
            let row_y = bounds.y + row.y;

            for guide in &row.indent_guides {
                let x = bounds.x + guide.x;
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x,
                            y: row_y + 2.0,
                            width: 1.0,
                            height: metrics.line_height - 4.0,
                        },
                        ..renderer::Quad::default()
                    },
                    Background::Color(style.indent_guides),
                );
            }

            let row_geometry = row_geometries.get_by_row_index(row_index);

            draw_row_markers(
                renderer,
                bounds,
                layout,
                decorations,
                row,
                row_geometry,
                style,
                text_clip_bounds,
            );
        }
    });

    renderer.with_layer(text_clip_bounds, |renderer| {
        let fallback_caret = plan.caret.iter();
        let carets: Box<dyn Iterator<Item = _>> = if plan.carets.is_empty() {
            Box::new(fallback_caret)
        } else {
            Box::new(plan.carets.iter())
        };

        for caret in carets {
            let row_geometry = row_geometries.get(caret.position.line);
            let x = measured_virtual_caret_x(
                row_geometry,
                caret.position.column,
                caret.visual_column,
                layout,
                decorations,
            );
            let caret_bounds = Rectangle {
                x: bounds.x + x,
                y: bounds.y + caret.y + 1.0,
                width: 1.5,
                height: caret.height - 2.0,
            };

            renderer.fill_quad(
                renderer::Quad {
                    bounds: caret_bounds,
                    ..renderer::Quad::default()
                },
                Background::Color(if caret_visible {
                    style.caret
                } else {
                    Color::TRANSPARENT
                }),
            );
        }
    });
}

pub(super) fn draw_vertical_scrollbar<Renderer>(
    renderer: &mut Renderer,
    layout: EditorLayout,
    total_visible_rows: usize,
    bounds: Rectangle,
    style: EditorStyle,
) where
    Renderer: iced::advanced::Renderer,
{
    let Some(scrollbar) = vertical_scrollbar_geometry(layout, total_visible_rows) else {
        return;
    };
    let track = Rectangle {
        x: bounds.x + scrollbar.track.x,
        y: bounds.y + scrollbar.track.y,
        width: scrollbar.track.width,
        height: scrollbar.track.height,
    };
    let thumb = Rectangle {
        x: bounds.x + scrollbar.thumb.x,
        y: bounds.y + scrollbar.thumb.y,
        width: scrollbar.thumb.width,
        height: scrollbar.thumb.height,
    };

    renderer.fill_quad(
        renderer::Quad {
            bounds: track,
            border: iced::Border {
                color: style.line_numbers.scale_alpha(0.14),
                width: 1.0,
                radius: 3.0.into(),
            },
            ..renderer::Quad::default()
        },
        Background::Color(style.gutter.scale_alpha(0.78)),
    );
    renderer.fill_quad(
        renderer::Quad {
            bounds: thumb,
            border: iced::Border {
                color: style.line_numbers.scale_alpha(0.28),
                width: 1.0,
                radius: 3.0.into(),
            },
            ..renderer::Quad::default()
        },
        Background::Color(style.line_numbers.scale_alpha(0.42)),
    );
}

fn scroll_text_area_bounds(
    bounds: Rectangle,
    layout: EditorLayout,
    decorations: &DecorationModel,
    total_visible_rows: usize,
) -> Rectangle {
    let mut text_bounds = text_area_bounds(bounds, layout, decorations);

    if let Some(scrollbar) = vertical_scrollbar_geometry(layout, total_visible_rows) {
        let scrollbar_left = bounds.x + scrollbar.track.x;
        text_bounds.width = (scrollbar_left - text_bounds.x).max(0.0);
    }

    text_bounds
}

fn draw_line_number<Renderer>(
    renderer: &mut Renderer,
    line_number: usize,
    bounds: Rectangle,
    row_y: f32,
    metrics: EditorMetrics,
    style: EditorStyle,
) where
    Renderer: text::Renderer<Font = Font>,
{
    let content = line_number.to_string();
    let text_width = (content.len() as f32 * metrics.character_width).max(metrics.character_width);
    let x = bounds.x + line_number_left_x(metrics, text_width);

    draw_text(
        renderer,
        content,
        Point::new(x, row_y + text_baseline_offset(metrics)),
        Size::new(text_width, metrics.line_height),
        style.line_numbers,
        text::Alignment::Left,
        metrics,
        Rectangle::INFINITE,
    );
}

fn draw_batched_row_text<Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    layout: EditorLayout,
    decorations: &DecorationModel,
    plan: &RenderPlan,
    style: EditorStyle,
    clip_bounds: Rectangle,
) where
    Renderer: text::Renderer<Font = Font>,
{
    let Some(first_row) = plan.rows.first() else {
        return;
    };
    let metrics = layout.metrics;
    let text_origin_x = scrolled_text_origin_x(layout, decorations);
    let Some((first_visible_column, last_visible_column)) = visible_marker_columns(
        bounds.x + text_origin_x,
        metrics.character_width,
        clip_bounds,
    ) else {
        return;
    };
    let visible_start = first_visible_column.saturating_sub(LONG_LINE_VISIBLE_MARGIN_COLUMNS);
    let visible_end = last_visible_column.saturating_add(LONG_LINE_VISIBLE_MARGIN_COLUMNS);

    let mut content = String::new();
    let mut max_width = metrics.character_width;
    for row in &plan.rows {
        if !content.is_empty() {
            content.push('\n');
        }

        let start = visible_start.min(row.text.len());
        let end = visible_end.min(row.text.len());
        content.push_str(&row.text[start..end]);

        let width = (end - start) as f32 * metrics.character_width;
        max_width = max_width.max(width);
    }

    draw_text(
        renderer,
        content,
        Point::new(
            bounds.x + text_origin_x + visible_start as f32 * metrics.character_width,
            bounds.y + first_row.y + text_baseline_offset(metrics),
        ),
        Size::new(max_width, plan.rows.len() as f32 * metrics.line_height),
        style.syntax_fallback_text,
        text::Alignment::Left,
        metrics,
        clip_bounds,
    );
}

fn draw_fold_control<Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    row_y: f32,
    collapsed: bool,
    metrics: EditorMetrics,
    style: EditorStyle,
) where
    Renderer: iced::advanced::Renderer
        + text::Renderer<Font = Font>
        + advanced_image::Renderer<Handle = advanced_image::Handle>,
{
    let icon_size = (metrics.fold_lane_width - 5.0).max(7.0);
    let x = bounds.x + metrics.padding_left + metrics.line_number_width + 2.5;
    let y = row_y + (metrics.line_height - icon_size) / 2.0;

    renderer.fill_quad(
        renderer::Quad {
            bounds: Rectangle {
                x,
                y,
                width: icon_size,
                height: icon_size,
            },
            border: iced::Border {
                color: style.fold_controls.scale_alpha(0.55),
                width: 1.0,
                radius: 2.0.into(),
            },
            ..renderer::Quad::default()
        },
        Background::Color(style.fold_control_background),
    );

    draw_icon(
        renderer,
        if collapsed {
            HeroIcon::Plus
        } else {
            HeroIcon::Minus
        },
        Rectangle {
            x,
            y,
            width: icon_size,
            height: icon_size,
        },
        Rectangle::INFINITE,
        style.fold_controls,
    );
}

fn draw_icon<Renderer>(
    renderer: &mut Renderer,
    icon: HeroIcon,
    bounds: Rectangle,
    clip_bounds: Rectangle,
    color: Color,
) where
    Renderer: advanced_image::Renderer<Handle = advanced_image::Handle>,
{
    renderer.draw_image(
        advanced_image::Image::new(hero::handle_with_color(icon, color))
            .filter_method(advanced_image::FilterMethod::Linear),
        bounds,
        clip_bounds,
    );
}

fn draw_hidden_line_hint<Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    row_y: f32,
    hidden_line_count: usize,
    metrics: EditorMetrics,
    style: EditorStyle,
) where
    Renderer: iced::advanced::Renderer + text::Renderer<Font = Font>,
{
    let indicator_x =
        bounds.x + metrics.padding_left + metrics.line_number_width + metrics.fold_lane_width;
    let indicator_y = row_y + (metrics.line_height - 8.0) / 2.0;

    renderer.fill_quad(
        renderer::Quad {
            bounds: Rectangle {
                x: indicator_x + 2.0,
                y: indicator_y,
                width: 6.0,
                height: 8.0,
            },
            border: iced::Border {
                color: style.hidden_line_indicators.scale_alpha(0.45),
                width: 1.0,
                radius: 2.0.into(),
            },
            ..renderer::Quad::default()
        },
        Background::Color(style.hidden_line_indicators.scale_alpha(0.08)),
    );

    if hidden_line_count > 0 {
        draw_text(
            renderer,
            hidden_line_count.to_string(),
            Point::new(indicator_x + 10.0, row_y + text_baseline_offset(metrics)),
            Size::new(metrics.hidden_indicator_width + 24.0, metrics.line_height),
            style.hidden_line_indicators,
            text::Alignment::Left,
            metrics,
            Rectangle::INFINITE,
        );
    }
}

fn draw_text<Renderer>(
    renderer: &mut Renderer,
    content: impl Into<String>,
    position: Point,
    bounds: Size,
    color: Color,
    align_x: text::Alignment,
    metrics: EditorMetrics,
    clip_bounds: Rectangle,
) where
    Renderer: text::Renderer<Font = Font>,
{
    renderer.fill_text(
        text::Text {
            content: content.into(),
            bounds,
            size: Pixels((metrics.line_height / 1.25).max(8.0)),
            line_height: text::LineHeight::Absolute(Pixels(metrics.line_height)),
            font: EDITOR_FONT,
            align_x,
            align_y: alignment::Vertical::Top,
            shaping: EDITOR_TEXT_SHAPING,
            wrapping: text::Wrapping::None,
            ellipsis: text::Ellipsis::None,
            hint_factor: renderer.scale_factor(),
        },
        position,
        color,
        clip_bounds,
    );
}
