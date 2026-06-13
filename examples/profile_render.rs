use fragile_notepad::editor::{
    DecorationModel, DecorationSettings, EditorBuffer, EditorLayout, EditorMetrics, EditorPosition,
    EditorSelection, FoldModel, RenderPlan, RowRenderPlan, ScrollOffset, SyntaxLineCache,
    ViewportModel, build_render_plan, build_render_plan_with_cache, planned_text_draws,
    planned_text_draws_with_markers,
};
use iced::highlighter;
use std::hint::black_box;
use std::time::{Duration, Instant};

const LINE_COUNT: usize = 20_000;
const LONG_LINE_COUNT: usize = 4_000;
const SAMPLES: usize = 240;
const WORST_SCROLL_COUNT: usize = 10;
const PROFILE_WIDTH: f32 = 1200.0;
const PROFILE_HEIGHT: f32 = 720.0;
const LONG_LINE_VISIBLE_MARGIN_COLUMNS: usize = 8;

fn main() {
    let source = rust_fixture(LINE_COUNT);
    let buffer = EditorBuffer::from_text(source);
    let folds = FoldModel::default();
    let viewport = ViewportModel::new(buffer.line_count(), &folds);
    let decorations = DecorationModel::from_folds(
        DecorationSettings::default(),
        buffer.line_count(),
        &folds,
        vec![],
    );
    let marker_decorations = DecorationModel::from_folds(
        DecorationSettings {
            show_spaces: true,
            show_tabs: true,
            show_end_of_line_markers: true,
            ..DecorationSettings::default()
        },
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

    let txt = highlighter::Settings {
        token: "txt".to_owned(),
        theme: highlighter::Theme::SolarizedDark,
    };
    let rs = highlighter::Settings {
        token: "rs".to_owned(),
        theme: highlighter::Theme::SolarizedDark,
    };

    let rows = viewport.visible_row_count();
    let jump_scrolls = (0..SAMPLES)
        .map(|sample| sample * rows.saturating_sub(1) / SAMPLES)
        .collect::<Vec<_>>();
    let sequential_scrolls = (0..SAMPLES).collect::<Vec<_>>();

    println!("fixture_lines={LINE_COUNT} samples={SAMPLES}");
    measure(
        "plan_txt",
        &buffer,
        &viewport,
        &decorations,
        selection,
        metrics,
        &txt,
        &sequential_scrolls,
    );
    measure(
        "plan_txt_markers",
        &buffer,
        &viewport,
        &marker_decorations,
        selection,
        metrics,
        &txt,
        &sequential_scrolls,
    );
    let cache = measure_cache_build("syntax_cache_build_rs", &buffer, &rs);
    measure_visible_cache_build("syntax_cache_visible_rs", &buffer, &rs, 0, 36);
    measure_visible_cache_after_edit("syntax_cache_visible_after_edit_rs", &buffer, &rs, 20, 36);
    measure_with_cache(
        "plan_rs_cache_sequential",
        &buffer,
        &viewport,
        &decorations,
        selection,
        metrics,
        &cache,
        &sequential_scrolls,
    );
    measure_with_cache(
        "plan_rs_cache_jumps",
        &buffer,
        &viewport,
        &decorations,
        selection,
        metrics,
        &cache,
        &jump_scrolls,
    );

    let long_buffer = EditorBuffer::from_text(long_ascii_rust_fixture(LONG_LINE_COUNT));
    let long_viewport = ViewportModel::new(long_buffer.line_count(), &folds);
    let long_decorations = DecorationModel::from_folds(
        DecorationSettings::default(),
        long_buffer.line_count(),
        &folds,
        vec![],
    );
    let long_cache = measure_cache_build("syntax_cache_build_rs_long_ascii", &long_buffer, &rs);
    let long_scrolls = (0..SAMPLES).collect::<Vec<_>>();
    measure_with_cache(
        "plan_rs_long_ascii_cache_sequential",
        &long_buffer,
        &long_viewport,
        &long_decorations,
        selection,
        metrics,
        &long_cache,
        &long_scrolls,
    );

    let fallback_buffer = EditorBuffer::from_text(tabbed_unicode_rust_fixture(LONG_LINE_COUNT));
    let fallback_viewport = ViewportModel::new(fallback_buffer.line_count(), &folds);
    let fallback_decorations = DecorationModel::from_folds(
        DecorationSettings::default(),
        fallback_buffer.line_count(),
        &folds,
        vec![],
    );
    let fallback_cache = measure_cache_build(
        "syntax_cache_build_rs_tabbed_unicode",
        &fallback_buffer,
        &rs,
    );
    let fallback_scrolls = (0..SAMPLES).collect::<Vec<_>>();
    measure_with_cache(
        "plan_rs_tabbed_unicode_cache_sequential",
        &fallback_buffer,
        &fallback_viewport,
        &fallback_decorations,
        selection,
        metrics,
        &fallback_cache,
        &fallback_scrolls,
    );

    let real_source = include_str!("../src/editor/widget.rs");
    let real_buffer = EditorBuffer::from_text(real_source);
    let real_viewport = ViewportModel::new(real_buffer.line_count(), &folds);
    let real_decorations = DecorationModel::from_folds(
        DecorationSettings::default(),
        real_buffer.line_count(),
        &folds,
        vec![],
    );
    measure_lazy_cache_full_scroll(
        "plan_widget_rs_lazy_cache_full_scroll",
        &real_buffer,
        &real_viewport,
        &real_decorations,
        selection,
        metrics,
        &rs,
        false,
    );
    measure_lazy_cache_full_scroll(
        "plan_widget_rs_prewarmed_cache_full_scroll",
        &real_buffer,
        &real_viewport,
        &real_decorations,
        selection,
        metrics,
        &rs,
        true,
    );

    measure_paragraph_cache_simulation("rich_paragraph_cache_scroll", 37, SAMPLES);
}

fn measure(
    label: &str,
    buffer: &EditorBuffer,
    viewport: &ViewportModel,
    decorations: &DecorationModel,
    selection: EditorSelection,
    metrics: EditorMetrics,
    syntax: &highlighter::Settings,
    scrolls: &[usize],
) {
    let mut total = Duration::ZERO;
    let mut max = Duration::ZERO;
    let mut total_rows = 0usize;
    let mut total_spans = 0usize;
    let mut total_draws = 0usize;
    let mut total_fast_draws = 0usize;
    let mut total_marker_draws = 0usize;
    let mut total_old_marker_draws = 0usize;
    let mut visible_text = VisibleTextTotals::default();

    for first_visible_row in scrolls {
        let layout = EditorLayout::new(
            metrics,
            ScrollOffset {
                first_visible_row: *first_visible_row,
                horizontal_px: 0.0,
            },
            PROFILE_WIDTH,
            PROFILE_HEIGHT,
        );
        let start = Instant::now();
        let plan = build_render_plan(
            black_box(buffer),
            black_box(viewport),
            black_box(decorations),
            black_box(selection),
            black_box(layout),
            black_box(syntax),
        );
        let elapsed = start.elapsed();

        total += elapsed;
        max = max.max(elapsed);
        total_rows += plan.rows.len();
        total_spans += plan
            .rows
            .iter()
            .map(|row| row.syntax_spans.len())
            .sum::<usize>();
        total_draws += planned_text_draws(&plan, false);
        total_fast_draws += planned_text_draws(&plan, true);
        total_marker_draws += planned_text_draws_with_markers(&plan, false);
        total_old_marker_draws += planned_text_draws(&plan, false)
            + plan
                .rows
                .iter()
                .map(|row| row.whitespace.len() + usize::from(row.eol.is_some()))
                .sum::<usize>();
        visible_text.add_plan(&plan, decorations, metrics, PROFILE_WIDTH);
        black_box(plan);
    }

    let samples = scrolls.len() as f64;
    let visible_summary = visible_text.summary(samples);
    println!(
        "{label}: avg_us={:.2} max_us={:.2} avg_rows={:.2} avg_spans={:.2} avg_draws={:.2} avg_fast_draws={:.2} avg_marker_draws={:.2} old_avg_marker_draws={:.2} avg_visible_text_bytes={:.2} avg_full_text_bytes={:.2} visible_text_pct={:.2} avg_fallback_rows={:.2}",
        total.as_secs_f64() * 1_000_000.0 / samples,
        max.as_secs_f64() * 1_000_000.0,
        total_rows as f64 / samples,
        total_spans as f64 / samples,
        total_draws as f64 / samples,
        total_fast_draws as f64 / samples,
        total_marker_draws as f64 / samples,
        total_old_marker_draws as f64 / samples,
        visible_summary.avg_visible_bytes,
        visible_summary.avg_full_bytes,
        visible_summary.visible_percent,
        visible_summary.avg_fallback_rows,
    );
}

fn measure_visible_cache_build(
    label: &str,
    buffer: &EditorBuffer,
    syntax: &highlighter::Settings,
    first_line: usize,
    last_line: usize,
) {
    let start = Instant::now();
    let mut cache = SyntaxLineCache::new(syntax);
    cache.ensure_visible(buffer, syntax, first_line, last_line);
    let elapsed = start.elapsed();

    println!(
        "{label}: total_us={:.2} cached_lines={}",
        elapsed.as_secs_f64() * 1_000_000.0,
        cache.cached_line_count(),
    );
}

fn measure_visible_cache_after_edit(
    label: &str,
    buffer: &EditorBuffer,
    syntax: &highlighter::Settings,
    changed_line: usize,
    last_visible_line: usize,
) {
    let mut cache = SyntaxLineCache::new(syntax);
    cache.ensure_visible(buffer, syntax, 0, last_visible_line);
    cache.invalidate_from(changed_line);

    let start = Instant::now();
    cache.ensure_visible(buffer, syntax, changed_line, last_visible_line);
    let elapsed = start.elapsed();

    println!(
        "{label}: total_us={:.2} cached_lines={} changed_line={} last_visible_line={}",
        elapsed.as_secs_f64() * 1_000_000.0,
        cache.cached_line_count(),
        changed_line,
        last_visible_line,
    );
}

fn measure_lazy_cache_full_scroll(
    label: &str,
    buffer: &EditorBuffer,
    viewport: &ViewportModel,
    decorations: &DecorationModel,
    selection: EditorSelection,
    metrics: EditorMetrics,
    syntax: &highlighter::Settings,
    prewarm_initial_window: bool,
) {
    let visible_rows =
        EditorLayout::new(metrics, ScrollOffset::ZERO, PROFILE_WIDTH, PROFILE_HEIGHT)
            .visible_row_capacity();
    let max_first_visible_row = viewport.visible_row_count().saturating_sub(visible_rows);
    let mut cache = SyntaxLineCache::new(syntax);
    let mut samples = Vec::with_capacity(max_first_visible_row + 1);
    let mut total = Duration::ZERO;
    let mut total_cache = Duration::ZERO;
    let mut total_plan = Duration::ZERO;

    if prewarm_initial_window {
        let prewarm_last_row = visible_rows.saturating_mul(2);
        let prewarm_last_line = viewport
            .visible_row_to_document_line(prewarm_last_row)
            .unwrap_or_else(|| buffer.line_count().saturating_sub(1));
        cache.ensure_visible(buffer, syntax, 0, prewarm_last_line);
    }

    for first_visible_row in 0..=max_first_visible_row {
        let layout = EditorLayout::new(
            metrics,
            ScrollOffset {
                first_visible_row,
                horizontal_px: 0.0,
            },
            PROFILE_WIDTH,
            PROFILE_HEIGHT,
        );
        let first_line = viewport
            .visible_row_to_document_line(first_visible_row)
            .unwrap_or(0);
        let last_row = first_visible_row.saturating_add(layout.visible_row_capacity());
        let last_line = viewport
            .visible_row_to_document_line(last_row)
            .unwrap_or_else(|| buffer.line_count().saturating_sub(1));
        let cached_before = cache.cached_line_count();

        let cache_start = Instant::now();
        cache.ensure_visible(buffer, syntax, first_line, last_line);
        let cache_elapsed = cache_start.elapsed();

        let plan_start = Instant::now();
        let plan = build_render_plan_with_cache(
            black_box(buffer),
            black_box(viewport),
            black_box(decorations),
            black_box(selection),
            black_box(layout),
            black_box(&cache),
        );
        let plan_elapsed = plan_start.elapsed();
        let elapsed = cache_elapsed + plan_elapsed;
        let visible = VisibleTextTotals::from_plan(&plan, decorations, metrics, PROFILE_WIDTH);

        total += elapsed;
        total_cache += cache_elapsed;
        total_plan += plan_elapsed;
        samples.push(LazyScrollSample {
            first_visible_row,
            first_line,
            last_line,
            cache_elapsed,
            plan_elapsed,
            total_elapsed: elapsed,
            cached_before,
            cached_after: cache.cached_line_count(),
            rows: plan.rows.len(),
            spans: plan.rows.iter().map(|row| row.syntax_spans.len()).sum(),
            visible_bytes: visible.visible_bytes,
            full_bytes: visible.full_bytes,
            fallback_rows: visible.fallback_rows,
        });
    }

    let sample_count = samples.len() as f64;
    let mut totals = samples
        .iter()
        .map(|sample| sample.total_elapsed)
        .collect::<Vec<_>>();
    totals.sort_unstable();
    let p95_index = ((totals.len() * 95) / 100).min(totals.len().saturating_sub(1));
    let p99_index = ((totals.len() * 99) / 100).min(totals.len().saturating_sub(1));
    let max_total = totals.last().copied().unwrap_or(Duration::ZERO);

    samples.sort_by_key(|sample| std::cmp::Reverse(sample.total_elapsed));

    println!(
        "{label}: file=src/editor/widget.rs lines={} scroll_offsets={} visible_rows={} avg_total_us={:.2} avg_cache_us={:.2} avg_plan_us={:.2} p95_total_us={:.2} p99_total_us={:.2} max_total_us={:.2} cached_lines_final={}",
        buffer.line_count(),
        samples.len(),
        visible_rows,
        total.as_secs_f64() * 1_000_000.0 / sample_count,
        total_cache.as_secs_f64() * 1_000_000.0 / sample_count,
        total_plan.as_secs_f64() * 1_000_000.0 / sample_count,
        totals[p95_index].as_secs_f64() * 1_000_000.0,
        totals[p99_index].as_secs_f64() * 1_000_000.0,
        max_total.as_secs_f64() * 1_000_000.0,
        cache.cached_line_count(),
    );

    for (rank, sample) in samples.iter().take(WORST_SCROLL_COUNT).enumerate() {
        let visible_percent = if sample.full_bytes == 0 {
            0.0
        } else {
            sample.visible_bytes as f64 * 100.0 / sample.full_bytes as f64
        };

        println!(
            "{label}_worst_scroll rank={} first_visible_row={} document_lines={}..{} total_us={:.2} cache_us={:.2} plan_us={:.2} cached_before={} cached_after={} rows={} spans={} visible_bytes={} full_bytes={} visible_pct={:.2} fallback_rows={}",
            rank + 1,
            sample.first_visible_row,
            sample.first_line + 1,
            sample.last_line + 1,
            sample.total_elapsed.as_secs_f64() * 1_000_000.0,
            sample.cache_elapsed.as_secs_f64() * 1_000_000.0,
            sample.plan_elapsed.as_secs_f64() * 1_000_000.0,
            sample.cached_before,
            sample.cached_after,
            sample.rows,
            sample.spans,
            sample.visible_bytes,
            sample.full_bytes,
            visible_percent,
            sample.fallback_rows,
        );
    }
}

#[derive(Debug)]
struct LazyScrollSample {
    first_visible_row: usize,
    first_line: usize,
    last_line: usize,
    cache_elapsed: Duration,
    plan_elapsed: Duration,
    total_elapsed: Duration,
    cached_before: usize,
    cached_after: usize,
    rows: usize,
    spans: usize,
    visible_bytes: usize,
    full_bytes: usize,
    fallback_rows: usize,
}

fn measure_with_cache(
    label: &str,
    buffer: &EditorBuffer,
    viewport: &ViewportModel,
    decorations: &DecorationModel,
    selection: EditorSelection,
    metrics: EditorMetrics,
    cache: &SyntaxLineCache,
    scrolls: &[usize],
) {
    let mut total = Duration::ZERO;
    let mut max = Duration::ZERO;
    let mut total_rows = 0usize;
    let mut total_spans = 0usize;
    let mut total_draws = 0usize;
    let mut total_fast_draws = 0usize;
    let mut total_marker_draws = 0usize;
    let mut total_old_marker_draws = 0usize;
    let mut visible_text = VisibleTextTotals::default();

    for first_visible_row in scrolls {
        let layout = EditorLayout::new(
            metrics,
            ScrollOffset {
                first_visible_row: *first_visible_row,
                horizontal_px: 0.0,
            },
            PROFILE_WIDTH,
            PROFILE_HEIGHT,
        );
        let start = Instant::now();
        let plan = build_render_plan_with_cache(
            black_box(buffer),
            black_box(viewport),
            black_box(decorations),
            black_box(selection),
            black_box(layout),
            black_box(cache),
        );
        let elapsed = start.elapsed();

        total += elapsed;
        max = max.max(elapsed);
        total_rows += plan.rows.len();
        total_spans += plan
            .rows
            .iter()
            .map(|row| row.syntax_spans.len())
            .sum::<usize>();
        total_draws += planned_text_draws(&plan, false);
        total_fast_draws += planned_text_draws(&plan, true);
        total_marker_draws += planned_text_draws_with_markers(&plan, false);
        total_old_marker_draws += planned_text_draws(&plan, false)
            + plan
                .rows
                .iter()
                .map(|row| row.whitespace.len() + usize::from(row.eol.is_some()))
                .sum::<usize>();
        visible_text.add_plan(&plan, decorations, metrics, PROFILE_WIDTH);
        black_box(plan);
    }

    let samples = scrolls.len() as f64;
    let visible_summary = visible_text.summary(samples);
    println!(
        "{label}: avg_us={:.2} max_us={:.2} avg_rows={:.2} avg_spans={:.2} avg_draws={:.2} avg_fast_draws={:.2} avg_marker_draws={:.2} old_avg_marker_draws={:.2} avg_visible_text_bytes={:.2} avg_full_text_bytes={:.2} visible_text_pct={:.2} avg_fallback_rows={:.2}",
        total.as_secs_f64() * 1_000_000.0 / samples,
        max.as_secs_f64() * 1_000_000.0,
        total_rows as f64 / samples,
        total_spans as f64 / samples,
        total_draws as f64 / samples,
        total_fast_draws as f64 / samples,
        total_marker_draws as f64 / samples,
        total_old_marker_draws as f64 / samples,
        visible_summary.avg_visible_bytes,
        visible_summary.avg_full_bytes,
        visible_summary.visible_percent,
        visible_summary.avg_fallback_rows,
    );
}

#[derive(Debug, Default)]
struct VisibleTextTotals {
    visible_bytes: usize,
    full_bytes: usize,
    fallback_rows: usize,
}

impl VisibleTextTotals {
    fn from_plan(
        plan: &RenderPlan,
        decorations: &DecorationModel,
        metrics: EditorMetrics,
        width: f32,
    ) -> Self {
        let mut totals = Self::default();
        totals.add_plan(plan, decorations, metrics, width);
        totals
    }

    fn add_plan(
        &mut self,
        plan: &RenderPlan,
        decorations: &DecorationModel,
        metrics: EditorMetrics,
        width: f32,
    ) {
        let text_area_x = metrics.text_origin_x(decorations);
        let text_area_width = (width - text_area_x).max(0.0);

        for row in &plan.rows {
            let full_len = row.text.len();
            let visible_len =
                estimated_visible_text_len(row, text_area_x, text_area_width, metrics);

            self.full_bytes += full_len;
            self.visible_bytes += visible_len;

            if visible_len == full_len && !can_clip_row_text(row, metrics) {
                self.fallback_rows += 1;
            }
        }
    }

    fn summary(&self, samples: f64) -> VisibleTextSummary {
        let visible_percent = if self.full_bytes == 0 {
            0.0
        } else {
            self.visible_bytes as f64 * 100.0 / self.full_bytes as f64
        };

        VisibleTextSummary {
            avg_visible_bytes: self.visible_bytes as f64 / samples,
            avg_full_bytes: self.full_bytes as f64 / samples,
            visible_percent,
            avg_fallback_rows: self.fallback_rows as f64 / samples,
        }
    }
}

#[derive(Debug)]
struct VisibleTextSummary {
    avg_visible_bytes: f64,
    avg_full_bytes: f64,
    visible_percent: f64,
    avg_fallback_rows: f64,
}

fn estimated_visible_text_len(
    row: &RowRenderPlan,
    text_area_x: f32,
    text_area_width: f32,
    metrics: EditorMetrics,
) -> usize {
    if !can_clip_row_text(row, metrics) {
        return row.text.len();
    }

    let first = ((text_area_x - row.text_x) / metrics.character_width)
        .floor()
        .max(0.0) as usize;
    let last = ((text_area_x + text_area_width - row.text_x) / metrics.character_width)
        .ceil()
        .max(0.0) as usize;
    let start = first.saturating_sub(LONG_LINE_VISIBLE_MARGIN_COLUMNS);
    let end = last
        .saturating_add(LONG_LINE_VISIBLE_MARGIN_COLUMNS)
        .min(row.text.len());
    let start = estimated_style_boundary_start(row, start.min(row.text.len()));

    end.max(start).min(row.text.len()) - start.min(row.text.len())
}

fn estimated_style_boundary_start(row: &RowRenderPlan, start: usize) -> usize {
    row.syntax_spans
        .iter()
        .filter(|span| span.range.start < start && start < span.range.end)
        .map(|span| span.range.start)
        .max()
        .unwrap_or(start)
}

fn can_clip_row_text(row: &RowRenderPlan, metrics: EditorMetrics) -> bool {
    metrics.character_width > 0.0
        && row
            .text
            .bytes()
            .all(|byte| byte.is_ascii() && byte != b'\t')
        && row.syntax_spans.iter().all(|span| {
            span.range.start < span.range.end
                && span.range.end <= row.text.len()
                && row.text.is_char_boundary(span.range.start)
                && row.text.is_char_boundary(span.range.end)
        })
}

fn measure_cache_build(
    label: &str,
    buffer: &EditorBuffer,
    syntax: &highlighter::Settings,
) -> SyntaxLineCache {
    let start = Instant::now();
    let cache = SyntaxLineCache::rebuild(black_box(buffer), black_box(syntax));
    let elapsed = start.elapsed();

    println!(
        "{label}: total_ms={:.2} line_count={}",
        elapsed.as_secs_f64() * 1_000.0,
        buffer.line_count(),
    );

    cache
}

fn measure_paragraph_cache_simulation(label: &str, visible_rows: usize, samples: usize) {
    use std::collections::HashSet;

    let mut cached = HashSet::new();
    let mut builds = 0usize;
    let mut hits = 0usize;
    let mut linear_lookup_probes = 0usize;
    let mut cached_rows = 0usize;

    for first_visible_row in 0..samples {
        for line in first_visible_row..first_visible_row + visible_rows {
            linear_lookup_probes += cached_rows.min(256);
            if cached.insert(line) {
                builds += 1;
                cached_rows = (cached_rows + 1).min(256);
            } else {
                hits += 1;
            }
        }
    }

    let direct_lookup_probes = builds + hits;

    println!(
        "{label}: frames={samples} visible_rows={visible_rows} paragraph_builds={builds} cache_hits={hits} old_linear_lookup_probes={linear_lookup_probes} direct_lookup_probes={direct_lookup_probes}"
    );
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

fn long_ascii_rust_fixture(lines: usize) -> String {
    let mut text = String::with_capacity(lines * 512);

    for index in 0..lines {
        text.push_str(&format!(
            "let highlighted_value_{index} = compute_highlighted_scroll_cost(input_{index}, \"{}\", {}); // {}\n",
            "ascii token ".repeat(24),
            index.saturating_mul(31),
            "visible styled range pressure ".repeat(8),
        ));
    }

    text
}

fn tabbed_unicode_rust_fixture(lines: usize) -> String {
    let mut text = String::with_capacity(lines * 160);

    for index in 0..lines {
        if index % 2 == 0 {
            text.push_str(&format!(
                "let fallback_{index} = value_{index};\t// tab keeps full row fallback\n"
            ));
        } else {
            text.push_str(&format!(
                "let unicode_{index} = \"syntax \u{597d} fallback {index}\"; // non ASCII fallback\n"
            ));
        }
    }

    text
}
