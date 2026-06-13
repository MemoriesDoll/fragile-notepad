#[cfg(not(feature = "hybrid-rendering"))]
#[rustfmt::skip]
mod tiny_skia_profile {
use fragile_notepad::editor::{
    DecorationModel, DecorationSettings, EditorBuffer, EditorLayout, EditorMetrics, EditorPosition,
    EditorSelection, FoldModel, RenderPlan, RowRenderPlan, ScrollOffset, SyntaxLineCache,
    ViewportModel, build_render_plan_with_cache, space_marker_bounds, text_baseline_offset,
    text_size, widget::EDITOR_FONT,
};
use iced::advanced::Renderer as _;
use iced::advanced::graphics::damage;
use iced::advanced::text::{Paragraph, Renderer as TextRenderer};
use iced::advanced::{renderer, text};
use iced::{Color, Font, Pixels, Point, Rectangle, Size, alignment, highlighter};
use std::collections::HashMap;
use std::hint::black_box;
use std::time::{Duration, Instant};

const LINE_COUNT: usize = 20_000;
const SAMPLES: usize = 180;
const WIDTH: u32 = 1200;
const HEIGHT: u32 = 720;
const LARGE_WIDTH: u32 = 1920;
const LARGE_HEIGHT: u32 = 1080;
const WORST_FRAME_COUNT: usize = 10;
const LONG_LINE_VISIBLE_MARGIN_COLUMNS: usize = 8;
const LONG_STYLE_RUN_SUBDIVISION_COLUMNS: usize = 100;

pub fn main() {
    let source = rust_fixture(LINE_COUNT);
    let buffer = EditorBuffer::from_text(source);
    let folds = FoldModel::default();
    let viewport_model = ViewportModel::new(buffer.line_count(), &folds);
    let decorations = DecorationModel::from_folds(
        DecorationSettings::default(),
        buffer.line_count(),
        &folds,
        vec![],
    );
    let selection = EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(0, 0));
    let metrics = EditorMetrics {
        line_height: 20.0,
        character_width: 9.0,
        ..EditorMetrics::default()
    };
    let syntax = highlighter::Settings {
        token: "rs".to_owned(),
        theme: highlighter::Theme::SolarizedDark,
    };
    let cache = SyntaxLineCache::rebuild(&buffer, &syntax);
    let rows = visible_rows(metrics);

    println!("fixture_lines={LINE_COUNT} samples={SAMPLES} visible_rows={rows}");

    measure_mode(
        "tiny_skia_rich_paragraph_rows",
        DrawMode::RichParagraphRows,
        &buffer,
        &viewport_model,
        &decorations,
        selection,
        metrics,
        &cache,
    );
    measure_mode(
        "tiny_skia_rich_page_paragraph",
        DrawMode::RichPageParagraph,
        &buffer,
        &viewport_model,
        &decorations,
        selection,
        metrics,
        &cache,
    );
    measure_mode(
        "tiny_skia_plain_paragraph_rows",
        DrawMode::PlainParagraphRows,
        &buffer,
        &viewport_model,
        &decorations,
        selection,
        metrics,
        &cache,
    );
    measure_mode(
        "tiny_skia_cached_syntax_runs",
        DrawMode::CachedSyntaxRuns,
        &buffer,
        &viewport_model,
        &decorations,
        selection,
        metrics,
        &cache,
    );
    measure_mode(
        "tiny_skia_plaintext_batched",
        DrawMode::PlainTextBatched,
        &buffer,
        &viewport_model,
        &decorations,
        selection,
        metrics,
        &cache,
    );
    measure_mode(
        "tiny_skia_rich_rows_editor_chrome",
        DrawMode::RichRowsEditorChrome,
        &buffer,
        &viewport_model,
        &decorations,
        selection,
        metrics,
        &cache,
    );
    measure_mode(
        "tiny_skia_plaintext_batched_editor_chrome",
        DrawMode::PlainTextBatchedEditorChrome,
        &buffer,
        &viewport_model,
        &decorations,
        selection,
        metrics,
        &cache,
    );
    measure_mode(
        "tiny_skia_whitespace_marker_stress",
        DrawMode::WhitespaceMarkerStress,
        &buffer,
        &viewport_model,
        &decorations,
        selection,
        metrics,
        &cache,
    );
    measure_mode(
        "tiny_skia_whitespace_marker_quads",
        DrawMode::WhitespaceMarkerQuads,
        &buffer,
        &viewport_model,
        &decorations,
        selection,
        metrics,
        &cache,
    );
    measure_mode(
        "tiny_skia_scrollbar_rounded_quads",
        DrawMode::ScrollbarRoundedQuads,
        &buffer,
        &viewport_model,
        &decorations,
        selection,
        metrics,
        &cache,
    );

    let long_source = long_line_fixture(240, 8_000);
    let long_buffer = EditorBuffer::from_text(long_source);
    let long_viewport_model = ViewportModel::new(long_buffer.line_count(), &folds);
    let long_decorations = DecorationModel::from_folds(
        DecorationSettings::default(),
        long_buffer.line_count(),
        &folds,
        vec![],
    );
    let long_cache = SyntaxLineCache::rebuild(&long_buffer, &syntax);

    measure_mode(
        "tiny_skia_long_rich_rows_clipped",
        DrawMode::ClippedRichRowsEditorChrome,
        &long_buffer,
        &long_viewport_model,
        &long_decorations,
        selection,
        metrics,
        &long_cache,
    );
    measure_mode_at_scroll(
        "tiny_skia_long_rich_rows_clipped_hscroll",
        DrawMode::ClippedRichRowsEditorChrome,
        &long_buffer,
        &long_viewport_model,
        &long_decorations,
        selection,
        metrics,
        &long_cache,
        3_000.0,
    );
    measure_mode(
        "tiny_skia_long_rich_rows_full",
        DrawMode::RichRowsEditorChrome,
        &long_buffer,
        &long_viewport_model,
        &long_decorations,
        selection,
        metrics,
        &long_cache,
    );

    let real_source = include_str!("../src/editor/widget.rs");
    let real_buffer = EditorBuffer::from_text(real_source);
    let real_viewport_model = ViewportModel::new(real_buffer.line_count(), &folds);
    let real_decorations = DecorationModel::from_folds(
        DecorationSettings::default(),
        real_buffer.line_count(),
        &folds,
        vec![],
    );
    let real_cache = SyntaxLineCache::rebuild(&real_buffer, &syntax);

    measure_real_file_full_scroll(
        "tiny_skia_widget_rs_full_scroll",
        DrawMode::ClippedRichRowsEditorChrome,
        &real_buffer,
        &real_viewport_model,
        &real_decorations,
        selection,
        metrics,
        &real_cache,
    );
    measure_real_file_damage_scroll(
        "tiny_skia_widget_rs_damage_scroll_scale1",
        DrawMode::ClippedRichRowsWidgetLayers,
        &real_buffer,
        &real_viewport_model,
        &real_decorations,
        selection,
        metrics,
        &real_cache,
        1.0,
    );
    measure_real_file_damage_scroll(
        "tiny_skia_widget_rs_damage_scroll_scale2",
        DrawMode::ClippedRichRowsWidgetLayers,
        &real_buffer,
        &real_viewport_model,
        &real_decorations,
        selection,
        metrics,
        &real_cache,
        2.0,
    );
    measure_real_file_damage_scroll_size(
        "tiny_skia_widget_rs_damage_scroll_2k_scale150",
        DrawMode::ClippedRichRowsWidgetLayers,
        &real_buffer,
        &real_viewport_model,
        &real_decorations,
        selection,
        metrics,
        &real_cache,
        2560,
        1440,
        1.5,
    );
}

#[derive(Debug, Clone, Copy)]
enum DrawMode {
    RichParagraphRows,
    RichPageParagraph,
    PlainParagraphRows,
    CachedSyntaxRuns,
    PlainTextBatched,
    RichRowsEditorChrome,
    PlainTextBatchedEditorChrome,
    WhitespaceMarkerStress,
    WhitespaceMarkerQuads,
    ScrollbarRoundedQuads,
    ClippedRichRowsEditorChrome,
    ClippedRichRowsWidgetLayers,
}

fn measure_mode(
    label: &str,
    mode: DrawMode,
    buffer: &EditorBuffer,
    viewport_model: &ViewportModel,
    decorations: &DecorationModel,
    selection: EditorSelection,
    metrics: EditorMetrics,
    cache: &SyntaxLineCache,
) {
    measure_mode_at_scroll(
        label,
        mode,
        buffer,
        viewport_model,
        decorations,
        selection,
        metrics,
        cache,
        0.0,
    );
}

fn measure_mode_at_scroll(
    label: &str,
    mode: DrawMode,
    buffer: &EditorBuffer,
    viewport_model: &ViewportModel,
    decorations: &DecorationModel,
    selection: EditorSelection,
    metrics: EditorMetrics,
    cache: &SyntaxLineCache,
    horizontal_px: f32,
) {
    let mut renderer = iced::Renderer::new(renderer::Settings::default());
    renderer.hint(1.0);
    let viewport =
        iced::advanced::graphics::Viewport::with_physical_size(Size::new(WIDTH, HEIGHT), 1.0);
    let mut pixels = tiny_skia::Pixmap::new(WIDTH, HEIGHT).expect("create pixmap");
    let mut clip_mask = tiny_skia::Mask::new(WIDTH, HEIGHT).expect("create clip mask");
    let damage = [Rectangle::new(
        Point::ORIGIN,
        Size::new(WIDTH as f32, HEIGHT as f32),
    )];
    let scroll_strip_damage = [Rectangle::new(
        Point::new(0.0, HEIGHT as f32 - metrics.line_height),
        Size::new(WIDTH as f32, metrics.line_height),
    )];
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(WIDTH as f32, HEIGHT as f32));
    let mut rich_cache: HashMap<usize, <iced::Renderer as TextRenderer>::Paragraph> =
        HashMap::new();

    for first_visible_row in 0..3 {
        draw_frame(
            &mut renderer,
            &mut rich_cache,
            mode,
            buffer,
            viewport_model,
            decorations,
            selection,
            metrics,
            cache,
            first_visible_row,
            horizontal_px,
            bounds,
        );
        renderer.draw(
            &mut pixels.as_mut(),
            &mut clip_mask,
            &viewport,
            &damage,
            Color::BLACK,
        );
    }

    let mut total_record = Duration::ZERO;
    let mut total_raster = Duration::ZERO;
    let mut total_strip_raster = Duration::ZERO;
    let mut max_record = Duration::ZERO;
    let mut max_raster = Duration::ZERO;
    let mut max_strip_raster = Duration::ZERO;

    for first_visible_row in 0..SAMPLES {
        let start = Instant::now();
        draw_frame(
            &mut renderer,
            &mut rich_cache,
            mode,
            buffer,
            viewport_model,
            decorations,
            selection,
            metrics,
            cache,
            first_visible_row,
            horizontal_px,
            bounds,
        );
        let record_elapsed = start.elapsed();
        let start = Instant::now();
        renderer.draw(
            &mut pixels.as_mut(),
            &mut clip_mask,
            &viewport,
            &damage,
            Color::BLACK,
        );
        let raster_elapsed = start.elapsed();
        let start = Instant::now();
        renderer.draw(
            &mut pixels.as_mut(),
            &mut clip_mask,
            &viewport,
            &scroll_strip_damage,
            Color::BLACK,
        );
        let strip_raster_elapsed = start.elapsed();

        total_record += record_elapsed;
        total_raster += raster_elapsed;
        total_strip_raster += strip_raster_elapsed;
        max_record = max_record.max(record_elapsed);
        max_raster = max_raster.max(raster_elapsed);
        max_strip_raster = max_strip_raster.max(strip_raster_elapsed);
    }

    let samples = SAMPLES as f64;
    let full_area = WIDTH as f32 * HEIGHT as f32;
    let strip_area = WIDTH as f32 * metrics.line_height;
    println!(
        "{label}: avg_record_us={:.2} max_record_us={:.2} avg_raster_us={:.2} max_raster_us={:.2} avg_scroll_strip_raster_us={:.2} max_scroll_strip_raster_us={:.2} scroll_strip_area_pct={:.2}",
        total_record.as_secs_f64() * 1_000_000.0 / samples,
        max_record.as_secs_f64() * 1_000_000.0,
        total_raster.as_secs_f64() * 1_000_000.0 / samples,
        max_raster.as_secs_f64() * 1_000_000.0,
        total_strip_raster.as_secs_f64() * 1_000_000.0 / samples,
        max_strip_raster.as_secs_f64() * 1_000_000.0,
        strip_area * 100.0 / full_area,
    );
}

fn measure_real_file_full_scroll(
    label: &str,
    mode: DrawMode,
    buffer: &EditorBuffer,
    viewport_model: &ViewportModel,
    decorations: &DecorationModel,
    selection: EditorSelection,
    metrics: EditorMetrics,
    cache: &SyntaxLineCache,
) {
    let mut renderer = iced::Renderer::new(renderer::Settings::default());
    renderer.hint(1.0);
    let viewport =
        iced::advanced::graphics::Viewport::with_physical_size(Size::new(WIDTH, HEIGHT), 1.0);
    let mut pixels = tiny_skia::Pixmap::new(WIDTH, HEIGHT).expect("create pixmap");
    let mut clip_mask = tiny_skia::Mask::new(WIDTH, HEIGHT).expect("create clip mask");
    let damage = [Rectangle::new(
        Point::ORIGIN,
        Size::new(WIDTH as f32, HEIGHT as f32),
    )];
    let scroll_strip_damage = [Rectangle::new(
        Point::new(0.0, HEIGHT as f32 - metrics.line_height),
        Size::new(WIDTH as f32, metrics.line_height),
    )];
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(WIDTH as f32, HEIGHT as f32));
    let mut rich_cache: HashMap<usize, <iced::Renderer as TextRenderer>::Paragraph> =
        HashMap::new();
    let visible_rows = visible_rows(metrics);
    let max_first_visible_row = viewport_model
        .visible_row_count()
        .saturating_sub(visible_rows);

    for first_visible_row in 0..=max_first_visible_row.min(3) {
        draw_frame(
            &mut renderer,
            &mut rich_cache,
            mode,
            buffer,
            viewport_model,
            decorations,
            selection,
            metrics,
            cache,
            first_visible_row,
            0.0,
            bounds,
        );
        renderer.draw(
            &mut pixels.as_mut(),
            &mut clip_mask,
            &viewport,
            &damage,
            Color::BLACK,
        );
    }

    let mut samples = Vec::with_capacity(max_first_visible_row + 1);
    let mut total_record = Duration::ZERO;
    let mut total_raster = Duration::ZERO;
    let mut total_strip_raster = Duration::ZERO;

    for first_visible_row in 0..=max_first_visible_row {
        let start = Instant::now();
        draw_frame(
            &mut renderer,
            &mut rich_cache,
            mode,
            buffer,
            viewport_model,
            decorations,
            selection,
            metrics,
            cache,
            first_visible_row,
            0.0,
            bounds,
        );
        let record = start.elapsed();
        let start = Instant::now();
        renderer.draw(
            &mut pixels.as_mut(),
            &mut clip_mask,
            &viewport,
            &damage,
            Color::BLACK,
        );
        let raster = start.elapsed();
        let start = Instant::now();
        renderer.draw(
            &mut pixels.as_mut(),
            &mut clip_mask,
            &viewport,
            &scroll_strip_damage,
            Color::BLACK,
        );
        let strip_raster = start.elapsed();

        total_record += record;
        total_raster += raster;
        total_strip_raster += strip_raster;
        samples.push(FrameSample {
            first_visible_row,
            record,
            raster,
            strip_raster,
        });
    }

    let sample_count = samples.len() as f64;
    let mut totals = samples
        .iter()
        .map(FrameSample::full_frame_total)
        .collect::<Vec<_>>();
    totals.sort_unstable();
    let p95_index = ((totals.len() * 95) / 100).min(totals.len().saturating_sub(1));
    let p99_index = ((totals.len() * 99) / 100).min(totals.len().saturating_sub(1));
    let max_total = totals.last().copied().unwrap_or(Duration::ZERO);

    samples.sort_by_key(|sample| std::cmp::Reverse(sample.full_frame_total()));

    println!(
        "{label}: file=src/editor/widget.rs lines={} scroll_offsets={} visible_rows={} avg_record_us={:.2} avg_raster_us={:.2} avg_total_us={:.2} p95_total_us={:.2} p99_total_us={:.2} max_total_us={:.2} avg_scroll_strip_raster_us={:.2}",
        buffer.line_count(),
        samples.len(),
        visible_rows,
        total_record.as_secs_f64() * 1_000_000.0 / sample_count,
        total_raster.as_secs_f64() * 1_000_000.0 / sample_count,
        (total_record + total_raster).as_secs_f64() * 1_000_000.0 / sample_count,
        totals[p95_index].as_secs_f64() * 1_000_000.0,
        totals[p99_index].as_secs_f64() * 1_000_000.0,
        max_total.as_secs_f64() * 1_000_000.0,
        total_strip_raster.as_secs_f64() * 1_000_000.0 / sample_count,
    );

    for (rank, sample) in samples.iter().take(WORST_FRAME_COUNT).enumerate() {
        let stats = frame_stats(
            buffer,
            viewport_model,
            decorations,
            selection,
            metrics,
            cache,
            sample.first_visible_row,
            0.0,
            bounds,
        );

        println!(
            "{label}_worst_frame rank={} first_visible_row={} document_lines={}..{} total_us={:.2} record_us={:.2} raster_us={:.2} strip_raster_us={:.2} rows={} spans={} visible_bytes={} full_bytes={} visible_pct={:.2} fallback_rows={} max_row_spans={} max_span_line={} max_row_len={} max_len_line={}",
            rank + 1,
            sample.first_visible_row,
            stats.first_line + 1,
            stats.last_line + 1,
            sample.full_frame_total().as_secs_f64() * 1_000_000.0,
            sample.record.as_secs_f64() * 1_000_000.0,
            sample.raster.as_secs_f64() * 1_000_000.0,
            sample.strip_raster.as_secs_f64() * 1_000_000.0,
            stats.rows,
            stats.spans,
            stats.visible_bytes,
            stats.full_bytes,
            stats.visible_percent(),
            stats.fallback_rows,
            stats.max_row_spans,
            stats.max_span_line + 1,
            stats.max_row_len,
            stats.max_len_line + 1,
        );
    }
}

fn measure_real_file_damage_scroll(
    label: &str,
    mode: DrawMode,
    buffer: &EditorBuffer,
    viewport_model: &ViewportModel,
    decorations: &DecorationModel,
    selection: EditorSelection,
    metrics: EditorMetrics,
    cache: &SyntaxLineCache,
    scale_factor: f32,
) {
    let physical_width = (LARGE_WIDTH as f32 * scale_factor).round() as u32;
    let physical_height = (LARGE_HEIGHT as f32 * scale_factor).round() as u32;

    measure_real_file_damage_scroll_size(
        label,
        mode,
        buffer,
        viewport_model,
        decorations,
        selection,
        metrics,
        cache,
        physical_width,
        physical_height,
        scale_factor,
    );
}

#[allow(clippy::too_many_arguments)]
fn measure_real_file_damage_scroll_size(
    label: &str,
    mode: DrawMode,
    buffer: &EditorBuffer,
    viewport_model: &ViewportModel,
    decorations: &DecorationModel,
    selection: EditorSelection,
    metrics: EditorMetrics,
    cache: &SyntaxLineCache,
    physical_width: u32,
    physical_height: u32,
    scale_factor: f32,
) {
    let logical_width = physical_width as f32 / scale_factor;
    let logical_height = physical_height as f32 / scale_factor;
    let viewport = iced::advanced::graphics::Viewport::with_physical_size(
        Size::new(physical_width, physical_height),
        scale_factor,
    );
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(logical_width, logical_height));
    let full_damage = [Rectangle::new(
        Point::ORIGIN,
        Size::new(logical_width, logical_height),
    )];
    let mut pixels =
        tiny_skia::Pixmap::new(physical_width, physical_height).expect("create pixmap");
    let mut clip_mask =
        tiny_skia::Mask::new(physical_width, physical_height).expect("create clip mask");
    let mut previous_renderer = iced::Renderer::new(renderer::Settings::default());
    let mut current_renderer = iced::Renderer::new(renderer::Settings::default());
    let mut rich_cache: HashMap<usize, <iced::Renderer as TextRenderer>::Paragraph> =
        HashMap::new();
    let visible_rows =
        EditorLayout::new(metrics, ScrollOffset::ZERO, logical_width, logical_height)
            .visible_row_capacity();
    let max_first_visible_row = viewport_model
        .visible_row_count()
        .saturating_sub(visible_rows);

    draw_frame(
        &mut previous_renderer,
        &mut rich_cache,
        mode,
        buffer,
        viewport_model,
        decorations,
        selection,
        metrics,
        cache,
        0,
        0.0,
        bounds,
    );
    previous_renderer.draw(
        &mut pixels.as_mut(),
        &mut clip_mask,
        &viewport,
        &full_damage,
        Color::BLACK,
    );

    let mut total_record = Duration::ZERO;
    let mut total_damage = Duration::ZERO;
    let mut total_raster = Duration::ZERO;
    let mut total_area = 0.0f32;
    let mut max_raster = Duration::ZERO;
    let mut max_area = 0.0f32;
    let mut sample_count = 0usize;

    for first_visible_row in 1..=max_first_visible_row.min(SAMPLES) {
        let start = Instant::now();
        draw_frame(
            &mut current_renderer,
            &mut rich_cache,
            mode,
            buffer,
            viewport_model,
            decorations,
            selection,
            metrics,
            cache,
            first_visible_row,
            0.0,
            bounds,
        );
        let record = start.elapsed();

        let start = Instant::now();
        let damage_regions = damage::diff(
            previous_renderer.layers(),
            current_renderer.layers(),
            |layer| vec![layer.bounds],
            |previous, current| {
                if previous.bounds != current.bounds {
                    return vec![previous.bounds, current.bounds];
                }

                let layer_bounds = current.bounds.expand(1.0);
                let mut regions = damage::list(
                    &previous.quads,
                    &current.quads,
                    |(quad, _)| {
                        quad.bounds
                            .expand(1.0)
                            .intersection(&layer_bounds)
                            .into_iter()
                            .collect()
                    },
                    |(quad_a, background_a), (quad_b, background_b)| {
                        quad_a == quad_b && background_a == background_b
                    },
                );
                let text = {
                    let previous_text = previous
                        .text
                        .iter()
                        .flat_map(|item| item.as_slice())
                        .collect::<Vec<_>>();
                    let current_text = current
                        .text
                        .iter()
                        .flat_map(|item| item.as_slice())
                        .collect::<Vec<_>>();
                    let required_matches = previous_text
                        .len()
                        .min(current_text.len())
                        .saturating_sub(1)
                        .max(2);
                    let mut scroll_delta = None;

                    'outer: for previous_start in 0..previous_text.len() {
                        for current_start in 0..current_text.len() {
                            if !same_profile_text_content(
                                previous_text[previous_start],
                                current_text[current_start],
                            ) {
                                continue;
                            }

                            let Some(previous_y) = profile_text_y(previous_text[previous_start])
                            else {
                                continue;
                            };
                            let Some(current_y) = profile_text_y(current_text[current_start])
                            else {
                                continue;
                            };
                            let delta = current_y - previous_y;

                            if delta.abs() < 0.5 {
                                continue;
                            }

                            let mut matches = 0usize;
                            while previous_start + matches < previous_text.len()
                                && current_start + matches < current_text.len()
                                && same_profile_text_content(
                                    previous_text[previous_start + matches],
                                    current_text[current_start + matches],
                                )
                                && profile_text_y(previous_text[previous_start + matches])
                                    .zip(profile_text_y(current_text[current_start + matches]))
                                    .is_some_and(|(previous_y, current_y)| {
                                        (current_y - previous_y - delta).abs() <= 0.5
                                    })
                            {
                                matches += 1;
                            }

                            if matches >= required_matches {
                                scroll_delta = Some(delta);
                                break 'outer;
                            }
                        }
                    }

                    scroll_delta
                }
                .map(|delta| {
                    if delta < 0.0 {
                        vec![Rectangle {
                            x: current.bounds.x,
                            y: current.bounds.y + current.bounds.height + delta,
                            width: current.bounds.width,
                            height: -delta,
                        }]
                    } else {
                        vec![Rectangle {
                            x: current.bounds.x,
                            y: current.bounds.y,
                            width: current.bounds.width,
                            height: delta,
                        }]
                    }
                })
                .unwrap_or_else(|| {
                    damage::diff(
                        &previous.text,
                        &current.text,
                        |item| {
                            item.as_slice()
                                .iter()
                                .filter_map(iced::advanced::graphics::text::Text::visible_bounds)
                                .map(|bounds| bounds * item.transformation())
                                .collect()
                        },
                        |previous, current| {
                            damage::list(
                                previous.as_slice(),
                                current.as_slice(),
                                |text| {
                                    text.visible_bounds()
                                        .into_iter()
                                        .map(|bounds| bounds * previous.transformation())
                                        .collect()
                                },
                                |text_a, text_b| text_a == text_b,
                            )
                        },
                    )
                });

                regions.extend(text);
                regions
            },
        );
        let damage_regions = damage::group(damage_regions, bounds);
        let damage_elapsed = start.elapsed();
        let damage_area = damage_regions.iter().map(Rectangle::area).sum::<f32>();

        let start = Instant::now();
        current_renderer.draw(
            &mut pixels.as_mut(),
            &mut clip_mask,
            &viewport,
            &damage_regions,
            Color::BLACK,
        );
        let raster = start.elapsed();

        total_record += record;
        total_damage += damage_elapsed;
        total_raster += raster;
        total_area += damage_area;
        max_raster = max_raster.max(raster);
        max_area = max_area.max(damage_area);
        sample_count += 1;

        std::mem::swap(&mut previous_renderer, &mut current_renderer);
    }

    let samples = sample_count as f64;
    let full_area = logical_width * logical_height;

    println!(
        "{label}: file=src/editor/widget.rs scale_factor={scale_factor:.2} physical={}x{} logical={}x{} visible_rows={} avg_record_us={:.2} avg_damage_us={:.2} avg_raster_us={:.2} max_raster_us={:.2} avg_damage_area_pct={:.2} max_damage_area_pct={:.2}",
        physical_width,
        physical_height,
        logical_width,
        logical_height,
        visible_rows,
        total_record.as_secs_f64() * 1_000_000.0 / samples,
        total_damage.as_secs_f64() * 1_000_000.0 / samples,
        total_raster.as_secs_f64() * 1_000_000.0 / samples,
        max_raster.as_secs_f64() * 1_000_000.0,
        total_area as f64 * 100.0 / samples / full_area as f64,
        max_area * 100.0 / full_area,
    );
}

fn profile_text_y(text: &iced::advanced::graphics::text::Text) -> Option<f32> {
    match text {
        iced::advanced::graphics::text::Text::Paragraph { position, .. } => Some(position.y),
        iced::advanced::graphics::text::Text::Cached { bounds, .. } => Some(bounds.y),
        iced::advanced::graphics::text::Text::Raw { raw, .. } => Some(raw.position.y),
        iced::advanced::graphics::text::Text::Editor { position, .. } => Some(position.y),
    }
}

fn same_profile_text_content(
    previous: &iced::advanced::graphics::text::Text,
    current: &iced::advanced::graphics::text::Text,
) -> bool {
    match (previous, current) {
        (
            iced::advanced::graphics::text::Text::Paragraph {
                paragraph: paragraph_a,
                color: color_a,
                clip_bounds: clip_bounds_a,
                transformation: transformation_a,
                ..
            },
            iced::advanced::graphics::text::Text::Paragraph {
                paragraph: paragraph_b,
                color: color_b,
                clip_bounds: clip_bounds_b,
                transformation: transformation_b,
                ..
            },
        ) => {
            paragraph_a == paragraph_b
                && color_a == color_b
                && clip_bounds_a == clip_bounds_b
                && transformation_a == transformation_b
        }
        (
            iced::advanced::graphics::text::Text::Cached {
                content: content_a,
                bounds: bounds_a,
                color: color_a,
                size: size_a,
                line_height: line_height_a,
                font: font_a,
                align_x: align_x_a,
                align_y: align_y_a,
                shaping: shaping_a,
                wrapping: wrapping_a,
                ellipsis: ellipsis_a,
                clip_bounds: clip_bounds_a,
            },
            iced::advanced::graphics::text::Text::Cached {
                content: content_b,
                bounds: bounds_b,
                color: color_b,
                size: size_b,
                line_height: line_height_b,
                font: font_b,
                align_x: align_x_b,
                align_y: align_y_b,
                shaping: shaping_b,
                wrapping: wrapping_b,
                ellipsis: ellipsis_b,
                clip_bounds: clip_bounds_b,
            },
        ) => {
            content_a == content_b
                && bounds_a.size() == bounds_b.size()
                && color_a == color_b
                && size_a == size_b
                && line_height_a == line_height_b
                && font_a == font_b
                && align_x_a == align_x_b
                && align_y_a == align_y_b
                && shaping_a == shaping_b
                && wrapping_a == wrapping_b
                && ellipsis_a == ellipsis_b
                && clip_bounds_a == clip_bounds_b
        }
        _ => false,
    }
}

#[derive(Debug, Clone, Copy)]
struct FrameSample {
    first_visible_row: usize,
    record: Duration,
    raster: Duration,
    strip_raster: Duration,
}

impl FrameSample {
    fn full_frame_total(&self) -> Duration {
        self.record + self.raster
    }
}

#[derive(Debug, Default)]
struct FrameStats {
    first_line: usize,
    last_line: usize,
    rows: usize,
    spans: usize,
    visible_bytes: usize,
    full_bytes: usize,
    fallback_rows: usize,
    max_row_spans: usize,
    max_span_line: usize,
    max_row_len: usize,
    max_len_line: usize,
}

impl FrameStats {
    fn visible_percent(&self) -> f64 {
        if self.full_bytes == 0 {
            0.0
        } else {
            self.visible_bytes as f64 * 100.0 / self.full_bytes as f64
        }
    }
}

fn frame_stats(
    buffer: &EditorBuffer,
    viewport_model: &ViewportModel,
    decorations: &DecorationModel,
    selection: EditorSelection,
    metrics: EditorMetrics,
    cache: &SyntaxLineCache,
    first_visible_row: usize,
    horizontal_px: f32,
    bounds: Rectangle,
) -> FrameStats {
    let layout = EditorLayout::new(
        metrics,
        ScrollOffset {
            first_visible_row,
            horizontal_px,
        },
        bounds.width,
        bounds.height,
    );
    let plan = build_render_plan_with_cache(
        black_box(buffer),
        black_box(viewport_model),
        black_box(decorations),
        black_box(selection),
        black_box(layout),
        black_box(cache),
    );

    let clip_bounds = Rectangle {
        x: bounds.x + metrics.text_origin_x(decorations),
        y: bounds.y,
        width: (bounds.width - metrics.text_origin_x(decorations)).max(0.0),
        height: bounds.height,
    };

    plan_stats(&plan, metrics, clip_bounds)
}

fn plan_stats(plan: &RenderPlan, metrics: EditorMetrics, bounds: Rectangle) -> FrameStats {
    let mut stats = FrameStats {
        first_line: plan.rows.first().map(|row| row.line).unwrap_or(0),
        last_line: plan.rows.last().map(|row| row.line).unwrap_or(0),
        rows: plan.rows.len(),
        ..FrameStats::default()
    };

    for row in &plan.rows {
        let visible = visible_styled_range(row, row.text_x, metrics, bounds);
        let row_spans = row.syntax_spans.len();
        let row_len = row.text.len();

        stats.spans += row_spans;
        stats.visible_bytes += visible.end.saturating_sub(visible.start);
        stats.full_bytes += row_len;

        if visible.start == 0 && visible.end == row_len && !can_clip_row_text(row, metrics) {
            stats.fallback_rows += 1;
        }

        if row_spans > stats.max_row_spans {
            stats.max_row_spans = row_spans;
            stats.max_span_line = row.line;
        }

        if row_len > stats.max_row_len {
            stats.max_row_len = row_len;
            stats.max_len_line = row.line;
        }
    }

    stats
}

#[allow(clippy::too_many_arguments)]
fn draw_frame(
    renderer: &mut iced::Renderer,
    rich_cache: &mut HashMap<usize, <iced::Renderer as TextRenderer>::Paragraph>,
    mode: DrawMode,
    buffer: &EditorBuffer,
    viewport_model: &ViewportModel,
    decorations: &DecorationModel,
    selection: EditorSelection,
    metrics: EditorMetrics,
    cache: &SyntaxLineCache,
    first_visible_row: usize,
    horizontal_px: f32,
    bounds: Rectangle,
) {
    renderer.reset(bounds);
    renderer.fill_quad(
        renderer::Quad {
            bounds,
            ..renderer::Quad::default()
        },
        Color::from_rgb(0.04, 0.04, 0.04),
    );

    let layout = EditorLayout::new(
        metrics,
        ScrollOffset {
            first_visible_row,
            horizontal_px,
        },
        bounds.width,
        bounds.height,
    );
    let plan = build_render_plan_with_cache(
        black_box(buffer),
        black_box(viewport_model),
        black_box(decorations),
        black_box(selection),
        black_box(layout),
        black_box(cache),
    );

    match mode {
        DrawMode::RichParagraphRows | DrawMode::RichRowsEditorChrome => {
            if matches!(mode, DrawMode::RichRowsEditorChrome) {
                draw_editor_chrome(renderer, &plan, metrics, decorations, bounds);
            }

            for row in &plan.rows {
                if row.syntax_spans.is_empty() {
                    draw_plain_text(
                        renderer,
                        &row.text,
                        row.text_x,
                        row.y,
                        metrics,
                        Size::new(bounds.width, metrics.line_height),
                        Color::WHITE,
                    );
                    continue;
                }

                let mut spans: Vec<text::Span<'_, (), Font>> =
                    Vec::with_capacity(row.syntax_spans.len());
                for syntax_span in &row.syntax_spans {
                    if syntax_span.range.end <= row.text.len() {
                        spans.push(
                            text::Span::new(&row.text[syntax_span.range.clone()])
                                .color_maybe(syntax_span.color),
                        );
                    }
                }

                let paragraph = rich_cache.entry(row.line).or_insert_with(|| {
                    <iced::Renderer as TextRenderer>::Paragraph::with_spans(text::Text {
                        content: spans.as_slice(),
                        bounds: Size::new(bounds.width, metrics.line_height),
                        size: Pixels(text_size(metrics)),
                        line_height: text::LineHeight::Absolute(Pixels(metrics.line_height)),
                        font: EDITOR_FONT,
                        align_x: text::Alignment::Left,
                        align_y: alignment::Vertical::Top,
                        shaping: text::Shaping::Auto,
                        wrapping: text::Wrapping::None,
                        ellipsis: text::Ellipsis::None,
                        hint_factor: renderer.scale_factor(),
                    })
                });
                renderer.fill_paragraph(
                    paragraph,
                    Point::new(row.text_x, row.y + text_baseline_offset(metrics)),
                    Color::WHITE,
                    bounds,
                );
            }
        }
        DrawMode::ClippedRichRowsEditorChrome => {
            draw_editor_chrome(renderer, &plan, metrics, decorations, bounds);

            for row in &plan.rows {
                draw_clipped_rich_row(renderer, rich_cache, row, metrics, bounds);
            }
        }
        DrawMode::ClippedRichRowsWidgetLayers => {
            let text_clip_bounds = Rectangle {
                x: metrics.text_origin_x(decorations),
                y: bounds.y,
                width: (bounds.width - metrics.text_origin_x(decorations)).max(0.0),
                height: bounds.height,
            };

            draw_editor_chrome(renderer, &plan, metrics, decorations, bounds);

            renderer.with_layer(text_clip_bounds, |renderer| {
                for row in &plan.rows {
                    draw_clipped_rich_row(renderer, rich_cache, row, metrics, bounds);
                }
            });
        }
        DrawMode::RichPageParagraph => {
            let mut spans: Vec<text::Span<'_, (), Font>> = Vec::new();

            for (row_index, row) in plan.rows.iter().enumerate() {
                if row.syntax_spans.is_empty() {
                    spans.push(text::Span::new(row.text.as_str()).color(Color::WHITE));
                } else {
                    for syntax_span in &row.syntax_spans {
                        if syntax_span.range.end <= row.text.len() {
                            spans.push(
                                text::Span::new(&row.text[syntax_span.range.clone()])
                                    .color_maybe(syntax_span.color),
                            );
                        }
                    }
                }

                if row_index + 1 < plan.rows.len() {
                    spans.push(text::Span::new("\n").color(Color::WHITE));
                }
            }

            let paragraph = <iced::Renderer as TextRenderer>::Paragraph::with_spans(text::Text {
                content: spans.as_slice(),
                bounds: Size::new(bounds.width, plan.rows.len() as f32 * metrics.line_height),
                size: Pixels(text_size(metrics)),
                line_height: text::LineHeight::Absolute(Pixels(metrics.line_height)),
                font: EDITOR_FONT,
                align_x: text::Alignment::Left,
                align_y: alignment::Vertical::Top,
                shaping: text::Shaping::Auto,
                wrapping: text::Wrapping::None,
                ellipsis: text::Ellipsis::None,
                hint_factor: renderer.scale_factor(),
            });
            renderer.fill_paragraph(
                &paragraph,
                Point::new(
                    metrics.text_origin_x(decorations),
                    plan.rows.first().map(|row| row.y).unwrap_or(0.0)
                        + text_baseline_offset(metrics),
                ),
                Color::WHITE,
                bounds,
            );
        }
        DrawMode::PlainParagraphRows => {
            for row in &plan.rows {
                let paragraph = rich_cache.entry(row.line).or_insert_with(|| {
                    <iced::Renderer as TextRenderer>::Paragraph::with_text(text::Text {
                        content: row.text.as_str(),
                        bounds: Size::new(bounds.width, metrics.line_height),
                        size: Pixels(text_size(metrics)),
                        line_height: text::LineHeight::Absolute(Pixels(metrics.line_height)),
                        font: EDITOR_FONT,
                        align_x: text::Alignment::Left,
                        align_y: alignment::Vertical::Top,
                        shaping: text::Shaping::Auto,
                        wrapping: text::Wrapping::None,
                        ellipsis: text::Ellipsis::None,
                        hint_factor: renderer.scale_factor(),
                    })
                });
                renderer.fill_paragraph(
                    paragraph,
                    Point::new(row.text_x, row.y + text_baseline_offset(metrics)),
                    Color::WHITE,
                    bounds,
                );
            }
        }
        DrawMode::CachedSyntaxRuns => {
            for row in &plan.rows {
                for syntax_span in &row.syntax_spans {
                    if syntax_span.range.end <= row.text.len() {
                        let x = row.text_x
                            + row.text[..syntax_span.range.start].chars().count() as f32
                                * metrics.character_width;
                        draw_plain_text(
                            renderer,
                            &row.text[syntax_span.range.clone()],
                            x,
                            row.y,
                            metrics,
                            Size::new(bounds.width, metrics.line_height),
                            syntax_span.color.unwrap_or(Color::WHITE),
                        );
                    }
                }
            }
        }
        DrawMode::PlainTextBatched | DrawMode::PlainTextBatchedEditorChrome => {
            if matches!(mode, DrawMode::PlainTextBatchedEditorChrome) {
                draw_editor_chrome(renderer, &plan, metrics, decorations, bounds);
            }

            let mut content = String::new();
            for row in &plan.rows {
                if !content.is_empty() {
                    content.push('\n');
                }
                content.push_str(&row.text);
            }
            draw_plain_text(
                renderer,
                content,
                metrics.text_origin_x(decorations),
                plan.rows.first().map(|row| row.y).unwrap_or(0.0),
                metrics,
                Size::new(bounds.width, plan.rows.len() as f32 * metrics.line_height),
                Color::WHITE,
            );
        }
        DrawMode::WhitespaceMarkerStress => {
            draw_editor_chrome(renderer, &plan, metrics, decorations, bounds);
            draw_whitespace_marker_stress(renderer, &plan, metrics);
        }
        DrawMode::WhitespaceMarkerQuads => {
            draw_editor_chrome(renderer, &plan, metrics, decorations, bounds);
            draw_whitespace_marker_quads(renderer, &plan, metrics, decorations, bounds);
        }
        DrawMode::ScrollbarRoundedQuads => {
            draw_scrollbar_stress(renderer, bounds);
        }
    }
}

fn draw_clipped_rich_row(
    renderer: &mut iced::Renderer,
    rich_cache: &mut HashMap<usize, <iced::Renderer as TextRenderer>::Paragraph>,
    row: &RowRenderPlan,
    metrics: EditorMetrics,
    bounds: Rectangle,
) {
    let visible = visible_styled_range(row, row.text_x, metrics, bounds);
    let visible_text = &row.text[visible.clone()];
    let mut spans: Vec<text::Span<'_, (), Font>> = Vec::new();
    let mut cursor = visible.start;
    let first_span = row
        .syntax_spans
        .partition_point(|span| span.range.end <= visible.start);

    for syntax_span in &row.syntax_spans[first_span..] {
        if syntax_span.range.start >= visible.end {
            break;
        }

        let start = syntax_span.range.start.max(visible.start);
        let end = syntax_span.range.end.min(visible.end);

        if start >= end {
            continue;
        }

        if cursor < start {
            spans.push(text::Span::new(&row.text[cursor..start]).color(Color::WHITE));
        }

        spans.push(text::Span::new(&row.text[start..end]).color_maybe(syntax_span.color));
        cursor = end;
    }

    if cursor < visible.end {
        spans.push(text::Span::new(&row.text[cursor..visible.end]).color(Color::WHITE));
    }

    if spans.is_empty() {
        spans.push(text::Span::new(visible_text).color(Color::WHITE));
    }

    let cache_key = row
        .line
        .wrapping_mul(131_071)
        .wrapping_add(visible.start.wrapping_mul(257))
        .wrapping_add(visible.end);
    let paragraph = rich_cache.entry(cache_key).or_insert_with(|| {
        <iced::Renderer as TextRenderer>::Paragraph::with_spans(text::Text {
            content: spans.as_slice(),
            bounds: Size::new(
                visible_text.len() as f32 * metrics.character_width,
                metrics.line_height,
            ),
            size: Pixels(text_size(metrics)),
            line_height: text::LineHeight::Absolute(Pixels(metrics.line_height)),
            font: EDITOR_FONT,
            align_x: text::Alignment::Left,
            align_y: alignment::Vertical::Top,
            shaping: text::Shaping::Auto,
            wrapping: text::Wrapping::None,
            ellipsis: text::Ellipsis::None,
            hint_factor: renderer.scale_factor(),
        })
    });

    renderer.fill_paragraph(
        paragraph,
        Point::new(
            row.text_x + visible.start as f32 * metrics.character_width,
            row.y + text_baseline_offset(metrics),
        ),
        Color::WHITE,
        bounds,
    );
}

fn visible_styled_range(
    row: &RowRenderPlan,
    text_origin_x: f32,
    metrics: EditorMetrics,
    bounds: Rectangle,
) -> std::ops::Range<usize> {
    if !can_clip_row_text(row, metrics) {
        return 0..row.text.len();
    }

    let first = ((bounds.x - text_origin_x) / metrics.character_width)
        .floor()
        .max(0.0) as usize;
    let last = ((bounds.x + bounds.width - text_origin_x) / metrics.character_width)
        .ceil()
        .max(0.0) as usize;
    let start = first.saturating_sub(LONG_LINE_VISIBLE_MARGIN_COLUMNS);
    let end = last
        .saturating_add(LONG_LINE_VISIBLE_MARGIN_COLUMNS)
        .min(row.text.len());
    let start = style_boundary_start(row, start.min(row.text.len()));

    start.min(row.text.len())..end.max(start).min(row.text.len())
}

fn can_clip_row_text(row: &RowRenderPlan, metrics: EditorMetrics) -> bool {
    let mut previous_end = 0;

    metrics.character_width > 0.0
        && !row.text.is_empty()
        && row
            .text
            .bytes()
            .all(|byte| byte.is_ascii() && byte != b'\t')
        && row.syntax_spans.iter().all(|span| {
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

fn draw_editor_chrome(
    renderer: &mut iced::Renderer,
    plan: &fragile_notepad::editor::RenderPlan,
    metrics: EditorMetrics,
    decorations: &DecorationModel,
    bounds: Rectangle,
) {
    let text_origin_x = metrics.text_origin_x(decorations);

    renderer.fill_quad(
        renderer::Quad {
            bounds: Rectangle {
                x: 0.0,
                y: 0.0,
                width: text_origin_x,
                height: bounds.height,
            },
            ..renderer::Quad::default()
        },
        Color::from_rgb(0.07, 0.07, 0.07),
    );

    for (index, row) in plan.rows.iter().enumerate() {
        if index % 9 == 0 {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: text_origin_x,
                        y: row.y,
                        width: bounds.width - text_origin_x,
                        height: metrics.line_height,
                    },
                    ..renderer::Quad::default()
                },
                Color::from_rgb(0.09, 0.09, 0.09),
            );
        }

        if index % 5 == 0 {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: row.text_x + metrics.character_width * 4.0,
                        y: row.y + 1.0,
                        width: metrics.character_width * 16.0,
                        height: metrics.line_height - 2.0,
                    },
                    ..renderer::Quad::default()
                },
                Color::from_rgba(0.2, 0.42, 0.86, 0.45),
            );
        }

        draw_plain_text(
            renderer,
            (row.line + 1).to_string(),
            metrics.padding_left,
            row.y,
            metrics,
            Size::new(metrics.line_number_width, metrics.line_height),
            Color::from_rgb(0.55, 0.55, 0.55),
        );
    }
}

fn draw_whitespace_marker_stress(
    renderer: &mut iced::Renderer,
    plan: &fragile_notepad::editor::RenderPlan,
    metrics: EditorMetrics,
) {
    for row in &plan.rows {
        for column in [4, 8, 12, 16, 24, 32, 40, 48] {
            draw_plain_text(
                renderer,
                ".",
                row.text_x + metrics.character_width * column as f32,
                row.y,
                metrics,
                Size::new(metrics.character_width, metrics.line_height),
                Color::from_rgba(0.7, 0.7, 0.7, 0.45),
            );
        }
    }
}

fn draw_whitespace_marker_quads(
    renderer: &mut iced::Renderer,
    plan: &fragile_notepad::editor::RenderPlan,
    metrics: EditorMetrics,
    decorations: &DecorationModel,
    bounds: Rectangle,
) {
    for row in &plan.rows {
        for column in [4, 8, 12, 16, 24, 32, 40, 48] {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: space_marker_bounds(
                        bounds,
                        EditorLayout::new(metrics, ScrollOffset::ZERO, WIDTH as f32, HEIGHT as f32),
                        decorations,
                        row,
                        column,
                    ),
                    ..renderer::Quad::default()
                },
                Color::from_rgba(0.7, 0.7, 0.7, 0.45),
            );
        }
    }
}

fn draw_scrollbar_stress(renderer: &mut iced::Renderer, bounds: Rectangle) {
    let track = Rectangle {
        x: bounds.width - 14.0,
        y: 4.0,
        width: 10.0,
        height: bounds.height - 8.0,
    };
    let thumb = Rectangle {
        x: bounds.width - 13.0,
        y: bounds.height * 0.42,
        width: 8.0,
        height: 160.0,
    };

    for rectangle in [track, thumb] {
        renderer.fill_quad(
            renderer::Quad {
                bounds: rectangle,
                border: iced::Border {
                    color: Color::from_rgba(0.8, 0.8, 0.8, 0.25),
                    width: 1.0,
                    radius: 3.0.into(),
                },
                ..renderer::Quad::default()
            },
            Color::from_rgba(0.55, 0.55, 0.55, 0.35),
        );
    }
}

fn draw_plain_text(
    renderer: &mut iced::Renderer,
    content: impl Into<String>,
    x: f32,
    y: f32,
    metrics: EditorMetrics,
    bounds: Size,
    color: Color,
) {
    renderer.fill_text(
        text::Text {
            content: content.into(),
            bounds,
            size: Pixels(text_size(metrics)),
            line_height: text::LineHeight::Absolute(Pixels(metrics.line_height)),
            font: EDITOR_FONT,
            align_x: text::Alignment::Left,
            align_y: alignment::Vertical::Top,
            shaping: text::Shaping::Auto,
            wrapping: text::Wrapping::None,
            ellipsis: text::Ellipsis::None,
            hint_factor: renderer.scale_factor(),
        },
        Point::new(x, y + text_baseline_offset(metrics)),
        color,
        Rectangle::new(Point::ORIGIN, Size::new(WIDTH as f32, HEIGHT as f32)),
    );
}

fn visible_rows(metrics: EditorMetrics) -> usize {
    EditorLayout::new(metrics, ScrollOffset::ZERO, WIDTH as f32, HEIGHT as f32)
        .visible_row_capacity()
}

fn rust_fixture(lines: usize) -> String {
    let mut text = String::with_capacity(lines * 72);

    for index in 0..lines {
        match index % 6 {
            0 => text.push_str(&format!(
                "pub fn function_{index}(input: usize) -> usize {{\n"
            )),
            1 => text.push_str("    let mut value = input.saturating_mul(31);\n"),
            2 => text.push_str("    if value % 2 == 0 { value += 1; } else { value -= 1; }\n"),
            3 => text.push_str("    let label = \"syntax highlighting pressure\";\n"),
            4 => text.push_str("    value + label.len()\n"),
            _ => text.push_str("}\n"),
        }
    }

    text
}

fn long_line_fixture(lines: usize, line_len: usize) -> String {
    let mut text = String::with_capacity(lines * (line_len + 1));
    let chunk = "pub fn long_visible_line(input: usize) -> usize { input.saturating_mul(31) } ";

    for _ in 0..lines {
        let mut written = 0usize;

        while written < line_len {
            let remaining = line_len - written;
            let take = remaining.min(chunk.len());

            text.push_str(&chunk[..take]);
            written += take;
        }

        text.push('\n');
    }

    text
}
}

#[cfg(not(feature = "hybrid-rendering"))]
fn main() {
    tiny_skia_profile::main();
}

#[cfg(feature = "hybrid-rendering")]
fn main() {
    eprintln!("profile_tiny_skia_text is tiny-skia-only; run without --features hybrid-rendering");
}
