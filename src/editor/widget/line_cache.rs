use iced::advanced::text;
use iced::{Font, Pixels, Point, Size, alignment};
use unicode_segmentation::UnicodeSegmentation;

use crate::editor::buffer::EditorBuffer;
use crate::editor::decoration::DecorationModel;
use crate::editor::layout::{
    EditorLayout, EditorMetrics, HitTarget, byte_column_for, hit_visible_row,
    scrolled_text_origin_x, visual_column_for,
};
use crate::editor::position::EditorPosition;
use crate::editor::render::{RowRenderPlan, SelectionRenderPlan};
use crate::editor::viewport::ViewportModel;

use super::font::{EDITOR_FONT, EDITOR_TEXT_SHAPING};

const LINE_GEOMETRY_CACHE_MINIMUM: usize = 256;

#[derive(Debug)]
pub(super) struct LineGeometryCache<Paragraph> {
    entries: Vec<Option<LineGeometryEntry<Paragraph>>>,
    #[cfg(test)]
    build_count: usize,
}

impl<Paragraph> Default for LineGeometryCache<Paragraph> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            #[cfg(test)]
            build_count: 0,
        }
    }
}

#[derive(Debug)]
struct LineGeometryEntry<Paragraph> {
    line: usize,
    text: String,
    metrics: EditorMetrics,
    scale_factor: Option<f32>,
    geometry: LineGeometry<Paragraph>,
}

impl<Paragraph> LineGeometryCache<Paragraph> {
    fn ensure_capacity(&mut self, visible_rows: usize) {
        let target = visible_rows
            .saturating_add(1)
            .next_power_of_two()
            .max(LINE_GEOMETRY_CACHE_MINIMUM);

        if self.entries.len() != target {
            self.entries.clear();
            self.entries.resize_with(target, || None);
        }
    }

    fn slot_for_line(&self, line: usize) -> usize {
        debug_assert!(!self.entries.is_empty());
        line % self.entries.len()
    }

    fn geometry(&self, slot: usize) -> Option<&LineGeometry<Paragraph>> {
        self.entries
            .get(slot)
            .and_then(Option::as_ref)
            .map(|entry| &entry.geometry)
    }

    #[cfg(test)]
    fn build_count(&self) -> usize {
        self.build_count
    }
}

impl<Paragraph> LineGeometryCache<Paragraph>
where
    Paragraph: text::Paragraph<Font = Font>,
{
    fn ensure<Renderer>(
        &mut self,
        line: usize,
        text: &str,
        metrics: EditorMetrics,
        renderer: &Renderer,
    ) -> usize
    where
        Renderer: text::Renderer<Font = Font, Paragraph = Paragraph>,
    {
        let scale_factor = renderer.scale_factor();
        let slot = self.slot_for_line(line);
        let is_hit = self.entries[slot].as_ref().is_some_and(|entry| {
            entry.line == line
                && entry.text == text
                && entry.metrics == metrics
                && entry.scale_factor == scale_factor
        });

        if !is_hit {
            #[cfg(test)]
            {
                self.build_count += 1;
            }

            self.entries[slot] = Some(LineGeometryEntry {
                line,
                text: text.to_owned(),
                metrics,
                scale_factor,
                geometry: LineGeometry::new(text, metrics, renderer),
            });
        }

        slot
    }
}

pub(super) fn measured_text_hit_target<Renderer>(
    position: Point,
    layout: EditorLayout,
    buffer: &EditorBuffer,
    viewport: &ViewportModel,
    decorations: &DecorationModel,
    renderer: &Renderer,
) -> HitTarget
where
    Renderer: text::Renderer<Font = Font>,
{
    let Some((_visible_row, line)) = hit_visible_row(position.y, layout, viewport) else {
        return HitTarget::Outside;
    };

    let line_text = buffer.line(line).unwrap_or("");
    let text_x = scrolled_text_origin_x(layout, decorations);
    let x = (position.x - text_x).max(0.0);
    let column = LineGeometry::new(line_text, layout.metrics, renderer)
        .byte_column_for_x(x, decorations.settings.indent_width);

    HitTarget::Text(buffer.clamp_position(EditorPosition::new(line, column)))
}

pub(super) fn measured_caret_x<Paragraph>(
    line_geometry: &LineGeometry<Paragraph>,
    column: usize,
    layout: EditorLayout,
    decorations: &DecorationModel,
) -> f32
where
    Paragraph: text::Paragraph<Font = Font>,
{
    scrolled_text_origin_x(layout, decorations)
        + line_geometry.x_for_byte_column(column, decorations.settings.indent_width)
}

pub(super) fn measured_virtual_caret_x<Paragraph>(
    line_geometry: &LineGeometry<Paragraph>,
    column: usize,
    virtual_column: Option<usize>,
    layout: EditorLayout,
    decorations: &DecorationModel,
) -> f32
where
    Paragraph: text::Paragraph<Font = Font>,
{
    virtual_column
        .map(|visual_column| {
            scrolled_text_origin_x(layout, decorations)
                + visual_column as f32 * layout.metrics.character_width
        })
        .unwrap_or_else(|| measured_caret_x(line_geometry, column, layout, decorations))
}

pub(super) fn measured_selection_x_and_width<Paragraph>(
    selection: &SelectionRenderPlan,
    line_geometry: &LineGeometry<Paragraph>,
    layout: EditorLayout,
    decorations: &DecorationModel,
) -> (f32, f32)
where
    Paragraph: text::Paragraph<Font = Font>,
{
    let start_x = measured_virtual_caret_x(
        line_geometry,
        selection.start_column,
        selection.start_virtual_column,
        layout,
        decorations,
    );
    let end_x = measured_virtual_caret_x(
        line_geometry,
        selection.end_column,
        selection.end_virtual_column,
        layout,
        decorations,
    );

    (start_x.min(end_x), (end_x - start_x).abs())
}

pub(super) struct RowGeometries<'a, Paragraph>
where
    Paragraph: text::Paragraph<Font = Font>,
{
    cache: &'a LineGeometryCache<Paragraph>,
    rows: Vec<(usize, usize)>,
}

impl<'a, Paragraph> RowGeometries<'a, Paragraph>
where
    Paragraph: text::Paragraph<Font = Font>,
{
    pub(super) fn new<Renderer>(
        rows: &[RowRenderPlan],
        metrics: EditorMetrics,
        cache: &'a mut LineGeometryCache<Paragraph>,
        renderer: &Renderer,
    ) -> Self
    where
        Renderer: text::Renderer<Font = Font, Paragraph = Paragraph>,
    {
        cache.ensure_capacity(rows.len());
        let rows = rows
            .iter()
            .map(|row| {
                (
                    row.line,
                    cache.ensure(row.line, &row.text, metrics, renderer),
                )
            })
            .collect();

        Self {
            cache: &*cache,
            rows,
        }
    }

    pub(super) fn get(&self, line: usize) -> &LineGeometry<Paragraph> {
        match self.get_optional(line) {
            Some(geometry) => geometry,
            None => self
                .rows
                .first()
                .and_then(|(_, slot)| self.cache.geometry(*slot))
                .expect("visible row geometry"),
        }
    }

    pub(super) fn get_optional(&self, line: usize) -> Option<&LineGeometry<Paragraph>> {
        self.rows
            .binary_search_by_key(&line, |(row_line, _)| *row_line)
            .ok()
            .and_then(|index| self.cache.geometry(self.rows[index].1))
    }

    pub(super) fn get_by_row_index(&self, row_index: usize) -> &LineGeometry<Paragraph> {
        let slot = self
            .rows
            .get(row_index)
            .map(|(_, slot)| *slot)
            .expect("visible row geometry index");

        self.cache.geometry(slot).expect("visible row geometry")
    }
}

#[derive(Debug)]
pub(super) enum LineGeometry<Paragraph> {
    Fast {
        text: String,
        character_width: f32,
    },
    Tabular {
        text: String,
        character_width: f32,
    },
    Measured {
        text: String,
        paragraph: Paragraph,
        byte_to_grapheme: Vec<(usize, usize)>,
        fallback_character_width: f32,
    },
}

impl<Paragraph> LineGeometry<Paragraph>
where
    Paragraph: text::Paragraph<Font = Font>,
{
    pub(super) fn new<Renderer>(text: &str, metrics: EditorMetrics, renderer: &Renderer) -> Self
    where
        Renderer: text::Renderer<Font = Font, Paragraph = Paragraph>,
    {
        if can_use_fast_geometry(text) {
            return Self::Fast {
                text: text.to_owned(),
                character_width: metrics.character_width,
            };
        }

        if text.contains('\t') {
            return Self::Tabular {
                text: text.to_owned(),
                character_width: metrics.character_width,
            };
        }

        Self::Measured {
            text: text.to_owned(),
            paragraph: Renderer::Paragraph::with_text(measure_text(
                text,
                EDITOR_FONT,
                metrics,
                renderer,
            )),
            byte_to_grapheme: byte_to_grapheme_table(text),
            fallback_character_width: metrics.character_width,
        }
    }

    pub(super) fn x_for_byte_column(&self, byte_column: usize, tab_width: usize) -> f32 {
        match self {
            Self::Fast {
                text,
                character_width,
            }
            | Self::Tabular {
                text,
                character_width,
            } => visual_column_for(text, byte_column, tab_width) as f32 * character_width,
            Self::Measured {
                text,
                paragraph,
                byte_to_grapheme,
                fallback_character_width,
            } => {
                if text.contains('\t') {
                    return fallback_x_for_byte_column(
                        text,
                        byte_column,
                        *fallback_character_width,
                        tab_width,
                    );
                }

                let byte_column = clamp_byte_boundary(text, byte_column);
                let grapheme_index = grapheme_index_for_byte(byte_to_grapheme, byte_column);

                paragraph
                    .grapheme_position(0, grapheme_index)
                    .map(|position| position.x)
                    .unwrap_or_else(|| {
                        fallback_x_for_byte_column(
                            text,
                            byte_column,
                            *fallback_character_width,
                            tab_width,
                        )
                    })
            }
        }
    }

    pub(super) fn byte_column_for_x(&self, x: f32, tab_width: usize) -> usize {
        match self {
            Self::Fast {
                text,
                character_width,
            }
            | Self::Tabular {
                text,
                character_width,
            } => {
                let visual_column = (x / character_width).floor().max(0.0) as usize;
                byte_column_for(text, visual_column, tab_width)
            }
            Self::Measured {
                text,
                paragraph,
                fallback_character_width,
                ..
            } => {
                if text.contains('\t') {
                    return fallback_byte_column_for_x(
                        text,
                        x,
                        *fallback_character_width,
                        tab_width,
                    );
                }

                paragraph
                    .hit_test(Point::new(x.max(0.0), 0.5))
                    .map(text::Hit::cursor)
                    .map(|offset| clamp_byte_boundary(text, offset))
                    .unwrap_or_else(|| {
                        fallback_byte_column_for_x(text, x, *fallback_character_width, tab_width)
                    })
            }
        }
    }

    pub(super) fn width(&self, tab_width: usize) -> f32 {
        match self {
            Self::Fast {
                text,
                character_width,
            }
            | Self::Tabular {
                text,
                character_width,
            } => visual_column_for(text, text.len(), tab_width) as f32 * character_width,
            Self::Measured {
                text, paragraph, ..
            } => paragraph
                .min_width()
                .max(self.x_for_byte_column(text.len(), tab_width)),
        }
    }
}

fn can_use_fast_geometry(text: &str) -> bool {
    text.bytes().all(|byte| byte.is_ascii())
}

fn fallback_x_for_byte_column(
    text: &str,
    byte_column: usize,
    character_width: f32,
    tab_width: usize,
) -> f32 {
    visual_column_for(text, byte_column, tab_width) as f32 * character_width
}

fn fallback_byte_column_for_x(text: &str, x: f32, character_width: f32, tab_width: usize) -> usize {
    let target = (x / character_width).floor().max(0.0) as usize;

    byte_column_for(text, target, tab_width)
}

fn measure_text<'a, Renderer>(
    content: &'a str,
    font: Font,
    metrics: EditorMetrics,
    renderer: &Renderer,
) -> text::Text<&'a str, Font>
where
    Renderer: text::Renderer<Font = Font>,
{
    text::Text {
        content,
        bounds: Size::new(f32::INFINITY, metrics.line_height),
        size: Pixels((metrics.line_height / 1.25).max(8.0)),
        line_height: text::LineHeight::Absolute(Pixels(metrics.line_height)),
        font,
        align_x: text::Alignment::Left,
        align_y: alignment::Vertical::Top,
        shaping: EDITOR_TEXT_SHAPING,
        wrapping: text::Wrapping::None,
        ellipsis: text::Ellipsis::None,
        hint_factor: renderer.scale_factor(),
    }
}

pub(super) fn byte_to_grapheme_table(text: &str) -> Vec<(usize, usize)> {
    let mut table = Vec::new();

    for (grapheme_index, (byte_offset, grapheme)) in text.grapheme_indices(true).enumerate() {
        table.push((byte_offset, grapheme_index));
        table.push((byte_offset + grapheme.len(), grapheme_index + 1));
    }

    table.sort_unstable_by_key(|(byte_offset, _)| *byte_offset);
    table.dedup_by_key(|(byte_offset, _)| *byte_offset);
    table
}

fn grapheme_index_for_byte(table: &[(usize, usize)], byte_column: usize) -> usize {
    table
        .binary_search_by_key(&byte_column, |(byte_offset, _)| *byte_offset)
        .map(|index| table[index].1)
        .unwrap_or_else(|index| {
            index
                .checked_sub(1)
                .and_then(|previous| table.get(previous))
                .map(|(_, grapheme_index)| *grapheme_index)
                .unwrap_or(0)
        })
}

fn clamp_byte_boundary(text: &str, byte_column: usize) -> usize {
    let mut column = byte_column.min(text.len());

    while column > 0 && !text.is_char_boundary(column) {
        column -= 1;
    }

    column
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_geometry_cache_reuses_page_rows_across_wheel_scroll_frames() {
        let mut cache = LineGeometryCache::<()>::default();
        cache.ensure_capacity(37);

        for frame in 0..2 {
            for line in frame..frame + 37 {
                let slot = cache.slot_for_line(line);
                let is_hit = cache.entries[slot].as_ref().is_some_and(|entry| {
                    entry.line == line
                        && entry.text == format!("line {line}")
                        && entry.metrics == EditorMetrics::default()
                        && entry.scale_factor.is_none()
                });

                if !is_hit {
                    cache.build_count += 1;
                    cache.entries[slot] = Some(LineGeometryEntry {
                        line,
                        text: format!("line {line}"),
                        metrics: EditorMetrics::default(),
                        scale_factor: None,
                        geometry: LineGeometry::Fast {
                            text: format!("line {line}"),
                            character_width: EditorMetrics::default().character_width,
                        },
                    });
                }
            }
        }

        assert_eq!(
            cache.build_count(),
            38,
            "second scroll frame should reuse 36 of 37 row geometries"
        );
    }
}
