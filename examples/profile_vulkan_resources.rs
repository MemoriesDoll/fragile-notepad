//! Offscreen About resource/timestamp measurements without image readback.
//! cargo run --release --features wgpu/counters --example profile_vulkan_resources
//! Add `-- --editor` to scroll the actual document editor over a Rust fixture.
//! Add `-- --plain-text` for a text document without syntax/fold decorations.
//! `--workload=selection|edit|long-lines|unicode|idle` exercises additional editor paths.
//! `--resize` changes the viewport; `--in-flight=3` measures bounded offscreen throughput.
//! Use `--features gpu-profiling -- --single-scene --frames=240 --trace-dir=target/gpu-trace`
//! to inspect API uploads separately from timing runs (tracing adds overhead).

#[cfg(not(feature = "hybrid-rendering"))]
fn main() {
    eprintln!("Requires hybrid-rendering; enable wgpu/counters for memory counters.");
}

#[cfg(feature = "hybrid-rendering")]
fn main() {
    profile::run();
}

#[cfg(feature = "hybrid-rendering")]
mod profile {
    use fragile_notepad::{message::AboutTab, ui::about_dialog};
    use iced::advanced::graphics::core::shell::Waker;
    use iced::advanced::renderer::Renderer as _;
    use iced::advanced::widget::Tree;
    use iced::advanced::{Layout, Shell, layout, mouse, renderer};
    use iced::{Color, Event, Rectangle, Renderer, Size, Theme, window};
    use iced_wgpu::graphics::{Shell as GraphicsShell, Viewport};
    use std::time::{Duration, Instant};

    fn target(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        size: Size<u32>,
    ) -> wgpu::TextureView {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Profile target"),
                size: wgpu::Extent3d {
                    width: size.width,
                    height: size.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default())
    }

    fn report(instance: &wgpu::Instance, device: &wgpu::Device, stage: &str) {
        let hal = device.get_internal_counters().hal;
        let hub = instance.generate_report().unwrap().hub;
        println!(
            "VULKAN_RESOURCES stage={stage} buffers={} textures={} bindings={} pipelines={} buffer_bytes={} texture_bytes={}",
            hub.buffers.num_kept_from_user,
            hub.textures.num_kept_from_user,
            hub.bind_groups.num_kept_from_user,
            hub.render_pipelines.num_kept_from_user,
            hal.buffer_memory.read(),
            hal.texture_memory.read(),
        );
        if let Some(allocator) = device.generate_allocator_report() {
            println!(
                "VULKAN_MEMORY stage={stage} allocated_bytes={} reserved_bytes={}",
                allocator.total_allocated_bytes, allocator.total_reserved_bytes
            );
            for allocation in allocator.allocations {
                if allocation.name.contains("atlas") {
                    println!(
                        "VULKAN_ALLOCATION stage={stage} name={:?} bytes={}",
                        allocation.name, allocation.size
                    );
                }
            }
        }
    }

    pub fn run() {
        let arguments: Vec<_> = std::env::args().collect();
        let single_scene = arguments.iter().any(|arg| arg == "--single-scene");
        let frames: u32 = arguments
            .iter()
            .find_map(|arg| arg.strip_prefix("--frames="))
            .map(|value| value.parse().expect("--frames must be an integer"))
            .unwrap_or(120);
        assert!(frames > 12, "--frames must exceed the 12 warm-up frames");
        let in_flight: u32 = arguments
            .iter()
            .find_map(|arg| arg.strip_prefix("--in-flight="))
            .map(|value| value.parse().expect("--in-flight must be an integer"))
            .unwrap_or(1);
        assert!((1..=8).contains(&in_flight), "--in-flight must be 1..=8");
        let resize = arguments.iter().any(|arg| arg == "--resize");
        let trace_path = arguments
            .iter()
            .find_map(|arg| arg.strip_prefix("--trace-dir="));
        #[cfg(feature = "gpu-profiling")]
        let trace = trace_path.map_or(wgpu::Trace::Off, |path| {
            std::fs::create_dir(path).expect("trace directory must be new with an existing parent");
            wgpu::Trace::Directory(path.into())
        });
        #[cfg(not(feature = "gpu-profiling"))]
        let trace = {
            assert!(
                trace_path.is_none(),
                "API tracing requires --features gpu-profiling"
            );
            wgpu::Trace::Off
        };
        let workload = arguments
            .iter()
            .find_map(|arg| arg.strip_prefix("--workload="))
            .unwrap_or_else(|| {
                if arguments.iter().any(|arg| arg == "--plain-text") {
                    "plain-scroll"
                } else if arguments.iter().any(|arg| arg == "--editor") {
                    "rust-scroll"
                } else {
                    "about"
                }
            });
        assert!(
            [
                "about",
                "rust-scroll",
                "plain-scroll",
                "selection",
                "edit",
                "long-lines",
                "unicode",
                "idle"
            ]
            .contains(&workload),
            "unknown workload"
        );
        let plain_text = workload != "rust-scroll" && workload != "about";
        let editor_mode = workload != "about";
        let frame_interval =
            Duration::from_nanos(if editor_mode { 41_666_667 } else { 16_666_667 });
        let source = editor_mode.then(|| {
            if workload == "long-lines" {
                format!("{}\n", "long line with tabs\tand columns | ".repeat(500)).repeat(128)
            } else if workload == "unicode" {
                "中文 日本語 한국어 café e\u{301} Ελληνικά العربية עברית 👩‍💻 🐇\n".repeat(12_000)
            } else if plain_text {
                "A plain text document with enough content for scrolling and glyph uploads.\n"
                    .repeat(12_000)
            } else {
                include_str!("../src/editor/widget.rs").repeat(12)
            }
        });
        let settings = fragile_notepad::core::EditorSettings::default();
        println!("VULKAN_WORKLOAD {workload} in_flight={in_flight} resize={resize}");
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter =
            futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: if arguments.iter().any(|arg| arg == "--low-power") {
                    wgpu::PowerPreference::LowPower
                } else {
                    wgpu::PowerPreference::HighPerformance
                },
                ..Default::default()
            }))
            .expect("Vulkan adapter");
        println!("VULKAN_ADAPTER {:?}", adapter.get_info());
        let timestamps =
            wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        let supported = in_flight == 1 && adapter.features().contains(timestamps);
        let immediate_size = if !arguments.iter().any(|arg| arg == "--uniforms")
            && adapter.features().contains(wgpu::Features::IMMEDIATES)
        {
            adapter.limits().max_immediate_size.min(128)
        } else {
            0
        };
        let mut features = if supported {
            timestamps
        } else {
            wgpu::Features::empty()
        };
        if immediate_size > 0 {
            features |= wgpu::Features::IMMEDIATES;
        }
        println!(
            "VULKAN_PARAMETERS {}",
            if immediate_size >= 16 {
                "push_constants"
            } else {
                "uniform_buffer"
            }
        );
        let (device, queue) =
            futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                required_features: features,
                required_limits: wgpu::Limits {
                    max_immediate_size: immediate_size,
                    ..Default::default()
                },
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace,
                ..Default::default()
            }))
            .unwrap();
        let queries = supported.then(|| {
            device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("About frame timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            })
        });
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Timestamp resolve"),
            size: 16,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Timestamp readback"),
            size: 16,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let engine = iced_wgpu::Engine::new(
            &adapter,
            device.clone(),
            queue.clone(),
            format,
            None,
            GraphicsShell::headless(),
        );
        report(&instance, &device, "engine");
        let start = Instant::now();
        for &windows in if single_scene { &[1][..] } else { &[1, 2][..] } {
            let mut scenes: Vec<_> = (0..windows)
                .map(|index| {
                    let renderer = Renderer::Primary(iced_wgpu::Renderer::new(
                        engine.clone(),
                        renderer::Settings::default(),
                    ));
                    let document = source.as_ref().map(|source| {
                        let mut document = fragile_notepad::core::Document::from_path(
                            fragile_notepad::core::DocumentId::new(index as u64 + 1),
                            if plain_text {
                                "profile.txt"
                            } else {
                                "profile.rs"
                            },
                            source,
                        );
                        // Loaded application documents defer analysis to workers.
                        document.defer_analysis = true;
                        document
                    });
                    (renderer, Tree::empty(), document)
                })
                .collect();
            report(&instance, &device, &format!("{windows}_windows_created"));
            let scales: &[f32] = if single_scene {
                &[1.0]
            } else {
                &[1.0, 1.5, 2.0]
            };
            for (scale_index, &scale) in scales.iter().enumerate() {
                let mut size = Size::new(900.0, 640.0);
                let mut bounds = Rectangle::with_size(size);
                let physical = Size::new((size.width * scale) as u32, (size.height * scale) as u32);
                let mut viewport = Viewport::with_physical_size(physical, scale);
                let mut target_view = target(&device, format, physical);
                let mut recording = Vec::new();
                let mut encoding = Vec::new();
                let mut rendering = Vec::new();
                let mut measured_start = Instant::now();
                for frame in 0..frames {
                    if frame == 12 {
                        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                        measured_start = Instant::now();
                    }
                    if resize && frame % 60 == 0 {
                        size = match (frame / 60) % 3 {
                            0 => Size::new(900.0, 640.0),
                            1 => Size::new(740.0, 480.0),
                            _ => Size::new(1080.0, 720.0),
                        };
                        bounds = Rectangle::with_size(size);
                        let physical =
                            Size::new((size.width * scale) as u32, (size.height * scale) as u32);
                        viewport = Viewport::with_physical_size(physical, scale);
                        target_view = target(&device, format, physical);
                    }
                    for (index, (renderer, tree, document)) in scenes.iter_mut().enumerate() {
                        let mut content = if let Some(document) = document {
                            use fragile_notepad::editor::{
                                EditorPosition, EditorRange, EditorSelection,
                            };
                            match workload {
                                "idle" => {}
                                "selection" => document.set_main_selection(EditorSelection::new(
                                    EditorPosition::new(2, 3),
                                    EditorPosition::new(3 + (frame as usize % 18), 25),
                                )),
                                "edit" => {
                                    let position = EditorPosition::new(4, 0);
                                    let _ = document.buffer.replace_range(
                                        EditorRange::new(position, EditorPosition::new(4, 1)),
                                        if frame % 2 == 0 { "A" } else { "B" },
                                    );
                                    document.refresh_text_lines(4, 4);
                                }
                                "long-lines" => {
                                    document.scroll.first_visible_row = frame as usize % 80;
                                    document.scroll.horizontal_px = (frame % 160) as f32 * 9.0;
                                }
                                _ => {
                                    document.scroll.first_visible_row =
                                        ((frame as usize + scale_index * frames as usize) * 3)
                                            % document
                                                .viewport
                                                .visible_row_count()
                                                .saturating_sub(40)
                                                .max(1)
                                }
                            }
                            fragile_notepad::ui::editor::view(document, &settings)
                        } else {
                            about_dialog::view(
                                AboutTab::About,
                                about_dialog::RenderingDebugInfo {
                                    current_renderer: "Vulkan".into(),
                                    rendering_policy: "Automatic".into(),
                                    title_bar_style:
                                        fragile_notepad::ui::title_bar::ControlStyle::Windows,
                                },
                                1.0,
                                true,
                            )
                        };
                        tree.diff(content.as_widget_mut());
                        renderer.hint(scale);
                        let node = content.as_widget_mut().layout(
                            tree,
                            renderer,
                            &layout::Limits::new(size, size),
                        );
                        let mut messages = Vec::new();
                        let mut shell = Shell::new(&window::Headless, Waker::noop(), &mut messages);
                        content.as_widget_mut().update(
                            tree,
                            &Event::Window(window::Event::RedrawRequested(
                                start + frame_interval * (frame + frames * scale_index as u32),
                            )),
                            Layout::new(&node),
                            mouse::Cursor::Unavailable,
                            renderer,
                            &mut shell,
                            &bounds,
                        );
                        renderer.reset(bounds);
                        let recording_start = Instant::now();
                        content.as_widget().draw(
                            tree,
                            renderer,
                            if index == 0 {
                                &Theme::Light
                            } else {
                                &Theme::Dark
                            },
                            &renderer::Style::default(),
                            Layout::new(&node),
                            mouse::Cursor::Unavailable,
                            &bounds,
                        );
                        let record_us = recording_start.elapsed().as_secs_f64() * 1e6;
                        let Renderer::Primary(renderer) = renderer else {
                            unreachable!()
                        };
                        let mut before = device.create_command_encoder(&Default::default());
                        before.insert_debug_marker(&format!(
                            "profile frame={frame} window={index} scale={scale}"
                        ));
                        if let Some(queries) = &queries {
                            before.write_timestamp(queries, 0);
                        }
                        let encode_start = Instant::now();
                        let commands = renderer.draw(Some(Color::WHITE), &target_view, &viewport);
                        let encode_us = encode_start.elapsed().as_secs_f64() * 1e6;
                        let mut after = device.create_command_encoder(&Default::default());
                        if let Some(queries) = &queries {
                            after.write_timestamp(queries, 1);
                            after.resolve_query_set(queries, 0..2, &resolve, 0);
                            after.copy_buffer_to_buffer(&resolve, 0, &readback, 0, 16);
                        }
                        renderer.finish();
                        queue.submit([before.finish(), commands.finish(), after.finish()]);
                        renderer.recall();
                        let gpu_us = if supported {
                            let (sender, receiver) = std::sync::mpsc::channel();
                            readback
                                .slice(..)
                                .map_async(wgpu::MapMode::Read, move |result| {
                                    sender.send(result).unwrap();
                                });
                            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                            receiver.recv().unwrap().unwrap();
                            let data = readback.slice(..).get_mapped_range();
                            let first = u64::from_ne_bytes(data[..8].try_into().unwrap());
                            let last = u64::from_ne_bytes(data[8..].try_into().unwrap());
                            let elapsed = last.wrapping_sub(first) as f64
                                * queue.get_timestamp_period() as f64
                                / 1000.0;
                            drop(data);
                            readback.unmap();
                            elapsed
                        } else {
                            if ((frame as usize * windows + index + 1) as u32)
                                .is_multiple_of(in_flight)
                            {
                                device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                            }
                            f64::NAN
                        };
                        if frame >= 12 {
                            recording.push(record_us);
                            encoding.push(encode_us);
                            rendering.push(gpu_us);
                        }
                    }
                    if frame == 12 {
                        report(
                            &instance,
                            &device,
                            &format!("{windows}_windows_scale_{scale}_warm"),
                        );
                    }
                }
                device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                let seconds = measured_start.elapsed().as_secs_f64();
                println!(
                    "VULKAN_THROUGHPUT windows={windows} scale={scale} in_flight={in_flight} frames={} seconds={seconds:.6} fps={:.2}",
                    (frames - 12) as usize * windows,
                    (frames - 12) as f64 * windows as f64 / seconds
                );
                recording.sort_by(f64::total_cmp);
                encoding.sort_by(f64::total_cmp);
                rendering.sort_by(f64::total_cmp);
                let mid = recording.len() / 2;
                let p95 = recording.len() * 95 / 100;
                println!(
                    "VULKAN_TIMING windows={windows} scale={scale} record_median_us={:.2} record_p95_us={:.2} encode_median_us={:.2} encode_p95_us={:.2} gpu_median_us={:.2} gpu_p95_us={:.2}",
                    recording[mid],
                    recording[p95],
                    encoding[mid],
                    encoding[p95],
                    rendering[mid],
                    rendering[p95]
                );
                report(
                    &instance,
                    &device,
                    &format!("{windows}_windows_scale_{scale}_steady"),
                );
            }
            drop(scenes);
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            report(&instance, &device, &format!("{windows}_windows_closed"));
        }
        drop(engine);
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        report(&instance, &device, "engine_released");
    }
}
