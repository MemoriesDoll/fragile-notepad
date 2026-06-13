use iced::advanced::{image as advanced_image, renderer, text};
use iced::{Background, Font, Rectangle};

use crate::editor::decoration::DecorationModel;
use crate::editor::layout::{EditorLayout, scrolled_text_origin_x, visual_column_for};
use crate::editor::render::{
    RowRenderPlan, WhitespaceKind, space_marker_bounds, visible_marker_columns,
};
use crate::ui::icons::hero::{self, HeroIcon};

use super::line_cache::{LineGeometry, measured_caret_x};
use super::style::EditorStyle;

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_row_markers<Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    layout: EditorLayout,
    decorations: &DecorationModel,
    row: &RowRenderPlan,
    line_geometry: &LineGeometry<Renderer::Paragraph>,
    style: EditorStyle,
    clip_bounds: Rectangle,
) where
    Renderer: iced::advanced::Renderer
        + text::Renderer<Font = Font>
        + advanced_image::Renderer<Handle = advanced_image::Handle>,
{
    let context = MarkerRenderContext {
        bounds,
        layout,
        decorations,
        row,
        style,
        clip_bounds,
    };

    match line_geometry {
        LineGeometry::Fast { .. } => draw_fast_row_markers(renderer, context),
        _ => draw_measured_row_markers(renderer, context, line_geometry),
    }
}

#[derive(Debug, Clone, Copy)]
struct MarkerRenderContext<'a> {
    bounds: Rectangle,
    layout: EditorLayout,
    decorations: &'a DecorationModel,
    row: &'a RowRenderPlan,
    style: EditorStyle,
    clip_bounds: Rectangle,
}

fn draw_fast_row_markers<Renderer>(renderer: &mut Renderer, context: MarkerRenderContext<'_>)
where
    Renderer: iced::advanced::Renderer + advanced_image::Renderer<Handle = advanced_image::Handle>,
{
    let MarkerRenderContext {
        bounds,
        layout,
        decorations,
        row,
        clip_bounds,
        ..
    } = context;

    if row.whitespace.is_empty() && row.eol.is_none() {
        return;
    }

    let metrics = layout.metrics;
    let text_origin_x = bounds.x + scrolled_text_origin_x(layout, decorations);
    let Some((first_visible_column, last_visible_column)) =
        visible_marker_columns(text_origin_x, metrics.character_width, clip_bounds)
    else {
        return;
    };

    for whitespace in &row.whitespace {
        let visual_column = visual_column_for(
            &row.text,
            whitespace.column,
            decorations.settings.indent_width,
        );

        if visual_column < first_visible_column || visual_column > last_visible_column {
            continue;
        }

        if whitespace.kind == WhitespaceKind::Space {
            draw_space_marker(renderer, context, visual_column);
            continue;
        }

        draw_marker_icon(renderer, context, visual_column, HeroIcon::ChevronRight);
    }

    if row.eol.is_some() {
        let visual_column =
            visual_column_for(&row.text, row.text.len(), decorations.settings.indent_width);

        if visual_column >= first_visible_column && visual_column <= last_visible_column {
            draw_marker_icon(
                renderer,
                context,
                visual_column,
                HeroIcon::ArrowTurnDownLeft,
            );
        }
    }
}

fn draw_space_marker<Renderer>(
    renderer: &mut Renderer,
    context: MarkerRenderContext<'_>,
    visual_column: usize,
) where
    Renderer: iced::advanced::Renderer,
{
    renderer.fill_quad(
        renderer::Quad {
            bounds: space_marker_bounds(
                context.bounds,
                context.layout,
                context.decorations,
                context.row,
                visual_column,
            ),
            ..renderer::Quad::default()
        },
        Background::Color(context.style.whitespace_markers),
    );
}

fn draw_measured_row_markers<Renderer>(
    renderer: &mut Renderer,
    context: MarkerRenderContext<'_>,
    line_geometry: &LineGeometry<Renderer::Paragraph>,
) where
    Renderer:
        advanced_image::Renderer<Handle = advanced_image::Handle> + text::Renderer<Font = Font>,
{
    for whitespace in &context.row.whitespace {
        let x = measured_caret_x(
            line_geometry,
            whitespace.column,
            context.layout,
            context.decorations,
        );
        match whitespace.kind {
            WhitespaceKind::Space => draw_space_marker(
                renderer,
                context,
                visual_column_for(
                    &context.row.text,
                    whitespace.column,
                    context.decorations.settings.indent_width,
                ),
            ),
            WhitespaceKind::Tab => {
                draw_marker_icon_at_x(renderer, context, x, HeroIcon::ChevronRight)
            }
        }
    }

    if context.row.eol.is_some() {
        let x = measured_caret_x(
            line_geometry,
            context.row.text.len(),
            context.layout,
            context.decorations,
        );

        draw_marker_icon_at_x(renderer, context, x, HeroIcon::ArrowTurnDownLeft);
    }
}

fn draw_marker_icon<Renderer>(
    renderer: &mut Renderer,
    context: MarkerRenderContext<'_>,
    visual_column: usize,
    icon: HeroIcon,
) where
    Renderer: advanced_image::Renderer<Handle = advanced_image::Handle>,
{
    let x = scrolled_text_origin_x(context.layout, context.decorations)
        + visual_column as f32 * context.layout.metrics.character_width;

    draw_marker_icon_at_x(renderer, context, x, icon);
}

fn draw_marker_icon_at_x<Renderer>(
    renderer: &mut Renderer,
    context: MarkerRenderContext<'_>,
    x: f32,
    icon: HeroIcon,
) where
    Renderer: advanced_image::Renderer<Handle = advanced_image::Handle>,
{
    let metrics = context.layout.metrics;
    let size = metrics
        .character_width
        .min(metrics.line_height * 0.72)
        .max(7.0);
    let bounds = Rectangle {
        x: context.bounds.x + x + (metrics.character_width - size) / 2.0,
        y: context.bounds.y + context.row.y + (metrics.line_height - size) / 2.0,
        width: size,
        height: size,
    };

    renderer.draw_image(
        advanced_image::Image::new(hero::handle_with_color(
            icon,
            context.style.whitespace_markers,
        ))
        .filter_method(advanced_image::FilterMethod::Linear),
        bounds,
        context.clip_bounds,
    );
}
