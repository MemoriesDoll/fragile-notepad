use fragile_notepad::editor::layout::{caret_x, scrolled_text_origin_x};
use fragile_notepad::editor::render::collapsed_fold_indicator_bounds;
use fragile_notepad::editor::{
    DecorationModel, DecorationSettings, EditorBuffer, EditorLayout, EditorMetrics, EditorPosition,
    EditorSelection, FoldModel, FoldRange, RenderPlan, ScrollOffset, SyntaxLineCache,
    ViewportModel, build_render_plan_with_cache, planned_text_draws,
};
use iced::Rectangle;

fn plan_for(
    text: &str,
    collapsed: bool,
    settings: DecorationSettings,
    layout: EditorLayout,
) -> (RenderPlan, DecorationModel) {
    let buffer = EditorBuffer::from_text(text);
    let range = FoldRange::new(0, buffer.line_count() - 1);
    let mut folds = FoldModel::new(vec![range]);
    folds.set_collapsed(range, collapsed);
    let viewport = ViewportModel::new(buffer.line_count(), &folds);
    let decorations = DecorationModel::from_folds(settings, buffer.line_count(), &folds, vec![]);
    let selection = EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(0, 0));
    let plan = build_render_plan_with_cache(
        &buffer,
        &viewport,
        &decorations,
        selection,
        layout,
        &SyntaxLineCache::default(),
    );

    (plan, decorations)
}

fn default_layout() -> EditorLayout {
    EditorLayout::new(EditorMetrics::default(), ScrollOffset::ZERO, 640.0, 320.0)
}

#[test]
fn collapsed_block_has_one_inline_indicator_and_expanded_block_has_none() {
    for collapsed in [false, true] {
        let layout = default_layout();
        let (plan, _) = plan_for(
            "fn main() {\n    run();\n}",
            collapsed,
            DecorationSettings::default(),
            layout,
        );
        let indicators = plan
            .rows
            .iter()
            .filter(|row| {
                row.collapsed_indicator_bounds(layout.metrics, 180.0)
                    .is_some()
            })
            .count();

        assert_eq!(indicators, usize::from(collapsed));
        assert_eq!(planned_text_draws(&plan, true), 1);
    }
}

#[test]
fn collapsed_indicator_survives_hidden_gutter_controls() {
    let layout = default_layout();
    let (plan, _) = plan_for(
        "{\n}",
        true,
        DecorationSettings {
            show_folding_controls: false,
            ..DecorationSettings::default()
        },
        layout,
    );

    assert!(plan.rows[0].fold.is_none());
    assert!(
        plan.rows[0]
            .collapsed_indicator_bounds(layout.metrics, 100.0)
            .is_some()
    );
}

#[test]
fn collapsed_indicator_uses_measured_endpoint_and_reserves_eol_marker_cell() {
    let layout = default_layout();
    let (plan, _) = plan_for("\t字 {\n}", true, DecorationSettings::default(), layout);
    let measured_end_x = 173.25;
    let indicator = plan.rows[0]
        .collapsed_indicator_bounds(layout.metrics, measured_end_x)
        .expect("collapsed indicator");
    let with_eol =
        collapsed_fold_indicator_bounds(layout.metrics, plan.rows[0].y, measured_end_x, true);

    assert!(indicator.x > measured_end_x);
    assert!(indicator.x < measured_end_x + layout.metrics.character_width);
    assert_eq!(with_eol.x - indicator.x, layout.metrics.character_width);
    assert_eq!(with_eol.y, indicator.y);
    assert_eq!(with_eol.size(), indicator.size());
}

#[test]
fn collapsed_indicator_moves_with_horizontal_scroll_and_stays_after_long_header() {
    let unscrolled = default_layout();
    let scrolled = EditorLayout {
        scroll: ScrollOffset {
            first_visible_row: 0,
            horizontal_px: 160.0,
        },
        ..unscrolled
    };
    let header = format!("\t{} {{", "x".repeat(100));
    let (plan, decorations) = plan_for(
        &format!("{header}\n}}"),
        true,
        DecorationSettings::default(),
        unscrolled,
    );
    let row = &plan.rows[0];
    let indicator_at = |layout: EditorLayout| {
        row.collapsed_indicator_bounds(
            layout.metrics,
            caret_x(&header, header.len(), layout, &decorations),
        )
        .expect("collapsed indicator")
    };
    let before = indicator_at(unscrolled);
    let after = indicator_at(scrolled);

    assert_eq!(before.x - after.x, scrolled.scroll.horizontal_px);
    assert!(
        before.x > unscrolled.width,
        "long headers retain their true endpoint"
    );
    assert!(before.x > scrolled_text_origin_x(unscrolled, &decorations));
}

#[test]
fn collapsed_indicator_fits_inside_its_row_and_scales_with_zoom() {
    for line_height in [10.0, 18.0, 36.0, 72.0] {
        let metrics = EditorMetrics::new(line_height, line_height * 0.45);
        let row_y = 40.0;
        let indicator = collapsed_fold_indicator_bounds(metrics, row_y, 180.0, false);

        assert!(indicator.y >= row_y);
        assert!(indicator.y + indicator.height <= row_y + line_height);
        assert_eq!(indicator.center_y(), row_y + line_height / 2.0);
        assert!(indicator.width >= metrics.character_width * 2.0);
        assert!(indicator.height > 0.0);
    }
}

#[test]
fn indicator_near_viewport_edge_is_clipped_without_shifting_onto_text() {
    let metrics = EditorMetrics::default();
    let clip = Rectangle {
        x: 80.0,
        y: 0.0,
        width: 220.0,
        height: 60.0,
    };
    let partially_visible = collapsed_fold_indicator_bounds(metrics, 4.0, 280.0, false);
    let hidden = collapsed_fold_indicator_bounds(metrics, 4.0, 320.0, false);

    assert!(partially_visible.intersects(&clip));
    assert!(partially_visible.x + partially_visible.width > clip.x + clip.width);
    assert!(!hidden.intersects(&clip));
    assert!(hidden.x > 320.0);
}
