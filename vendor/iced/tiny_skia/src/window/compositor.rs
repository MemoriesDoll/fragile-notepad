use crate::core::backend;
use crate::core::renderer;
use crate::core::{Color, Rectangle, Size};
use crate::graphics::compositor::{self, Information};
use crate::graphics::damage;
use crate::graphics::{Shell, Viewport};
use crate::{Layer, Renderer, trace};

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

pub struct Compositor {
    context: softbuffer::Context<Box<dyn compositor::Display>>,
}

pub struct Surface {
    window: softbuffer::Surface<Box<dyn compositor::Display>, Box<dyn compositor::Window>>,
    clip_mask: tiny_skia::Mask,
    frames: VecDeque<Frame>,
    max_age: u8,
}

#[derive(Clone)]
struct Frame {
    background: Color,
    layers: Arc<[Layer]>,
}

impl crate::graphics::Compositor for Compositor {
    type Renderer = Renderer;
    type Surface = Surface;

    async fn new(
        settings: backend::Settings,
        display: impl compositor::Display,
        _compatible_window: impl compositor::Window,
        _shell: Shell,
    ) -> Result<Self, backend::Error> {
        if !settings.backend.is_software() && !settings.backend.matches("tiny-skia") {
            return Err(backend::Error::GraphicsAdapterNotFound {
                backend: "tiny-skia",
                reason: backend::Reason::DidNotMatch {
                    preferred_backend: settings.backend,
                },
            });
        }

        Ok(new(display))
    }

    fn create_renderer(&self, settings: renderer::Settings) -> Self::Renderer {
        Renderer::new(settings)
    }

    fn create_surface(
        &mut self,
        window: impl compositor::Window + Clone,
        width: u32,
        height: u32,
    ) -> Self::Surface {
        let window = softbuffer::Surface::new(&self.context, Box::new(window.clone()) as _)
            .expect("Create softbuffer surface for window");

        let mut surface = Surface {
            window,
            clip_mask: tiny_skia::Mask::new(1, 1).expect("Create clip mask"),
            frames: VecDeque::new(),
            max_age: 0,
        };

        if width > 0 && height > 0 {
            self.configure_surface(&mut surface, width, height);
        }

        surface
    }

    fn configure_surface(&mut self, surface: &mut Self::Surface, width: u32, height: u32) {
        surface
            .window
            .resize(
                NonZeroU32::new(width).expect("Non-zero width"),
                NonZeroU32::new(height).expect("Non-zero height"),
            )
            .expect("Resize surface");

        surface.clip_mask = tiny_skia::Mask::new(width, height).expect("Create clip mask");
        surface.frames.clear();
    }

    fn information(&self) -> Information {
        Information {
            adapter: String::from("CPU"),
            backend: String::from("tiny-skia"),
        }
    }

    fn present(
        &mut self,
        renderer: &mut Self::Renderer,
        surface: &mut Self::Surface,
        viewport: &Viewport,
        background_color: Color,
        on_pre_present: impl FnOnce(),
    ) -> Result<(), compositor::SurfaceError> {
        present(
            renderer,
            surface,
            viewport,
            background_color,
            on_pre_present,
        )
    }

    fn screenshot(
        &mut self,
        renderer: &mut Self::Renderer,
        viewport: &Viewport,
        background_color: Color,
    ) -> Vec<u8> {
        screenshot(renderer, viewport, background_color)
    }

    fn warm_up_offscreen(
        &mut self,
        _renderer: &mut Self::Renderer,
        _viewport: &Viewport,
        _background_color: Color,
    ) -> Result<compositor::OffscreenWarmUpEvidence, compositor::OffscreenWarmUpError> {
        Err(compositor::OffscreenWarmUpError::Unsupported)
    }
}

pub fn new(display: impl compositor::Display) -> Compositor {
    #[allow(unsafe_code)]
    let context =
        softbuffer::Context::new(Box::new(display) as _).expect("Create softbuffer context");

    Compositor { context }
}

pub fn present(
    renderer: &mut Renderer,
    surface: &mut Surface,
    viewport: &Viewport,
    background: Color,
    on_pre_present: impl FnOnce(),
) -> Result<(), compositor::SurfaceError> {
    let present_started = Instant::now();
    let present_delta_us = present_frame_delta_us(present_started);
    let physical_size = viewport.physical_size();

    let mut buffer = surface
        .window
        .buffer_mut()
        .map_err(|_| compositor::SurfaceError::Lost)?;

    let last_frame = {
        let age = buffer.age();

        surface.max_age = surface.max_age.max(age);
        surface.frames.truncate(surface.max_age as usize);

        if age > 0 {
            surface.frames.get(age as usize - 1)
        } else {
            None
        }
    };

    let scroll_started = Instant::now();
    let layer_scrolls = last_frame
        .filter(|last_frame| last_frame.background == background)
        .map(|last_frame| {
            last_frame
                .layers
                .iter()
                .zip(renderer.layers())
                .map(|(previous, current)| Layer::scroll(previous, current))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let scrolls = damage::collect_scrolls(layer_scrolls.iter().copied().flatten());
    let scroll_us = scroll_started.elapsed().as_micros();

    let damage_started = Instant::now();
    let damage_summary = RefCell::new(crate::layer::DamageSummary::default());
    let layer_index = Cell::new(0);
    let damage = last_frame
        .and_then(|last_frame| {
            (last_frame.background == background).then(|| {
                damage::diff(
                    &last_frame.layers,
                    renderer.layers(),
                    |layer| vec![layer.bounds],
                    |previous, current| {
                        let index = layer_index.get();
                        layer_index.set(index + 1);
                        let scroll = layer_scrolls[index].filter(|scroll| scrolls.contains(scroll));
                        let (damage, summary) =
                            Layer::damage_with_scroll_summary(previous, current, scroll);
                        damage_summary.borrow_mut().extend(summary);
                        damage
                    },
                )
            })
        })
        .unwrap_or_else(|| vec![Rectangle::with_size(viewport.logical_size())]);
    let damage_us = damage_started.elapsed().as_micros();
    let raw_damage_count = damage.len();
    let snapshot_started = Instant::now();
    let snapshot_us;
    let mut grouping_us = 0;

    if damage.is_empty() {
        if let Some(last_frame) = last_frame {
            surface.frames.push_front(last_frame.clone());
        }
        snapshot_us = snapshot_started.elapsed().as_micros();
    } else {
        surface.frames.push_front(Frame {
            background,
            layers: Arc::from(renderer.layers()),
        });
        snapshot_us = snapshot_started.elapsed().as_micros();

        let grouping_started = Instant::now();
        let bounds = Rectangle::with_size(viewport.logical_size());
        let grouped_damage = damage::group(damage, bounds);
        let grouped_damage_count = grouped_damage.len();
        let grouped_damage_area = grouped_damage.iter().map(Rectangle::area).sum::<f32>();
        let damage_summary = *damage_summary.borrow();
        let (damage, damage_strategy) =
            damage::collapse_fragmented(grouped_damage, bounds, damage_summary);
        let damage_area = damage.iter().map(Rectangle::area).sum::<f32>();
        grouping_us = grouping_started.elapsed().as_micros();

        if trace::enabled() {
            trace::event(
                "tiny_skia_damage_regions",
                0,
                format_args!("final={} regions={}", damage.len(), format_rects(&damage)),
            );
        }

        let mut pixels = tiny_skia::PixmapMut::from_bytes(
            bytemuck::cast_slice_mut(&mut buffer),
            physical_size.width,
            physical_size.height,
        )
        .expect("Create pixel map");

        let raster_started = Instant::now();
        let draw_stats = renderer.draw_with_scroll(
            &mut pixels,
            &mut surface.clip_mask,
            viewport,
            &damage,
            background,
            &scrolls,
        );
        trace::event(
            "tiny_skia_present_draw",
            raster_started.elapsed().as_micros(),
            format_args!(
                "physical={}x{} logical={:.1}x{:.1} scale={:.2} raw_damage={} grouped_damage={grouped_damage_count} grouped_damage_area={grouped_damage_area:.1} damage_strategy={} damage_area={damage_area:.1} scrolls={} damage_quads={} damage_text={} damage_primitives={} damage_images={} damage_scroll={} draw_regions={} draw_layer_visits={} draw_quads={} draw_primitives={} draw_images={} draw_text_groups={} draw_text_items={} clip_mask_rebuilds={} clip_mask_reuses={} paragraph_raster_hits={} paragraph_raster_misses={} paragraph_raster_bypasses={} glyph_hits={} glyph_misses={}",
                physical_size.width,
                physical_size.height,
                viewport.logical_size().width,
                viewport.logical_size().height,
                viewport.scale_factor(),
                raw_damage_count,
                damage_strategy.as_str(),
                scrolls.len(),
                damage_summary.quads,
                damage_summary.text,
                damage_summary.primitives,
                damage_summary.images,
                damage_summary.scroll,
                draw_stats.damage_regions,
                draw_stats.layer_visits,
                draw_stats.quads,
                draw_stats.primitives,
                draw_stats.images,
                draw_stats.text_groups,
                draw_stats.text_items,
                draw_stats.clip_mask.rebuilds,
                draw_stats.clip_mask.reuses,
                draw_stats.text_pipeline.paragraph_raster_hits,
                draw_stats.text_pipeline.paragraph_raster_misses,
                draw_stats.text_pipeline.paragraph_raster_bypasses,
                draw_stats.text_pipeline.glyph_hits,
                draw_stats.text_pipeline.glyph_misses,
            ),
        );
    }

    on_pre_present();
    let os_present_started = Instant::now();
    let result = buffer.present().map_err(|_| compositor::SurfaceError::Lost);
    trace::event(
        "tiny_skia_present",
        present_started.elapsed().as_micros(),
        format_args!(
            "damage_us={damage_us} scroll_us={scroll_us} snapshot_us={snapshot_us} grouping_us={grouping_us} os_present_us={} frame_delta_us={} frame_fps={:.1} physical={}x{} scale={:.2} result={}",
            os_present_started.elapsed().as_micros(),
            present_delta_us
                .map(|delta| delta.to_string())
                .unwrap_or_else(|| String::from("first")),
            present_delta_us
                .map(|delta| 1_000_000.0 / delta.max(1) as f64)
                .unwrap_or(0.0),
            physical_size.width,
            physical_size.height,
            viewport.scale_factor(),
            if result.is_ok() { "ok" } else { "lost" },
        ),
    );

    result
}

fn present_frame_delta_us(now: Instant) -> Option<u128> {
    static LAST_PRESENT: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();

    let Ok(mut last_present) = LAST_PRESENT.get_or_init(|| Mutex::new(None)).lock() else {
        return None;
    };

    let delta = last_present.map(|last| now.duration_since(last).as_micros());
    *last_present = Some(now);
    delta
}

pub fn screenshot(
    renderer: &mut Renderer,
    viewport: &Viewport,
    background_color: Color,
) -> Vec<u8> {
    let size = viewport.physical_size();

    let mut offscreen_buffer: Vec<u32> = vec![0; size.width as usize * size.height as usize];

    let mut clip_mask = tiny_skia::Mask::new(size.width, size.height).expect("Create clip mask");

    renderer.draw(
        &mut tiny_skia::PixmapMut::from_bytes(
            bytemuck::cast_slice_mut(&mut offscreen_buffer),
            size.width,
            size.height,
        )
        .expect("Create offscreen pixel map"),
        &mut clip_mask,
        viewport,
        &[Rectangle::with_size(Size::new(
            size.width as f32,
            size.height as f32,
        ))],
        background_color,
    );

    offscreen_buffer.iter().fold(
        Vec::with_capacity(offscreen_buffer.len() * 4),
        |mut acc, pixel| {
            const A_MASK: u32 = 0xFF_00_00_00;
            const R_MASK: u32 = 0x00_FF_00_00;
            const G_MASK: u32 = 0x00_00_FF_00;
            const B_MASK: u32 = 0x00_00_00_FF;

            let a = ((A_MASK & pixel) >> 24) as u8;
            let r = ((R_MASK & pixel) >> 16) as u8;
            let g = ((G_MASK & pixel) >> 8) as u8;
            let b = (B_MASK & pixel) as u8;

            acc.extend([r, g, b, a]);
            acc
        },
    )
}

fn format_rects(rects: &[Rectangle]) -> String {
    rects
        .iter()
        .map(|rect| {
            format!(
                "[{:.1},{:.1},{:.1},{:.1}]",
                rect.x, rect.y, rect.width, rect.height
            )
        })
        .collect::<Vec<_>>()
        .join(";")
}
