//! Diffuse color and slow light rays for the Info header.

use std::cell::RefCell;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use iced::advanced::image::{self, Renderer as _};
use iced::advanced::widget::{Tree, tree};
use iced::advanced::{Layout, Shell, Widget, layout, mouse, renderer};
use iced::{Element, Event, Length, Rectangle, Renderer, Size, Theme, window};

use crate::message::Message;

const FRAME_INTERVAL: Duration = Duration::from_nanos(41_666_667);
const HEADER_HEIGHT: f32 = 84.0;
const QUIET_WIDTH: f32 = 260.0;
const ART_WIDTH: f32 = 280.0;
const QUILL_WIDTH: f32 = 100.0;
const QUILL_HEIGHT: f32 = 110.0;
const QUILL_RIGHT_INSET: f32 = 12.0;
const QUILL_PIXELS: &[u8] = include_bytes!("../../assets/illustrations/macaw-quill.rgba");
static QUILL: LazyLock<image::Handle> =
    LazyLock::new(|| image::Handle::from_rgba(400, 440, QUILL_PIXELS));

pub fn view(progress: f32, running: bool) -> Element<'static, Message> {
    Element::new(InfoVfx {
        progress: progress.clamp(0.0, 1.0),
        running,
    })
}

struct InfoVfx {
    progress: f32,
    running: bool,
}

struct State {
    enabled: bool,
    focused: bool,
    zero_sized: bool,
    elapsed: f64,
    last_tick: Option<Instant>,
    next_tick: Option<Instant>,
    image: RefCell<Option<CachedField>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct FieldKey {
    phase: u64,
    width: u32,
    height: u32,
    dark: bool,
}

struct CachedField {
    key: FieldKey,
    handle: image::Handle,
}

impl State {
    fn pause(&mut self) {
        self.last_tick = None;
        self.next_tick = None;
    }
}

impl Widget<Message, Theme, Renderer> for InfoVfx {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(HEADER_HEIGHT))
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::atomic(limits, Length::Fill, HEADER_HEIGHT)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        if self.progress <= 0.0 {
            return;
        }
        let bounds = layout.bounds();
        let Some(clip) = visible_art(bounds, *viewport) else {
            return;
        };
        let art = art_bounds(bounds);
        let state = tree.state.downcast_ref::<State>();
        // This deliberately low-resolution field has no sharp details. Linear
        // sampling stays diffuse at every DPI while keeping generation bounded.
        let key = FieldKey {
            phase: state.elapsed.to_bits(),
            width: (art.width * 0.5).ceil().clamp(8.0, 160.0) as u32,
            height: (art.height * 0.5).ceil().clamp(8.0, 48.0) as u32,
            dark: theme.palette().is_dark,
        };
        let mut cached = state.image.borrow_mut();
        if cached.as_ref().is_none_or(|field| field.key != key) {
            *cached = Some(CachedField {
                key,
                handle: image::Handle::from_rgba(key.width, key.height, render_field(key)),
            });
        }
        if let Some(field) = cached.as_ref() {
            renderer.draw_image(
                image::Image::new(&field.handle)
                    .filter_method(image::FilterMethod::Linear)
                    .opacity(self.progress),
                art,
                clip,
            );
        }
        if bounds.width < QUIET_WIDTH + QUILL_WIDTH + QUILL_RIGHT_INSET {
            return;
        }
        let quill_bounds = Rectangle {
            x: bounds.x + bounds.width - QUILL_WIDTH - QUILL_RIGHT_INSET,
            y: bounds.y + (HEADER_HEIGHT - QUILL_HEIGHT) * 0.5,
            width: QUILL_WIDTH,
            height: QUILL_HEIGHT,
        };
        // Let the feather overlap the header's padding above and below the
        // glow. The original 4× artwork stays sharp across desktop scales and
        // shares one image handle across themes, phases, and dialog instances.
        // The width guard keeps it entirely clear of the title.
        if let Some(clip) = quill_bounds.intersection(viewport) {
            renderer.draw_image(
                image::Image::new(&*QUILL)
                    .filter_method(image::FilterMethod::Linear)
                    .opacity(self.progress),
                quill_bounds,
                clip,
            );
        }
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State {
            enabled: self.running && self.progress > 0.0,
            focused: true,
            zero_sized: false,
            elapsed: 0.0,
            last_tick: None,
            next_tick: None,
            image: RefCell::new(None),
        })
    }

    fn diff(&mut self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<State>();
        let enabled = self.running && self.progress > 0.0;
        if enabled != state.enabled {
            state.enabled = enabled;
            state.pause();
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        match event {
            Event::Window(window::Event::Unfocused | window::Event::Closed) => {
                state.focused = false;
                state.pause();
            }
            Event::Window(window::Event::Focused)
            | Event::Mouse(mouse::Event::ButtonPressed(_))
            | Event::Keyboard(iced::keyboard::Event::KeyPressed { .. })
            | Event::Touch(iced::touch::Event::FingerPressed { .. }) => {
                if !state.focused {
                    state.focused = true;
                    state.pause();
                }
            }
            Event::Window(window::Event::Resized(size)) => {
                state.zero_sized = size.width <= 0.0 || size.height <= 0.0;
                state.pause();
            }
            _ => {}
        }

        if !self.running
            || self.progress <= 0.0
            || !state.focused
            || state.zero_sized
            || visible_art(layout.bounds(), *viewport).is_none()
        {
            state.pause();
            return;
        }

        let now = match event {
            Event::Window(window::Event::RedrawRequested(now)) => *now,
            _ => Instant::now(),
        };
        if let Event::Window(window::Event::RedrawRequested(_)) = event {
            if state.next_tick.is_none_or(|deadline| now >= deadline) {
                if let Some(previous) = state.last_tick {
                    // If presentation stalls without a focus/resize event,
                    // do not jump the light field forward on its return.
                    state.elapsed += now
                        .saturating_duration_since(previous)
                        .min(FRAME_INTERVAL * 2)
                        .as_secs_f64();
                }
                state.last_tick = Some(now);
                state.next_tick = Some(now + FRAME_INTERVAL);
            }
        }
        let deadline = *state.next_tick.get_or_insert(now + FRAME_INTERVAL);
        shell.request_redraw_at(deadline);
    }
}

fn art_bounds(bounds: Rectangle) -> Rectangle {
    let width = (bounds.width - QUIET_WIDTH).clamp(0.0, ART_WIDTH);
    Rectangle {
        x: bounds.x + bounds.width - width,
        width,
        ..bounds
    }
}

fn visible_art(bounds: Rectangle, viewport: Rectangle) -> Option<Rectangle> {
    let art = art_bounds(bounds);
    if art.width < 16.0 || art.height <= 0.0 {
        return None;
    }
    art.intersection(&viewport)
        .filter(|clip| clip.width > 0.0 && clip.height > 0.0)
}

fn smooth(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

/// Analytic Gaussian blooms and feathered rays, encoded as straight RGBA.
/// Only one small image is retained; opacity is applied later by the renderer.
fn render_field(key: FieldKey) -> Vec<u8> {
    let time = f64::from_bits(key.phase);
    let wave =
        |period: f64, offset: f64| ((time / period * std::f64::consts::TAU + offset).sin()) as f32;
    let centers = [
        (
            0.36 + 0.055 * wave(34.0, 0.0),
            0.48 + 0.10 * wave(29.0, 1.2),
            0.25,
            0.42,
        ),
        (
            0.67 + 0.065 * wave(41.0, 2.1),
            0.32 + 0.11 * wave(37.0, 0.6),
            0.23,
            0.41,
        ),
        (
            0.79 + 0.035 * wave(31.0, 4.0),
            0.71 + 0.09 * wave(43.0, 2.8),
            0.28,
            0.38,
        ),
    ];
    let colors = if key.dark {
        [
            [244.0, 177.0, 150.0],
            [177.0, 154.0, 237.0],
            [135.0, 211.0, 220.0],
        ]
    } else {
        [
            [238.0, 153.0, 126.0],
            [166.0, 142.0, 222.0],
            [109.0, 185.0, 199.0],
        ]
    };
    let ray_centers = [
        0.48 + 0.035 * wave(32.0, 0.3),
        0.72 + 0.035 * wave(39.0, 1.8),
        0.93 + 0.030 * wave(46.0, 3.1),
    ];
    let slope = 0.28 + 0.025 * wave(38.0, 0.0);
    let mut pixels = vec![0; key.width as usize * key.height as usize * 4];
    for y in 0..key.height {
        let v = y as f32 / (key.height - 1) as f32;
        for x in 0..key.width {
            let u = x as f32 / (key.width - 1) as f32;
            let envelope = smooth(u / 0.38)
                * smooth((1.0 - u) / 0.16)
                * smooth(v / 0.28)
                * smooth((1.0 - v) / 0.28);
            let mut weight = 0.0;
            let mut rgb = [0.0; 3];
            for ((cx, cy, sx, sy), color) in centers.into_iter().zip(colors) {
                let dx = (u - cx) / sx;
                let dy = (v - cy) / sy;
                let bloom = (-0.5 * (dx * dx + dy * dy)).exp() * 0.58;
                weight += bloom;
                for channel in 0..3 {
                    rgb[channel] += color[channel] * bloom;
                }
            }
            for (index, center) in ray_centers.into_iter().enumerate() {
                let distance = (u + (v - 0.5) * slope - center) / (0.022 + index as f32 * 0.008);
                let ray = (-0.5 * distance * distance).exp() * 0.64;
                weight += ray;
                for channel in 0..3 {
                    // Colored light remains visible on a white dialog, while
                    // gently lifting each hue toward white keeps it luminous.
                    let light = if key.dark {
                        colors[index][channel] * 0.72 + 255.0 * 0.28
                    } else {
                        // A white surface needs more chroma to reveal the
                        // diagonal rays without darkening the diffuse band.
                        [
                            [238.0, 115.0, 80.0],
                            [139.0, 92.0, 223.0],
                            [46.0, 164.0, 193.0],
                        ][index][channel]
                    };
                    rgb[channel] += light * ray;
                }
            }
            let alpha = (1.0 - (-weight).exp()) * envelope * if key.dark { 0.66 } else { 0.56 };
            let index = ((y * key.width + x) * 4) as usize;
            for channel in 0..3 {
                pixels[index + channel] = (rgb[channel] / weight.max(0.0001))
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
            pixels[index + 3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Color;
    use iced::advanced::Renderer as _;
    use iced::advanced::graphics::core::shell::Waker;
    use iced::advanced::renderer::Headless;

    const BOUNDS: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 600.0,
        height: HEADER_HEIGHT,
    };

    fn renderer() -> Renderer {
        futures::executor::block_on(<Renderer as Headless>::new(
            renderer::Settings::default(),
            Some("tiny-skia"),
        ))
        .expect("CPU renderer")
    }

    fn mount(renderer: &Renderer) -> (Element<'static, Message>, Tree, layout::Node) {
        let mut widget = view(1.0, true);
        let mut tree = Tree::empty();
        tree.diff(widget.as_widget_mut());
        let node = widget.as_widget_mut().layout(
            &mut tree,
            renderer,
            &layout::Limits::new(Size::ZERO, BOUNDS.size()),
        );
        (widget, tree, node)
    }

    fn update(
        widget: &mut Element<'_, Message>,
        tree: &mut Tree,
        node: &layout::Node,
        renderer: &Renderer,
        event: Event,
        viewport: Rectangle,
    ) -> window::RedrawRequest {
        let mut messages = Vec::new();
        let mut shell = Shell::new(&window::Headless, Waker::noop(), &mut messages);
        widget.as_widget_mut().update(
            tree,
            &event,
            Layout::new(node),
            mouse::Cursor::Unavailable,
            renderer,
            &mut shell,
            &viewport,
        );
        assert!(!shell.are_widgets_invalid());
        assert!(shell.is_layout_invalid().is_none());
        assert!(shell.is_empty());
        shell.redraw_request()
    }

    #[test]
    fn deadline_throttles_external_and_duplicate_redraws_without_relayout() {
        let renderer = renderer();
        let (mut widget, mut tree, node) = mount(&renderer);
        let start = Instant::now();
        let tick = |at| Event::Window(window::Event::RedrawRequested(at));
        assert_eq!(
            update(
                &mut widget,
                &mut tree,
                &node,
                &renderer,
                tick(start),
                BOUNDS
            ),
            window::RedrawRequest::At(start + FRAME_INTERVAL)
        );
        assert_eq!(
            update(
                &mut widget,
                &mut tree,
                &node,
                &renderer,
                tick(start + FRAME_INTERVAL / 2),
                BOUNDS
            ),
            window::RedrawRequest::At(start + FRAME_INTERVAL)
        );
        assert_eq!(tree.state.downcast_ref::<State>().elapsed, 0.0);
        update(
            &mut widget,
            &mut tree,
            &node,
            &renderer,
            tick(start + FRAME_INTERVAL),
            BOUNDS,
        );
        let elapsed = tree.state.downcast_ref::<State>().elapsed;
        assert!((elapsed - FRAME_INTERVAL.as_secs_f64()).abs() < 1e-9);
        assert_eq!(
            update(
                &mut widget,
                &mut tree,
                &node,
                &renderer,
                tick(start + FRAME_INTERVAL),
                BOUNDS
            ),
            window::RedrawRequest::At(start + FRAME_INTERVAL * 2)
        );
        assert_eq!(tree.state.downcast_ref::<State>().elapsed, elapsed);
        assert_eq!(node.bounds(), BOUNDS);
    }

    #[test]
    fn pause_unfocus_clipping_and_zero_size_stop_frames_and_resume_without_jump() {
        let renderer = renderer();
        let (mut widget, mut tree, node) = mount(&renderer);
        let start = Instant::now();
        let tick = |at| Event::Window(window::Event::RedrawRequested(at));
        update(
            &mut widget,
            &mut tree,
            &node,
            &renderer,
            tick(start),
            BOUNDS,
        );
        update(
            &mut widget,
            &mut tree,
            &node,
            &renderer,
            tick(start + FRAME_INTERVAL),
            BOUNDS,
        );
        let elapsed = tree.state.downcast_ref::<State>().elapsed;
        assert_eq!(
            update(
                &mut widget,
                &mut tree,
                &node,
                &renderer,
                Event::Window(window::Event::Unfocused),
                BOUNDS
            ),
            window::RedrawRequest::Wait
        );
        assert_eq!(
            update(
                &mut widget,
                &mut tree,
                &node,
                &renderer,
                tick(start + Duration::from_secs(10)),
                BOUNDS
            ),
            window::RedrawRequest::Wait
        );
        update(
            &mut widget,
            &mut tree,
            &node,
            &renderer,
            Event::Window(window::Event::Focused),
            BOUNDS,
        );
        update(
            &mut widget,
            &mut tree,
            &node,
            &renderer,
            tick(start + Duration::from_secs(20)),
            BOUNDS,
        );
        assert_eq!(tree.state.downcast_ref::<State>().elapsed, elapsed);

        widget = view(1.0, false);
        tree.diff(widget.as_widget_mut());
        assert_eq!(
            update(
                &mut widget,
                &mut tree,
                &node,
                &renderer,
                tick(start + Duration::from_secs(30)),
                BOUNDS
            ),
            window::RedrawRequest::Wait
        );
        widget = view(1.0, true);
        tree.diff(widget.as_widget_mut());
        update(
            &mut widget,
            &mut tree,
            &node,
            &renderer,
            tick(start + Duration::from_secs(40)),
            BOUNDS,
        );
        assert_eq!(tree.state.downcast_ref::<State>().elapsed, elapsed);

        let hidden = Rectangle {
            x: 0.0,
            y: 100.0,
            ..BOUNDS
        };
        assert_eq!(
            update(
                &mut widget,
                &mut tree,
                &node,
                &renderer,
                tick(start + Duration::from_secs(50)),
                hidden
            ),
            window::RedrawRequest::Wait
        );
        update(
            &mut widget,
            &mut tree,
            &node,
            &renderer,
            tick(start + Duration::from_secs(60)),
            BOUNDS,
        );
        assert_eq!(tree.state.downcast_ref::<State>().elapsed, elapsed);
        assert_eq!(
            update(
                &mut widget,
                &mut tree,
                &node,
                &renderer,
                Event::Window(window::Event::Resized(Size::ZERO)),
                BOUNDS
            ),
            window::RedrawRequest::Wait
        );
        assert_eq!(
            update(
                &mut widget,
                &mut tree,
                &node,
                &renderer,
                tick(start + Duration::from_secs(70)),
                BOUNDS
            ),
            window::RedrawRequest::Wait
        );

        // Restore the resize gate so zero-opacity scheduling is tested on its own.
        update(
            &mut widget,
            &mut tree,
            &node,
            &renderer,
            Event::Window(window::Event::Resized(BOUNDS.size())),
            BOUNDS,
        );
        assert!(!tree.state.downcast_ref::<State>().zero_sized);
        widget = view(0.0, true);
        tree.diff(widget.as_widget_mut());
        assert_eq!(
            update(
                &mut widget,
                &mut tree,
                &node,
                &renderer,
                tick(start + Duration::from_secs(80)),
                BOUNDS
            ),
            window::RedrawRequest::Wait
        );
    }

    fn snapshot(
        renderer: &mut Renderer,
        theme: &Theme,
        progress: f32,
        elapsed: f64,
        scale: f32,
    ) -> Vec<u8> {
        renderer.hint(scale);
        let mut widget = view(progress, false);
        let mut tree = Tree::empty();
        tree.diff(widget.as_widget_mut());
        tree.state.downcast_mut::<State>().elapsed = elapsed;
        let node = widget.as_widget_mut().layout(
            &mut tree,
            renderer,
            &layout::Limits::new(Size::ZERO, BOUNDS.size()),
        );
        renderer.reset(BOUNDS);
        widget.as_widget().draw(
            &tree,
            renderer,
            theme,
            &renderer::Style::default(),
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &BOUNDS,
        );
        renderer.screenshot(
            Size::new(
                (BOUNDS.width * scale) as u32,
                (BOUNDS.height * scale) as u32,
            ),
            scale,
            Color::TRANSPARENT,
        )
    }

    #[test]
    fn light_field_cache_reuses_handles_and_invalidates_only_for_paint_changes() {
        let mut renderer = renderer();
        let (mut widget, mut tree, node) = mount(&renderer);
        let theme = crate::ui::styles::modern_theme(crate::core::AppearanceMode::Light).unwrap();
        let draw = |widget: &Element<'_, Message>,
                    tree: &Tree,
                    renderer: &mut Renderer,
                    theme: &Theme,
                    node: &layout::Node| {
            renderer.reset(BOUNDS);
            widget.as_widget().draw(
                tree,
                renderer,
                theme,
                &renderer::Style::default(),
                Layout::new(node),
                mouse::Cursor::Unavailable,
                &BOUNDS,
            );
            tree.state
                .downcast_ref::<State>()
                .image
                .borrow()
                .as_ref()
                .unwrap()
                .handle
                .id()
        };
        let first = draw(&widget, &tree, &mut renderer, &theme, &node);
        assert_eq!(first, draw(&widget, &tree, &mut renderer, &theme, &node));
        widget = view(0.4, false);
        tree.diff(widget.as_widget_mut());
        assert_eq!(first, draw(&widget, &tree, &mut renderer, &theme, &node));
        tree.state.downcast_mut::<State>().elapsed = 3.0;
        let moved = draw(&widget, &tree, &mut renderer, &theme, &node);
        assert_ne!(first, moved);
        let dark = crate::ui::styles::modern_theme(crate::core::AppearanceMode::Dark).unwrap();
        assert_ne!(moved, draw(&widget, &tree, &mut renderer, &dark, &node));
        let previous = draw(&widget, &tree, &mut renderer, &dark, &node);
        let narrow = layout::Node::new(Size::new(440.0, HEADER_HEIGHT));
        assert_ne!(
            previous,
            draw(&widget, &tree, &mut renderer, &dark, &narrow)
        );
        let field_before_dpi = draw(&widget, &tree, &mut renderer, &dark, &narrow);
        renderer.hint(1.5);
        assert_eq!(
            field_before_dpi,
            draw(&widget, &tree, &mut renderer, &dark, &narrow)
        );
    }

    #[test]
    fn quill_artwork_has_valid_dimensions_and_transparent_edges() {
        const WIDTH: usize = 400;
        const HEIGHT: usize = 440;
        assert_eq!(QUILL_PIXELS.len(), WIDTH * HEIGHT * 4);
        let alpha = |x: usize, y: usize| QUILL_PIXELS[(y * WIDTH + x) * 4 + 3];
        for x in 0..WIDTH {
            assert_eq!(alpha(x, 0), 0);
            assert_eq!(alpha(x, HEIGHT - 1), 0);
        }
        for y in 0..HEIGHT {
            assert_eq!(alpha(0, y), 0);
            assert_eq!(alpha(WIDTH - 1, y), 0);
        }
        assert!(QUILL_PIXELS.chunks_exact(4).any(|pixel| pixel[3] > 200));
        assert!(
            QUILL_PIXELS
                .chunks_exact(4)
                .any(|pixel| pixel[3] > 0 && pixel[3] < 255)
        );
        let image::Handle::Rgba {
            width,
            height,
            pixels,
            ..
        } = &*QUILL
        else {
            panic!("quill must use its original RGBA artwork");
        };
        assert_eq!((*width as usize, *height as usize), (WIDTH, HEIGHT));
        assert_eq!(pixels.len(), QUILL_PIXELS.len());
    }

    #[test]
    fn diffuse_field_is_straight_rgba_and_feathers_smoothly_at_every_edge() {
        for dark in [false, true] {
            let key = FieldKey {
                phase: 2.0_f64.to_bits(),
                width: 140,
                height: 42,
                dark,
            };
            let pixels = render_field(key);
            let alpha = |x: u32, y: u32| pixels[((y * key.width + x) * 4 + 3) as usize];
            for x in 0..key.width {
                assert_eq!(alpha(x, 0), 0);
                assert_eq!(alpha(x, key.height - 1), 0);
            }
            for y in 0..key.height {
                assert_eq!(alpha(0, y), 0);
                assert_eq!(alpha(key.width - 1, y), 0);
            }
            let mut largest_step = 0;
            for y in 1..key.height {
                for x in 1..key.width {
                    largest_step = largest_step.max(alpha(x, y).abs_diff(alpha(x - 1, y)));
                    largest_step = largest_step.max(alpha(x, y).abs_diff(alpha(x, y - 1)));
                }
            }
            assert!(
                largest_step <= 20,
                "diffuse field must not introduce hard alpha edges: {largest_step}"
            );
            assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] > 50));
            // Bright RGB under low alpha proves this is straight, not premultiplied.
            assert!(
                pixels
                    .chunks_exact(4)
                    .any(|pixel| pixel[3] > 0 && pixel[3] < 20 && pixel[0] > 100)
            );
        }
    }

    #[test]
    fn cpu_pixels_fade_to_exact_zero_leave_title_clear_and_change_deterministically() {
        let mut renderer = renderer();
        for appearance in [
            crate::core::AppearanceMode::Light,
            crate::core::AppearanceMode::Dark,
        ] {
            let theme = crate::ui::styles::modern_theme(appearance).unwrap();
            for scale in [1.0, 1.5] {
                let invisible = snapshot(&mut renderer, &theme, 0.0, 0.0, scale);
                assert!(invisible.chunks_exact(4).all(|pixel| pixel[3] == 0));
                let first = snapshot(&mut renderer, &theme, 1.0, 0.0, scale);
                let repeat = snapshot(&mut renderer, &theme, 1.0, 0.0, scale);
                let later = snapshot(&mut renderer, &theme, 1.0, 3.0, scale);
                assert_eq!(first, repeat);
                assert_ne!(first, later);
                assert!(first.chunks_exact(4).any(|pixel| pixel[3] > 0));
                let width = (BOUNDS.width * scale) as usize;
                let calm_width = (QUIET_WIDTH * scale) as usize;
                for row in first.chunks_exact(width * 4) {
                    assert!(
                        row[..calm_width * 4]
                            .chunks_exact(4)
                            .all(|pixel| pixel[3] == 0)
                    );
                }
                let partial = snapshot(&mut renderer, &theme, 0.4, 0.0, scale);
                let sum = |image: &[u8]| {
                    image
                        .chunks_exact(4)
                        .map(|pixel| pixel[3] as u64)
                        .sum::<u64>()
                };
                assert!(sum(&partial) > 0 && sum(&partial) < sum(&first));
            }
        }
    }
}
