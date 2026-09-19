#[cfg(not(feature = "hybrid-rendering"))]
mod profile {
    use iced::advanced::Renderer as _;
    use iced::advanced::{graphics, image, renderer, text};
    use iced::{Color, Font, Pixels, Rectangle, Size, alignment};
    use std::hint::black_box;
    use std::time::Instant;

    fn primitive(index: usize, repeated: bool, offset: f32) -> graphics::text::Text {
        graphics::text::Text::Cached {
            content: if repeated {
                "same repeated line".into()
            } else {
                format!("unique line {index}")
            },
            bounds: Rectangle {
                x: 0.0,
                y: index as f32 * 20.0 + offset,
                width: 200.0,
                height: 20.0,
            },
            color: Color::WHITE,
            size: Pixels(16.0),
            line_height: Pixels(20.0),
            font: Font::MONOSPACE,
            align_x: text::Alignment::Left,
            align_y: alignment::Vertical::Top,
            shaping: text::Shaping::Basic,
            wrapping: text::Wrapping::None,
            ellipsis: text::Ellipsis::None,
            clip_bounds: Rectangle {
                x: 0.0,
                y: 0.0,
                width: 1200.0,
                height: 10000.0,
            },
        }
    }

    fn scrolls() {
        for count in [36, 72, 144, 288] {
            for repeated in [false, true] {
                let previous: Vec<_> = (0..count).map(|i| primitive(i, repeated, 0.0)).collect();
                let current: Vec<_> = (0..count).map(|i| primitive(i, repeated, -20.0)).collect();
                let a: Vec<_> = previous.iter().collect();
                let b: Vec<_> = current.iter().collect();
                let samples = if count < 200 { 50 } else { 10 };
                let start = Instant::now();
                for _ in 0..samples {
                    black_box(graphics::text::scroll_delta(
                        black_box(&a),
                        black_box(&b),
                        10,
                    ));
                }
                println!(
                    "scroll count={count} repeated={repeated} samples={samples} avg_us={:.2} unchanged_result={:?}",
                    start.elapsed().as_secs_f64() * 1e6 / samples as f64,
                    graphics::text::scroll_delta(&a, &a, 10)
                );
            }
        }
    }

    fn images() {
        for (count, size) in [(1, 24), (100, 24), (100, 48), (1, 512)] {
            for filter in [image::FilterMethod::Nearest, image::FilterMethod::Linear] {
                let mut renderer = iced::Renderer::new(renderer::Settings::default());
                let viewport = graphics::Viewport::with_physical_size(Size::new(1200, 720), 1.0);
                let bounds = Rectangle::with_size(Size::new(1200.0, 720.0));
                let mut pixels = tiny_skia::Pixmap::new(1200, 720).unwrap();
                let mut mask = tiny_skia::Mask::new(1200, 720).unwrap();
                let handle = image::Handle::from_rgba(24, 24, [100, 150, 200, 200].repeat(24 * 24));
                renderer.reset(bounds);
                for i in 0..count {
                    image::Renderer::draw_image(
                        &mut renderer,
                        image::Image::new(handle.clone()).filter_method(filter),
                        Rectangle {
                            x: (i % 20) as f32 * size as f32,
                            y: (i / 20) as f32 * size as f32,
                            width: size as f32,
                            height: size as f32,
                        },
                        bounds,
                    );
                }
                for _ in 0..3 {
                    renderer.draw(
                        &mut pixels.as_mut(),
                        &mut mask,
                        &viewport,
                        &[bounds],
                        Color::BLACK,
                    );
                }
                let start = Instant::now();
                for _ in 0..100 {
                    renderer.draw(
                        &mut pixels.as_mut(),
                        &mut mask,
                        &viewport,
                        &[bounds],
                        Color::BLACK,
                    );
                }
                println!(
                    "images count={count} dest={size} filter={filter:?} samples=100 avg_us={:.2}",
                    start.elapsed().as_secs_f64() * 1e6 / 100.0
                );
            }
        }
    }

    fn damage() {
        for count in [16, 32, 64, 128, 256] {
            let regions: Vec<_> = (0..count)
                .map(|i| Rectangle {
                    x: (i % 16) as f32 * 200.0,
                    y: (i / 16) as f32 * 100.0,
                    width: 2.0,
                    height: 20.0,
                })
                .collect();
            let bounds = Rectangle::with_size(Size::new(3200.0, 1800.0));
            let start = Instant::now();
            for _ in 0..20 {
                black_box(graphics::damage::collapse_fragmented(
                    regions.clone(),
                    bounds,
                    graphics::damage::Summary {
                        text: count.max(32),
                        ..Default::default()
                    },
                ));
            }
            println!(
                "damage grouped_regions={count} samples=20 avg_us={:.2}",
                start.elapsed().as_secs_f64() * 1e6 / 20.0
            );
        }
    }

    pub fn run() {
        scrolls();
        images();
        damage();
    }
}

#[cfg(not(feature = "hybrid-rendering"))]
fn main() {
    profile::run();
}

#[cfg(feature = "hybrid-rendering")]
fn main() {
    eprintln!("Run cargo run --release --no-default-features --example profile_backend_hotspots");
}
