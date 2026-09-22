use super::{Allocation, Atlas, Entry};

#[test]
fn large_uploads_release_staging_and_preserve_padded_fragment_pixels() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter =
        futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference:
                wgpu::PowerPreference::from_env().unwrap_or(wgpu::PowerPreference::HighPerformance),
            ..Default::default()
        }))
        .unwrap();
    eprintln!("UPLOAD_ADAPTER {:?}", adapter.get_info());
    let (device, queue) =
        futures::executor::block_on(adapter.request_device(&Default::default())).unwrap();
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2Array,
                multisampled: false,
            },
            count: None,
        }],
    });
    let mut atlas = Atlas::new(&device, wgpu::Backend::Vulkan, layout);
    let mut belt =
        wgpu::util::StagingBelt::new(device.clone(), crate::buffer::MAX_WRITE_SIZE as u64);
    let mut encoder = device.create_command_encoder(&Default::default());
    let mut entries = Vec::new();
    // Keep every upload in one pending encoder, including atlas growth. The
    // copies must remain valid after each upload function releases its buffer.
    for (width, height) in [(17, 8), (257, 130), (1100, 131)] {
        let pixels: Vec<u8> = (0..height)
            .flat_map(|y| (0..width).flat_map(move |x| pixel(x, y)))
            .collect();
        entries.push(
            atlas
                .upload(&device, &mut encoder, &mut belt, width, height, &pixels)
                .unwrap(),
        );
    }
    assert!(matches!(entries[2], Entry::Fragmented { .. }));
    belt.finish();
    let _ = queue.submit([encoder.finish()]);
    belt.recall();
    let _ = device.poll(wgpu::PollType::wait_indefinitely()).unwrap();

    for entry in &entries {
        match entry {
            Entry::Contiguous(allocation) => {
                assert_pixels(&device, &queue, &atlas, allocation, (0, 0));
            }
            Entry::Fragmented { fragments, .. } => {
                for fragment in fragments {
                    assert_pixels(
                        &device,
                        &queue,
                        &atlas,
                        &fragment.allocation,
                        fragment.position,
                    );
                }
            }
        }
    }
    assert_eq!(
        instance
            .generate_report()
            .unwrap()
            .hub
            .buffers
            .num_kept_from_user,
        1,
        "only the small reusable staging chunk should remain after upload completion"
    );

    drop(belt);
    drop(atlas);
    let _ = device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let report = instance.generate_report().unwrap().hub;
    assert_eq!(report.buffers.num_kept_from_user, 0);
    assert_eq!(report.textures.num_kept_from_user, 0);
}

fn pixel(x: u32, y: u32) -> [u8; 4] {
    [(x % 251) as u8, (y % 251) as u8, ((x + y) % 251) as u8, 255]
}

fn assert_pixels(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    atlas: &Atlas,
    allocation: &Allocation,
    source: (u32, u32),
) {
    let (x, y) = allocation.position();
    let size = allocation.size();
    let padding = allocation.padding();
    let width = size.width + 2 * padding.width;
    let height = size.height + 2 * padding.height;
    let stride = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(stride) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &atlas.storage.as_ref().unwrap().texture,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: x - padding.width,
                y: y - padding.height,
                z: allocation.layer() as u32,
            },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let _ = queue.submit([encoder.finish()]);
    let (sender, receiver) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });
    let _ = device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    receiver.recv().unwrap().unwrap();
    let bytes = readback.slice(..).get_mapped_range();
    for row in 0..height {
        for column in 0..width {
            let offset = (row * stride + column * 4) as usize;
            assert_eq!(
                &bytes[offset..offset + 4],
                &pixel(
                    source.0 + column.saturating_sub(padding.width).min(size.width - 1),
                    source.1 + row.saturating_sub(padding.height).min(size.height - 1),
                ),
                "fragment {source:?}, pixel ({column}, {row})"
            );
        }
    }
    drop(bytes);
    readback.unmap();
}
