use iced::Rectangle;

use crate::editor::layout::EditorLayout;

const VERTICAL_SCROLLBAR_WIDTH: f32 = 12.0;
const VERTICAL_SCROLLBAR_MARGIN: f32 = 2.0;
const MIN_SCROLLBAR_THUMB_HEIGHT: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VerticalScrollbarGeometry {
    pub track: Rectangle,
    pub thumb: Rectangle,
}

pub fn vertical_scrollbar_geometry(
    layout: EditorLayout,
    total_visible_rows: usize,
) -> Option<VerticalScrollbarGeometry> {
    let visible_rows = layout.visible_row_capacity().max(1);

    if total_visible_rows <= visible_rows || layout.height <= 0.0 {
        return None;
    }

    let track = Rectangle {
        x: (layout.width - VERTICAL_SCROLLBAR_WIDTH).max(0.0),
        y: VERTICAL_SCROLLBAR_MARGIN,
        width: VERTICAL_SCROLLBAR_WIDTH,
        height: (layout.height - VERTICAL_SCROLLBAR_MARGIN * 2.0).max(0.0),
    };

    if track.height <= 0.0 {
        return None;
    }

    let thumb_height = (track.height * visible_rows as f32 / total_visible_rows as f32)
        .clamp(MIN_SCROLLBAR_THUMB_HEIGHT.min(track.height), track.height);
    let max_first_row = total_visible_rows.saturating_sub(visible_rows);
    let travel = (track.height - thumb_height).max(0.0);
    let progress = if max_first_row == 0 {
        0.0
    } else {
        layout.scroll.first_visible_row.min(max_first_row) as f32 / max_first_row as f32
    };
    let thumb = Rectangle {
        x: track.x + VERTICAL_SCROLLBAR_MARGIN,
        y: track.y + travel * progress,
        width: (track.width - VERTICAL_SCROLLBAR_MARGIN * 2.0).max(1.0),
        height: thumb_height,
    };

    Some(VerticalScrollbarGeometry { track, thumb })
}

pub fn scrollbar_row_for_position(
    y: f32,
    grab_offset_y: f32,
    layout: EditorLayout,
    total_visible_rows: usize,
) -> usize {
    let visible_rows = layout.visible_row_capacity().max(1);
    let max_first_row = total_visible_rows.saturating_sub(visible_rows);

    if max_first_row == 0 {
        return 0;
    }

    let Some(scrollbar) = vertical_scrollbar_geometry(layout, total_visible_rows) else {
        return 0;
    };
    let travel = (scrollbar.track.height - scrollbar.thumb.height).max(1.0);
    let top = (y - grab_offset_y).clamp(scrollbar.track.y, scrollbar.track.y + travel)
        - scrollbar.track.y;

    ((top / travel) * max_first_row as f32).round() as usize
}
