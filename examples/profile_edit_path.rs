use fragile_notepad::core::{Document, DocumentId, FindState};
use fragile_notepad::editor::{EditTransaction, EditorPosition, EditorSelection};
use std::hint::black_box;
use std::time::{Duration, Instant};

const LINE_COUNT: usize = 20_000;
const SAMPLES: usize = 160;

fn main() {
    let source = rust_fixture(LINE_COUNT);

    println!("fixture_lines={LINE_COUNT} samples={SAMPLES}");
    measure_document_inline_insert("document_inline_insert", &source);
    measure_buffer_replace_only("buffer_replace_inline", &source);
    measure_find_refresh("find_refresh_empty_query", &source, "");
    measure_find_refresh("find_refresh_active_query", &source, "value");
}

fn measure_document_inline_insert(label: &str, source: &str) {
    let mut total = Duration::ZERO;
    let mut max = Duration::ZERO;

    for sample in 0..SAMPLES {
        let mut document = Document::from_path(DocumentId::new(sample as u64), "main.rs", source);
        let position = EditorPosition::new(sample % LINE_COUNT, 4);
        document.selection = EditorSelection::new(position, position);

        let start = Instant::now();
        let before_selection = document.selection;
        let delta = document.buffer.replace_range(before_selection.range(), "x");
        let cursor = EditorPosition::new(position.line, position.column + 1);
        document.selection = EditorSelection::new(cursor, cursor);
        document.history.record_with_grouping(EditTransaction {
            delta,
            before_selection,
            after_selection: document.selection,
        });
        document.refresh_text_from(position.line);
        let elapsed = start.elapsed();

        total += elapsed;
        max = max.max(elapsed);
        black_box(document);
    }

    print_timing(label, total, max);
}

fn measure_buffer_replace_only(label: &str, source: &str) {
    let mut total = Duration::ZERO;
    let mut max = Duration::ZERO;

    for sample in 0..SAMPLES {
        let mut document = Document::from_path(DocumentId::new(sample as u64), "main.rs", source);
        let position = EditorPosition::new(sample % LINE_COUNT, 4);
        let selection = EditorSelection::new(position, position);

        let start = Instant::now();
        let delta = document.buffer.replace_range(selection.range(), "x");
        let elapsed = start.elapsed();

        total += elapsed;
        max = max.max(elapsed);
        black_box(delta);
        black_box(document);
    }

    print_timing(label, total, max);
}

fn measure_find_refresh(label: &str, source: &str, query: &str) {
    let mut total = Duration::ZERO;
    let mut max = Duration::ZERO;

    for _ in 0..SAMPLES {
        let mut find = FindState::with_query(query);

        let start = Instant::now();
        find.refresh_matches(black_box(source));
        let elapsed = start.elapsed();

        total += elapsed;
        max = max.max(elapsed);
        black_box(find);
    }

    print_timing(label, total, max);
}

fn print_timing(label: &str, total: Duration, max: Duration) {
    let samples = SAMPLES as f64;
    println!(
        "{label}: avg_us={:.2} max_us={:.2}",
        total.as_secs_f64() * 1_000_000.0 / samples,
        max.as_secs_f64() * 1_000_000.0,
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
