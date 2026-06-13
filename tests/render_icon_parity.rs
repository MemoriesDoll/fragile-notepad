use iced::advanced::image;
use iced::advanced::renderer::{self, Headless};
use iced::{Color, Rectangle, Size};

use fragile_notepad::ui::icons::ICON_SIZE;
use fragile_notepad::ui::icons::hero::HeroIconAsset;
use fragile_notepad::ui::icons::shortcut::ShortcutIconAsset;
use fragile_notepad::ui::icons::tango::TangoIconAsset;

const SURFACE_SIZE: u32 = 32;

#[derive(Debug, Clone, Copy)]
enum TestIconAsset {
    Hero(HeroIconAsset),
    Shortcut(ShortcutIconAsset),
    Tango(TangoIconAsset),
}

impl TestIconAsset {
    fn rgba_bytes(self) -> &'static [u8] {
        match self {
            Self::Hero(asset) => asset.rgba_bytes(),
            Self::Shortcut(asset) => asset.rgba_bytes(),
            Self::Tango(asset) => asset.rgba_bytes(),
        }
    }
}

const ICONS: &[(&str, TestIconAsset)] = &[
    (
        "bootstrap/command",
        TestIconAsset::Shortcut(ShortcutIconAsset::Command),
    ),
    (
        "bootstrap/option",
        TestIconAsset::Shortcut(ShortcutIconAsset::Option),
    ),
    (
        "bootstrap/pin-angle-fill",
        TestIconAsset::Shortcut(ShortcutIconAsset::PinAngleFill),
    ),
    (
        "bootstrap/pin-angle",
        TestIconAsset::Shortcut(ShortcutIconAsset::PinAngle),
    ),
    (
        "bootstrap/shift-fill",
        TestIconAsset::Shortcut(ShortcutIconAsset::ShiftFill),
    ),
    (
        "bootstrap/shift",
        TestIconAsset::Shortcut(ShortcutIconAsset::Shift),
    ),
    (
        "bootstrap/windows",
        TestIconAsset::Shortcut(ShortcutIconAsset::Windows),
    ),
    (
        "heroicons/arrow-turn-down-left",
        TestIconAsset::Hero(HeroIconAsset::ArrowTurnDownLeft),
    ),
    (
        "heroicons/chevron-down",
        TestIconAsset::Hero(HeroIconAsset::ChevronDown),
    ),
    (
        "heroicons/chevron-right",
        TestIconAsset::Hero(HeroIconAsset::ChevronRight),
    ),
    ("heroicons/minus", TestIconAsset::Hero(HeroIconAsset::Minus)),
    ("heroicons/plus", TestIconAsset::Hero(HeroIconAsset::Plus)),
    (
        "heroicons/question-mark-circle",
        TestIconAsset::Hero(HeroIconAsset::QuestionMarkCircle),
    ),
    (
        "heroicons/x-mark",
        TestIconAsset::Hero(HeroIconAsset::XMark),
    ),
    (
        "tango/accessories-character-map",
        TestIconAsset::Tango(TangoIconAsset::AccessoriesCharacterMap),
    ),
    (
        "tango/document-new",
        TestIconAsset::Tango(TangoIconAsset::DocumentNew),
    ),
    (
        "tango/document-open",
        TestIconAsset::Tango(TangoIconAsset::DocumentOpen),
    ),
    (
        "tango/document-print",
        TestIconAsset::Tango(TangoIconAsset::DocumentPrint),
    ),
    (
        "tango/document-save-as",
        TestIconAsset::Tango(TangoIconAsset::DocumentSaveAs),
    ),
    (
        "tango/document-save",
        TestIconAsset::Tango(TangoIconAsset::DocumentSave),
    ),
    (
        "tango/edit-copy",
        TestIconAsset::Tango(TangoIconAsset::EditCopy),
    ),
    (
        "tango/edit-cut",
        TestIconAsset::Tango(TangoIconAsset::EditCut),
    ),
    (
        "tango/edit-delete",
        TestIconAsset::Tango(TangoIconAsset::EditDelete),
    ),
    (
        "tango/edit-find-replace",
        TestIconAsset::Tango(TangoIconAsset::EditFindReplace),
    ),
    (
        "tango/edit-find",
        TestIconAsset::Tango(TangoIconAsset::EditFind),
    ),
    (
        "tango/edit-paste",
        TestIconAsset::Tango(TangoIconAsset::EditPaste),
    ),
    (
        "tango/edit-redo",
        TestIconAsset::Tango(TangoIconAsset::EditRedo),
    ),
    (
        "tango/edit-undo",
        TestIconAsset::Tango(TangoIconAsset::EditUndo),
    ),
    (
        "tango/emblem-favorite",
        TestIconAsset::Tango(TangoIconAsset::EmblemFavorite),
    ),
    (
        "tango/emblem-important",
        TestIconAsset::Tango(TangoIconAsset::EmblemImportant),
    ),
    (
        "tango/format-indent-more",
        TestIconAsset::Tango(TangoIconAsset::FormatIndentMore),
    ),
    (
        "tango/format-justify-fill",
        TestIconAsset::Tango(TangoIconAsset::FormatJustifyFill),
    ),
    (
        "tango/process-stop",
        TestIconAsset::Tango(TangoIconAsset::ProcessStop),
    ),
    (
        "tango/tab-close",
        TestIconAsset::Tango(TangoIconAsset::TabClose),
    ),
    (
        "tango/tab-document-monitoring",
        TestIconAsset::Tango(TangoIconAsset::TabDocumentMonitoring),
    ),
    (
        "tango/tab-document-read-only",
        TestIconAsset::Tango(TangoIconAsset::TabDocumentReadOnly),
    ),
    (
        "tango/tab-document-saved",
        TestIconAsset::Tango(TangoIconAsset::TabDocumentSaved),
    ),
    (
        "tango/tab-document-system-read-only",
        TestIconAsset::Tango(TangoIconAsset::TabDocumentSystemReadOnly),
    ),
    (
        "tango/tab-document-unsaved",
        TestIconAsset::Tango(TangoIconAsset::TabDocumentUnsaved),
    ),
    (
        "tango/text-x-generic-template",
        TestIconAsset::Tango(TangoIconAsset::TextXGenericTemplate),
    ),
    (
        "tango/text-x-generic",
        TestIconAsset::Tango(TangoIconAsset::TextXGeneric),
    ),
    (
        "tango/text-x-script",
        TestIconAsset::Tango(TangoIconAsset::TextXScript),
    ),
    (
        "tango/zoom-in",
        TestIconAsset::Tango(TangoIconAsset::ZoomIn),
    ),
    (
        "tango/zoom-out",
        TestIconAsset::Tango(TangoIconAsset::ZoomOut),
    ),
];

fn draw_icon(renderer: &mut iced::Renderer, rgba: &[u8]) {
    let bounds = Rectangle {
        x: 5.0,
        y: 5.0,
        width: ICON_SIZE as f32,
        height: ICON_SIZE as f32,
    };
    let clip_bounds = Rectangle::with_size(Size::new(SURFACE_SIZE as f32, SURFACE_SIZE as f32));
    let handle = image::Handle::from_rgba(ICON_SIZE, ICON_SIZE, rgba.to_vec());
    let icon = image::Image::new(handle).filter_method(image::FilterMethod::Linear);

    renderer::Renderer::reset(renderer, clip_bounds);
    image::Renderer::draw_image(renderer, icon, bounds, clip_bounds);
}

fn render_icon(backend: &str, scale_factor: f32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut renderer = futures::executor::block_on(<iced::Renderer as Headless>::new(
        renderer::Settings::default(),
        Some(backend),
    ))?;

    draw_icon(&mut renderer, rgba);

    Some(renderer.screenshot(
        Size::new(
            (SURFACE_SIZE as f32 * scale_factor).round() as u32,
            (SURFACE_SIZE as f32 * scale_factor).round() as u32,
        ),
        scale_factor,
        Color::TRANSPARENT,
    ))
}

fn diff_stats(a: &[u8], b: &[u8]) -> (u8, f32, usize) {
    let mut max_delta = 0;
    let mut sum_delta = 0usize;
    let mut changed_channels = 0usize;

    for (&a, &b) in a.iter().zip(b) {
        let delta = a.abs_diff(b);

        max_delta = max_delta.max(delta);
        sum_delta += usize::from(delta);

        if delta > 0 {
            changed_channels += 1;
        }
    }

    (
        max_delta,
        sum_delta as f32 / a.len() as f32,
        changed_channels,
    )
}

#[derive(Debug)]
struct TopEdgeOvershoot {
    amount: usize,
    detail: Option<(usize, usize, usize, u8, u8)>,
}

fn top_alpha_overshoot(
    cpu: &[u8],
    gpu: &[u8],
    width: usize,
    alpha_threshold: u8,
) -> TopEdgeOvershoot {
    let Some(cpu_top) = top_alpha_row(cpu, width, alpha_threshold) else {
        return TopEdgeOvershoot {
            amount: 0,
            detail: None,
        };
    };
    let Some(gpu_top) = top_alpha_row(gpu, width, alpha_threshold) else {
        return TopEdgeOvershoot {
            amount: 0,
            detail: None,
        };
    };

    if cpu_top >= gpu_top {
        return TopEdgeOvershoot {
            amount: 0,
            detail: None,
        };
    }

    let x = (0..width)
        .find(|&x| alpha_at(cpu, width, x, cpu_top) > alpha_threshold)
        .unwrap_or(0);
    TopEdgeOvershoot {
        amount: gpu_top - cpu_top,
        detail: Some((
            x,
            cpu_top,
            gpu_top,
            alpha_at(cpu, width, x, cpu_top),
            alpha_at(gpu, width, x, cpu_top),
        )),
    }
}

fn top_alpha_row(image: &[u8], width: usize, alpha_threshold: u8) -> Option<usize> {
    let height = image.len() / width / 4;

    (0..height).find(|&y| (0..width).any(|x| alpha_at(image, width, x, y) > alpha_threshold))
}

fn alpha_at(image: &[u8], width: usize, x: usize, y: usize) -> u8 {
    image[(y * width + x) * 4 + 3]
}

#[test]
fn tiny_skia_and_wgpu_render_real_icon_consistently() {
    for &(name, asset) in ICONS {
        let rgba = asset.rgba_bytes();

        for scale_factor in [1.0, 2.0] {
            let Some(cpu) = render_icon("tiny-skia", scale_factor, rgba) else {
                panic!("tiny-skia headless renderer should be available");
            };
            let Some(gpu) = render_icon("wgpu", scale_factor, rgba) else {
                eprintln!(
                    "skipping icon render parity test: wgpu headless renderer is unavailable"
                );
                return;
            };

            assert_eq!(cpu.len(), gpu.len());

            let (max_delta, mean_delta, changed_channels) = diff_stats(&cpu, &gpu);
            let surface_width = (SURFACE_SIZE as f32 * scale_factor).round() as usize;
            const VISIBLE_EDGE_ALPHA: u8 = 32;

            let top_overshoot = top_alpha_overshoot(&cpu, &gpu, surface_width, VISIBLE_EDGE_ALPHA);
            let (max_allowed, mean_allowed) = if scale_factor == 1.0 {
                (12, 0.25)
            } else {
                (255, 3.0)
            };

            assert!(
                max_delta <= max_allowed && mean_delta <= mean_allowed,
                "CPU/GPU icon output diverged for {name} at scale_factor={scale_factor}: max_delta={max_delta}, mean_delta={mean_delta:.3}, changed_channels={changed_channels}"
            );

            assert_eq!(
                top_overshoot.amount,
                0,
                "CPU icon top edge extends above GPU for {name} at scale_factor={scale_factor}: top_overshoot={}, cpu_top={:?}, gpu_top={:?}, detail={:?}",
                top_overshoot.amount,
                top_alpha_row(&cpu, surface_width, VISIBLE_EDGE_ALPHA),
                top_alpha_row(&gpu, surface_width, VISIBLE_EDGE_ALPHA),
                top_overshoot.detail
            );
        }
    }
}
