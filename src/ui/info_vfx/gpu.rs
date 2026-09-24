//! GPU-resident trail with 16-byte Vulkan push constants where supported.
//! Otherwise, reuse one uniform and binding per widget.
//! Draws in Iced's existing pass, without textures, extra submissions or readbacks.

use std::collections::HashMap;
use std::sync::{Arc, Weak};

use iced::Rectangle;
use iced::widget::shader::{self, Viewport};

#[derive(Debug)]
pub(super) struct Trail {
    pub instance: Instance,
    pub time: f32,
    pub opacity: f32,
    pub dark: bool,
}

/// Retain uniforms while the widget or a recorded frame still needs them.
/// Idle windows must not lose their resources as other windows render.
#[derive(Debug, Clone, Default)]
pub(super) struct Instance(Arc<()>);

impl Instance {
    fn key(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
}

struct Slot {
    owner: Weak<()>,
    uniform: wgpu::Buffer,
    binding: wgpu::BindGroup,
    last_parameters: Option<[f32; 4]>,
}

pub(super) struct Pipeline {
    raw: wgpu::RenderPipeline,
    layout: Option<wgpu::BindGroupLayout>,
    slots: HashMap<usize, Slot>,
    linear_target: bool,
}

impl shader::Pipeline for Pipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let immediates = device.features().contains(wgpu::Features::IMMEDIATES)
            && device.limits().max_immediate_size >= 16;
        let layout = (!immediates).then(|| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("About trail uniforms"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(16),
                    },
                    count: None,
                }],
            })
        });
        let layouts: Vec<_> = layout.as_ref().map(Some).into_iter().collect();
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("About trail pipeline layout"),
            bind_group_layouts: &layouts,
            immediate_size: if immediates { 16 } else { 0 },
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("About trail shader"),
            source: wgpu::ShaderSource::Wgsl(
                format!(
                    "{}\n{}",
                    if immediates {
                        "var<immediate> parameters: vec4<f32>;"
                    } else {
                        "@group(0) @binding(0) var<uniform> parameters: vec4<f32>;"
                    },
                    include_str!("trail.wgsl")
                )
                .into(),
            ),
        });
        let raw = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("About trail pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            raw,
            layout,
            slots: HashMap::new(),
            linear_target: format.is_srgb(),
        }
    }

    fn trim(&mut self) {
        self.slots.retain(|_, slot| slot.owner.strong_count() > 0);
    }
}

impl shader::Primitive for Trail {
    type Pipeline = Pipeline;

    fn prepare(
        &self,
        pipeline: &mut Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _bounds: &Rectangle,
        _viewport: &Viewport,
    ) {
        let Some(layout) = &pipeline.layout else {
            return;
        };
        let slot = pipeline
            .slots
            .entry(self.instance.key())
            .or_insert_with(|| {
                let uniform = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("About trail parameters"),
                    size: 16,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("About trail binding"),
                    layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    }],
                });
                Slot {
                    owner: Arc::downgrade(&self.instance.0),
                    uniform,
                    binding,
                    last_parameters: None,
                }
            });
        let parameters = self.parameters(pipeline.linear_target);
        if slot.last_parameters != Some(parameters) {
            queue.write_buffer(&slot.uniform, 0, &parameter_bytes(parameters));
            slot.last_parameters = Some(parameters);
        }
    }

    fn draw(&self, pipeline: &Pipeline, pass: &mut wgpu::RenderPass<'_>) -> bool {
        pass.set_pipeline(&pipeline.raw);
        if pipeline.layout.is_none() {
            pass.set_immediates(0, &parameter_bytes(self.parameters(pipeline.linear_target)));
        } else if let Some(slot) = pipeline.slots.get(&self.instance.key()) {
            pass.set_bind_group(0, &slot.binding, &[]);
        } else {
            return true;
        }
        pass.draw(0..3, 0..1);
        true
    }
}

impl Trail {
    fn parameters(&self, linear_target: bool) -> [f32; 4] {
        [
            self.time,
            self.opacity,
            u8::from(self.dark) as f32,
            u8::from(linear_target) as f32,
        ]
    }
}

fn parameter_bytes(parameters: [f32; 4]) -> [u8; 16] {
    let mut bytes = [0; 16];
    for (value, target) in parameters.into_iter().zip(bytes.chunks_exact_mut(4)) {
        target.copy_from_slice(&value.to_ne_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Size;
    use shader::{Pipeline as _, Primitive as _};

    fn device(immediates: bool) -> Option<(wgpu::Instance, wgpu::Device, wgpu::Queue)> {
        let vulkan = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let Some(adapter) =
            futures::executor::block_on(vulkan.request_adapter(&Default::default())).ok()
        else {
            eprintln!("Skipping Vulkan resource lifecycle validation: no Vulkan adapter");
            return None;
        };
        let Ok((device, queue)) =
            futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                required_features: if immediates {
                    wgpu::Features::IMMEDIATES
                } else {
                    wgpu::Features::empty()
                },
                required_limits: wgpu::Limits {
                    max_immediate_size: if immediates { 16 } else { 0 },
                    ..Default::default()
                },
                ..Default::default()
            }))
        else {
            eprintln!("Skipping Vulkan resource lifecycle validation: device unavailable");
            return None;
        };
        Some((vulkan, device, queue))
    }

    #[test]
    fn vulkan_push_constants_need_no_per_widget_gpu_allocations() {
        let Some((vulkan, device, queue)) = device(true) else {
            return;
        };
        let baseline = vulkan.generate_report().unwrap();
        let mut pipeline = Pipeline::new(&device, &queue, wgpu::TextureFormat::Rgba8Unorm);
        let bounds = Rectangle::with_size(Size::new(280.0, 96.0));
        let viewport = Viewport::with_physical_size(Size::new(640, 480), 1.0);
        for index in 0..240 {
            let trail = Trail {
                instance: Instance::default(),
                time: index as f32 / 24.0,
                opacity: 1.0,
                dark: false,
            };
            trail.prepare(&mut pipeline, &device, &queue, &bounds, &viewport);
            pipeline.trim();
        }
        assert!(pipeline.layout.is_none());
        assert!(pipeline.slots.is_empty());
        let report = vulkan.generate_report().unwrap();
        assert_eq!(report.hub.buffers, baseline.hub.buffers);
        assert_eq!(report.hub.bind_groups, baseline.hub.bind_groups);
        assert_eq!(report.hub.textures, baseline.hub.textures);
        assert_eq!(
            report.hub.render_pipelines.num_kept_from_user,
            baseline.hub.render_pipelines.num_kept_from_user + 1
        );
    }

    #[test]
    fn vulkan_resources_follow_widget_and_recorded_frame_lifetimes() {
        let Some((vulkan, device, queue)) = device(false) else {
            return;
        };
        let baseline = vulkan.generate_report().unwrap();
        let mut pipeline = Pipeline::new(&device, &queue, wgpu::TextureFormat::Rgba8Unorm);
        let widget = Instance::default();
        let other_widget = Instance::default();
        let mut trail = Trail {
            instance: widget.clone(),
            time: 0.0,
            opacity: 1.0,
            dark: false,
        };
        let bounds = Rectangle::with_size(Size::new(280.0, 96.0));
        let viewport = Viewport::with_physical_size(Size::new(640, 480), 1.0);
        trail.prepare(&mut pipeline, &device, &queue, &bounds, &viewport);
        let buffer = pipeline.slots[&widget.key()].uniform.clone();
        let binding = pipeline.slots[&widget.key()].binding.clone();

        // Another active window must not evict a paused window's uniforms.
        for frame in 0..240 {
            trail.instance = other_widget.clone();
            trail.time = frame as f32 / 24.0;
            trail.prepare(&mut pipeline, &device, &queue, &bounds, &viewport);
            pipeline.trim();
            assert_eq!(pipeline.slots[&widget.key()].uniform, buffer);
            assert_eq!(pipeline.slots[&widget.key()].binding, binding);
        }
        let report = vulkan.generate_report().unwrap();
        assert_eq!(
            report.hub.buffers.num_kept_from_user,
            baseline.hub.buffers.num_kept_from_user + 2
        );
        assert_eq!(
            report.hub.bind_groups.num_kept_from_user,
            baseline.hub.bind_groups.num_kept_from_user + 2
        );
        assert_eq!(
            report.hub.render_pipelines.num_kept_from_user,
            baseline.hub.render_pipelines.num_kept_from_user + 1
        );
        assert_eq!(report.hub.textures, baseline.hub.textures);

        drop(widget);
        drop(buffer);
        drop(binding);
        pipeline.trim();
        assert_eq!(pipeline.slots.len(), 1);
        drop(other_widget);
        // A recorded frame may outlive its widget during handoff or closing.
        pipeline.trim();
        assert_eq!(pipeline.slots.len(), 1);
        drop(trail);
        pipeline.trim();
        assert!(pipeline.slots.is_empty());
        drop(pipeline);
        queue.submit([]);
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let released = vulkan.generate_report().unwrap();
        assert_eq!(
            released.hub.buffers.num_kept_from_user,
            baseline.hub.buffers.num_kept_from_user
        );
        assert_eq!(
            released.hub.bind_groups.num_kept_from_user,
            baseline.hub.bind_groups.num_kept_from_user
        );
        assert_eq!(
            released.hub.render_pipelines.num_kept_from_user,
            baseline.hub.render_pipelines.num_kept_from_user
        );
    }
}
