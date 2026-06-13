use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Default)]
struct Stats {
    values: Vec<u128>,
}

impl Stats {
    fn push(&mut self, value: u128) {
        self.values.push(value);
    }

    fn print(&mut self, event: &str) {
        if self.values.is_empty() {
            return;
        }

        self.values.sort_unstable();
        let count = self.values.len();
        let sum = self.values.iter().sum::<u128>() as f64;
        let avg = sum / count as f64;
        let p95 = percentile(&self.values, 95);
        let p99 = percentile(&self.values, 99);
        let max = self.values[count - 1];

        println!(
            "{event}: count={count} avg_ms={:.3} p95_ms={:.3} p99_ms={:.3} max_ms={:.3}",
            avg / 1_000.0,
            p95 as f64 / 1_000.0,
            p99 as f64 / 1_000.0,
            max as f64 / 1_000.0,
        );
    }

    fn summary(&mut self) -> Option<Summary> {
        if self.values.is_empty() {
            return None;
        }

        self.values.sort_unstable();
        let count = self.values.len();
        let sum = self.values.iter().sum::<u128>() as f64;

        Some(Summary {
            count,
            avg: sum / count as f64,
            p95: percentile(&self.values, 95),
            p99: percentile(&self.values, 99),
            max: self.values[count - 1],
        })
    }
}

struct Summary {
    count: usize,
    avg: f64,
    p95: u128,
    p99: u128,
    max: u128,
}

impl Summary {
    fn print(&self, label: &str) {
        println!(
            "{label}: count={} avg_ms={:.3} p95_ms={:.3} p99_ms={:.3} max_ms={:.3}",
            self.count,
            self.avg / 1_000.0,
            self.p95 as f64 / 1_000.0,
            self.p99 as f64 / 1_000.0,
            self.max as f64 / 1_000.0,
        );
    }
}

#[derive(Debug)]
struct Row {
    since_start: u128,
    event: String,
    elapsed: u128,
    detail: String,
}

#[derive(Debug)]
struct PresentDraw {
    since_start: u128,
    elapsed: u128,
    raw_damage: Option<u64>,
    grouped_damage: Option<u64>,
    grouped_damage_area: Option<f64>,
    damage_strategy: Option<String>,
    damage_area: Option<f64>,
    scrolls: Option<u64>,
    damage_quads: Option<u64>,
    damage_text: Option<u64>,
    damage_primitives: Option<u64>,
    damage_images: Option<u64>,
    damage_scroll: Option<bool>,
    draw_layer_visits: Option<u64>,
    draw_quads: Option<u64>,
    draw_primitives: Option<u64>,
    draw_images: Option<u64>,
    draw_text_groups: Option<u64>,
    draw_text_items: Option<u64>,
    paragraph_raster_hits: Option<u64>,
    paragraph_raster_misses: Option<u64>,
    paragraph_raster_bypasses: Option<u64>,
    glyph_hits: Option<u64>,
    glyph_misses: Option<u64>,
    detail: String,
}

#[derive(Debug)]
struct EditorDraw {
    since_start: u128,
    total_us: Option<u128>,
    first_row: Option<u64>,
    rows: Option<u64>,
    spans: Option<u64>,
    selection_range_lines: Option<u64>,
    visible_selection_lines: Option<u64>,
    visible_selection_area: Option<f64>,
    visible_selection_max_width: Option<f64>,
    fast_text: Option<bool>,
    token: Option<String>,
    detail: String,
}

fn main() {
    let path = env::args_os().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from("target")
            .join("perf")
            .join("fragile-perf.csv")
    });
    let text = fs::read_to_string(&path).expect("read perf trace csv");
    let rows = parse_rows(&text);
    let mut stats = BTreeMap::<String, Stats>::new();
    let mut present_times = Vec::new();
    let mut present_draws = Vec::new();
    let mut editor_draws = Vec::new();

    for row in &rows {
        if row.elapsed > 0 {
            stats
                .entry(row.event.clone())
                .or_default()
                .push(row.elapsed);
        }

        if row.event == "tiny_skia_present" {
            present_times.push(row.since_start);
        } else if row.event == "tiny_skia_present_draw" {
            present_draws.push(parse_present_draw(row));
        } else if row.event == "editor_draw" {
            editor_draws.push(parse_editor_draw(row));
        }
    }

    println!("trace={}", path.display());
    for (event, stats) in &mut stats {
        stats.print(event);
    }

    if present_times.len() >= 2 {
        let duration_us = present_times[present_times.len() - 1] - present_times[0];
        let fps = (present_times.len() - 1) as f64 * 1_000_000.0 / duration_us as f64;
        println!(
            "tiny_skia_present_fps: frames={} duration_ms={:.3} avg_fps={:.2}",
            present_times.len(),
            duration_us as f64 / 1_000.0,
            fps,
        );
    }

    print_present_draw_breakdown(&present_draws);
    print_editor_draw_breakdown(&editor_draws);
    print_worst_present_context(&present_draws, &editor_draws);
}

fn percentile(values: &[u128], percent: usize) -> u128 {
    let index = (values.len() * percent / 100).min(values.len().saturating_sub(1));
    values[index]
}

fn parse_rows(text: &str) -> Vec<Row> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let mut fields = line.splitn(4, ',');
            let since_start = fields.next()?.parse::<u128>().ok()?;
            let event = fields.next()?.to_owned();
            let elapsed = fields.next()?.parse::<u128>().ok()?;
            let detail = unquote_csv(fields.next().unwrap_or_default());

            Some(Row {
                since_start,
                event,
                elapsed,
                detail,
            })
        })
        .collect()
}

fn unquote_csv(value: &str) -> String {
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        value[1..value.len() - 1].replace("\"\"", "\"")
    } else {
        value.to_owned()
    }
}

fn parse_present_draw(row: &Row) -> PresentDraw {
    PresentDraw {
        since_start: row.since_start,
        elapsed: row.elapsed,
        raw_damage: parse_u64_detail(&row.detail, "raw_damage"),
        grouped_damage: parse_u64_detail(&row.detail, "grouped_damage"),
        grouped_damage_area: parse_f64_detail(&row.detail, "grouped_damage_area"),
        damage_strategy: parse_string_detail(&row.detail, "damage_strategy"),
        damage_area: parse_f64_detail(&row.detail, "damage_area"),
        scrolls: parse_u64_detail(&row.detail, "scrolls"),
        damage_quads: parse_u64_detail(&row.detail, "damage_quads"),
        damage_text: parse_u64_detail(&row.detail, "damage_text"),
        damage_primitives: parse_u64_detail(&row.detail, "damage_primitives"),
        damage_images: parse_u64_detail(&row.detail, "damage_images"),
        damage_scroll: parse_bool_detail(&row.detail, "damage_scroll"),
        draw_layer_visits: parse_u64_detail(&row.detail, "draw_layer_visits"),
        draw_quads: parse_u64_detail(&row.detail, "draw_quads"),
        draw_primitives: parse_u64_detail(&row.detail, "draw_primitives"),
        draw_images: parse_u64_detail(&row.detail, "draw_images"),
        draw_text_groups: parse_u64_detail(&row.detail, "draw_text_groups"),
        draw_text_items: parse_u64_detail(&row.detail, "draw_text_items"),
        paragraph_raster_hits: parse_u64_detail(&row.detail, "paragraph_raster_hits"),
        paragraph_raster_misses: parse_u64_detail(&row.detail, "paragraph_raster_misses"),
        paragraph_raster_bypasses: parse_u64_detail(&row.detail, "paragraph_raster_bypasses"),
        glyph_hits: parse_u64_detail(&row.detail, "glyph_hits"),
        glyph_misses: parse_u64_detail(&row.detail, "glyph_misses"),
        detail: row.detail.clone(),
    }
}

fn parse_editor_draw(row: &Row) -> EditorDraw {
    EditorDraw {
        since_start: row.since_start,
        total_us: parse_u128_detail(&row.detail, "total_us"),
        first_row: parse_u64_detail(&row.detail, "first_row"),
        rows: parse_u64_detail(&row.detail, "rows"),
        spans: parse_u64_detail(&row.detail, "spans"),
        selection_range_lines: parse_u64_detail(&row.detail, "selection_range_lines"),
        visible_selection_lines: parse_u64_detail(&row.detail, "visible_selection_lines"),
        visible_selection_area: parse_f64_detail(&row.detail, "visible_selection_area"),
        visible_selection_max_width: parse_f64_detail(&row.detail, "visible_selection_max_width"),
        fast_text: parse_bool_detail(&row.detail, "fast_text"),
        token: parse_string_detail(&row.detail, "token"),
        detail: row.detail.clone(),
    }
}

fn parse_u64_detail(detail: &str, key: &str) -> Option<u64> {
    parse_string_detail(detail, key)?.parse().ok()
}

fn parse_u128_detail(detail: &str, key: &str) -> Option<u128> {
    parse_string_detail(detail, key)?.parse().ok()
}

fn parse_f64_detail(detail: &str, key: &str) -> Option<f64> {
    parse_string_detail(detail, key)?.parse().ok()
}

fn parse_bool_detail(detail: &str, key: &str) -> Option<bool> {
    parse_string_detail(detail, key)?.parse().ok()
}

fn parse_string_detail(detail: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    detail.split_whitespace().find_map(|part| {
        part.strip_prefix(&prefix)
            .map(|value| value.trim_end_matches(',').to_owned())
    })
}

fn print_present_draw_breakdown(draws: &[PresentDraw]) {
    if draws.is_empty() {
        return;
    }

    println!();
    println!("tiny_skia_present_draw_breakdown:");

    let mut by_scrolls = BTreeMap::<u64, Stats>::new();
    let mut raw_damage = Stats::default();
    let mut grouped_damage = Stats::default();
    let mut grouped_damage_area = Stats::default();
    let mut damage_area = Stats::default();
    let mut by_damage_strategy = BTreeMap::<String, usize>::new();
    let mut damage_text = Stats::default();
    let mut damage_quads = Stats::default();
    let mut damage_primitives = Stats::default();
    let mut damage_images = Stats::default();
    let mut draw_layer_visits = Stats::default();
    let mut draw_text_items = Stats::default();
    let mut draw_text_groups = Stats::default();
    let mut draw_quads = Stats::default();
    let mut draw_primitives = Stats::default();
    let mut draw_images = Stats::default();
    let mut paragraph_raster_hits = Stats::default();
    let mut paragraph_raster_misses = Stats::default();
    let mut paragraph_raster_bypasses = Stats::default();
    let mut glyph_hits = Stats::default();
    let mut glyph_misses = Stats::default();
    let mut damage_scroll_frames = 0usize;
    let mut slow = 0usize;

    for draw in draws {
        if draw.elapsed >= 16_667 {
            slow += 1;
        }

        if let Some(scrolls) = draw.scrolls {
            by_scrolls.entry(scrolls).or_default().push(draw.elapsed);
        }

        if let Some(value) = draw.raw_damage {
            raw_damage.push(u128::from(value));
        }

        if let Some(value) = draw.grouped_damage {
            grouped_damage.push(u128::from(value));
        }

        if let Some(value) = draw.grouped_damage_area {
            grouped_damage_area.push(value.max(0.0).round() as u128);
        }

        if let Some(strategy) = &draw.damage_strategy {
            *by_damage_strategy.entry(strategy.clone()).or_default() += 1;
        }

        if let Some(value) = draw.damage_area {
            damage_area.push(value.max(0.0).round() as u128);
        }

        if draw.damage_scroll == Some(true) {
            damage_scroll_frames += 1;
        }

        push_u64(&mut damage_text, draw.damage_text);
        push_u64(&mut damage_quads, draw.damage_quads);
        push_u64(&mut damage_primitives, draw.damage_primitives);
        push_u64(&mut damage_images, draw.damage_images);
        push_u64(&mut draw_layer_visits, draw.draw_layer_visits);
        push_u64(&mut draw_text_items, draw.draw_text_items);
        push_u64(&mut draw_text_groups, draw.draw_text_groups);
        push_u64(&mut draw_quads, draw.draw_quads);
        push_u64(&mut draw_primitives, draw.draw_primitives);
        push_u64(&mut draw_images, draw.draw_images);
        push_u64(&mut paragraph_raster_hits, draw.paragraph_raster_hits);
        push_u64(&mut paragraph_raster_misses, draw.paragraph_raster_misses);
        push_u64(
            &mut paragraph_raster_bypasses,
            draw.paragraph_raster_bypasses,
        );
        push_u64(&mut glyph_hits, draw.glyph_hits);
        push_u64(&mut glyph_misses, draw.glyph_misses);
    }

    println!(
        "  frames_over_16_7ms={} of {} ({:.1}%)",
        slow,
        draws.len(),
        slow as f64 * 100.0 / draws.len() as f64
    );

    for (scrolls, stats) in &mut by_scrolls {
        if let Some(summary) = stats.summary() {
            summary.print(&format!("  scrolls={scrolls}"));
        }
    }

    println!("  damage_scroll_frames={damage_scroll_frames}");

    if let Some(summary) = raw_damage.summary() {
        println!(
            "  raw_damage: avg={:.1} p95={} p99={} max={}",
            summary.avg, summary.p95, summary.p99, summary.max
        );
    }

    if let Some(summary) = grouped_damage.summary() {
        println!(
            "  grouped_damage: avg={:.1} p95={} p99={} max={}",
            summary.avg, summary.p95, summary.p99, summary.max
        );
    }

    if !by_damage_strategy.is_empty() {
        let strategies = by_damage_strategy
            .iter()
            .map(|(strategy, count)| format!("{strategy}={count}"))
            .collect::<Vec<_>>()
            .join(" ");

        println!("  damage_strategy: {strategies}");
    }

    if let Some(summary) = grouped_damage_area.summary() {
        println!(
            "  grouped_damage_area_logical_px: avg={:.0} p95={} p99={} max={}",
            summary.avg, summary.p95, summary.p99, summary.max
        );
    }

    if let Some(summary) = damage_area.summary() {
        println!(
            "  damage_area_logical_px: avg={:.0} p95={} p99={} max={}",
            summary.avg, summary.p95, summary.p99, summary.max
        );
    }

    print_count_summary("  damage_text", &mut damage_text);
    print_count_summary("  damage_quads", &mut damage_quads);
    print_count_summary("  damage_primitives", &mut damage_primitives);
    print_count_summary("  damage_images", &mut damage_images);
    print_count_summary("  draw_layer_visits", &mut draw_layer_visits);
    print_count_summary("  draw_text_items", &mut draw_text_items);
    print_count_summary("  draw_text_groups", &mut draw_text_groups);
    print_count_summary("  draw_quads", &mut draw_quads);
    print_count_summary("  draw_primitives", &mut draw_primitives);
    print_count_summary("  draw_images", &mut draw_images);
    print_count_summary("  paragraph_raster_hits", &mut paragraph_raster_hits);
    print_count_summary("  paragraph_raster_misses", &mut paragraph_raster_misses);
    print_count_summary(
        "  paragraph_raster_bypasses",
        &mut paragraph_raster_bypasses,
    );
    print_count_summary("  glyph_hits", &mut glyph_hits);
    print_count_summary("  glyph_misses", &mut glyph_misses);
}

fn push_u64(stats: &mut Stats, value: Option<u64>) {
    if let Some(value) = value {
        stats.push(u128::from(value));
    }
}

fn print_count_summary(label: &str, stats: &mut Stats) {
    if let Some(summary) = stats.summary() {
        println!(
            "{label}: avg={:.1} p95={} p99={} max={}",
            summary.avg, summary.p95, summary.p99, summary.max
        );
    }
}

fn print_editor_draw_breakdown(draws: &[EditorDraw]) {
    if draws.is_empty() {
        return;
    }

    println!();
    println!("editor_draw_breakdown:");

    let mut by_token = BTreeMap::<String, Stats>::new();
    let mut by_fast_text = BTreeMap::<bool, Stats>::new();
    let mut visible_rows = Stats::default();
    let mut syntax_spans = Stats::default();
    let mut selection_range_lines = Stats::default();
    let mut visible_selection_lines = Stats::default();
    let mut visible_selection_area = Stats::default();
    let mut visible_selection_max_width = Stats::default();
    let mut selected_frames = 0usize;
    let mut row_changes = 0usize;
    let mut row_resets = 0usize;
    let mut previous_first_row = None;

    for draw in draws {
        if let (Some(token), Some(total_us)) = (&draw.token, draw.total_us) {
            by_token.entry(token.clone()).or_default().push(total_us);
        }

        if let (Some(fast_text), Some(total_us)) = (draw.fast_text, draw.total_us) {
            by_fast_text.entry(fast_text).or_default().push(total_us);
        }

        if let Some(rows) = draw.rows {
            visible_rows.push(u128::from(rows));
        }

        if let Some(spans) = draw.spans {
            syntax_spans.push(u128::from(spans));
        }

        if let Some(lines) = draw.selection_range_lines {
            selection_range_lines.push(u128::from(lines));
        }

        if let Some(lines) = draw.visible_selection_lines {
            visible_selection_lines.push(u128::from(lines));
            if lines > 0 {
                selected_frames += 1;
            }
        }

        if let Some(area) = draw.visible_selection_area {
            visible_selection_area.push(area.max(0.0).round() as u128);
        }

        if let Some(width) = draw.visible_selection_max_width {
            visible_selection_max_width.push(width.max(0.0).round() as u128);
        }

        if let Some(first_row) = draw.first_row {
            if let Some(previous) = previous_first_row {
                if first_row != previous {
                    row_changes += 1;

                    if first_row == 0 && previous != 0 {
                        row_resets += 1;
                    }
                }
            }

            previous_first_row = Some(first_row);
        }
    }

    for (token, stats) in &mut by_token {
        if let Some(summary) = stats.summary() {
            summary.print(&format!("  token={token}"));
        }
    }

    for (fast_text, stats) in &mut by_fast_text {
        if let Some(summary) = stats.summary() {
            summary.print(&format!("  fast_text={fast_text}"));
        }
    }

    if let Some(summary) = visible_rows.summary() {
        println!(
            "  visible_rows: avg={:.1} p95={} p99={} max={}",
            summary.avg, summary.p95, summary.p99, summary.max
        );
    }

    if let Some(summary) = syntax_spans.summary() {
        println!(
            "  syntax_spans: avg={:.1} p95={} p99={} max={}",
            summary.avg, summary.p95, summary.p99, summary.max
        );
    }

    println!(
        "  selected_frames={} of {} ({:.1}%)",
        selected_frames,
        draws.len(),
        selected_frames as f64 * 100.0 / draws.len() as f64
    );

    if let Some(summary) = selection_range_lines.summary() {
        println!(
            "  selection_range_lines: avg={:.1} p95={} p99={} max={}",
            summary.avg, summary.p95, summary.p99, summary.max
        );
    }

    if let Some(summary) = visible_selection_lines.summary() {
        println!(
            "  visible_selection_lines: avg={:.1} p95={} p99={} max={}",
            summary.avg, summary.p95, summary.p99, summary.max
        );
    }

    if let Some(summary) = visible_selection_area.summary() {
        println!(
            "  visible_selection_area_logical_px: avg={:.0} p95={} p99={} max={}",
            summary.avg, summary.p95, summary.p99, summary.max
        );
    }

    if let Some(summary) = visible_selection_max_width.summary() {
        println!(
            "  visible_selection_max_width_logical_px: avg={:.0} p95={} p99={} max={}",
            summary.avg, summary.p95, summary.p99, summary.max
        );
    }

    println!("  first_row_changes={row_changes} first_row_resets_to_zero={row_resets}");
}

fn print_worst_present_context(draws: &[PresentDraw], editor_draws: &[EditorDraw]) {
    if draws.is_empty() {
        return;
    }

    println!();
    println!("worst_present_draw_context:");

    let mut worst = draws.iter().collect::<Vec<_>>();
    worst.sort_by_key(|draw| std::cmp::Reverse(draw.elapsed));

    for draw in worst.into_iter().take(8) {
        println!("  {}us {}", draw.elapsed, draw.detail);

        for editor in editor_draws
            .iter()
            .filter(|editor| editor.since_start <= draw.since_start)
            .rev()
            .take(3)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            println!(
                "    editor_draw {}us {}",
                editor.total_us.unwrap_or_default(),
                editor.detail
            );
        }
    }
}
