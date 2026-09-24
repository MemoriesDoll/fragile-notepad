use super::{
    Buffer, Cached, MAX_WRITE_SIZE, WIDE_INDEX_OFFSET, changed_range, index_buffer_offset,
};

#[test]
fn changed_spans_preserve_copy_alignment_and_float_bits() {
    let old = [0_u32, 1, f32::NAN.to_bits(), 0, 4, 5];
    let bytes = bytemuck::cast_slice(&old);
    assert_eq!(changed_range(bytes, bytes), 0..0);
    assert_eq!(changed_range(bytes, &[]), 0..0);
    assert_eq!(changed_range(&[], bytes), 0..24);
    assert_eq!(changed_range(bytes, &bytes[..12]), 0..0);
    assert_eq!(changed_range(&bytes[..12], bytes), 12..24);

    let mut new = old;
    new[3] = (-0.0_f32).to_bits();
    assert_eq!(changed_range(bytes, bytemuck::cast_slice(&new)), 12..16);
    new[1] = 99;
    assert_eq!(changed_range(bytes, bytemuck::cast_slice(&new)), 4..16);
    new[5] = 7;
    assert_eq!(changed_range(bytes, bytemuck::cast_slice(&new)), 4..24);
}

#[test]
fn retained_instances_survive_partial_writes_growth_and_pending_submissions() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let Some(adapter) =
        futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference:
                wgpu::PowerPreference::from_env().unwrap_or(wgpu::PowerPreference::HighPerformance),
            ..Default::default()
        }))
        .ok()
    else {
        eprintln!("Skipping Vulkan retained instances validation: no Vulkan adapter");
        return;
    };
    eprintln!("RETAINED_ADAPTER {:?}", adapter.get_info());
    let Ok((device, queue)) =
        futures::executor::block_on(adapter.request_device(&Default::default()))
    else {
        eprintln!("Skipping Vulkan retained instances validation: device unavailable");
        return;
    };
    let mut buffer = Cached::<u32>::new(
        &device,
        "retained instances test",
        8,
        wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
    );
    let mut expected: Vec<u32> = (0..8).collect();
    let mut belt = wgpu::util::StagingBelt::new(device.clone(), MAX_WRITE_SIZE as u64);
    let mut encoder = device.create_command_encoder(&Default::default());
    let _ = buffer.write(&mut encoder, &mut belt, 0, &expected);
    // Repartition the same storage as image nearest/linear groups do. Record
    // both updates and a readback before submitting the initial upload.
    expected[1] = 81;
    expected[6] = 86;
    let _ = buffer.write(&mut encoder, &mut belt, 0, &expected[..3]);
    let _ = buffer.write(&mut encoder, &mut belt, 12, &expected[3..]);
    assert_contents(&device, &queue, &buffer, &mut belt, encoder, &expected);
    drop(belt);
    let _ = device.poll(wgpu::PollType::wait_indefinitely()).unwrap();

    // An identical frame must not allocate even one staging buffer.
    let mut belt = wgpu::util::StagingBelt::new(device.clone(), MAX_WRITE_SIZE as u64);
    let mut encoder = device.create_command_encoder(&Default::default());
    let _ = buffer.write(&mut encoder, &mut belt, 0, &expected[..2]);
    let _ = buffer.write(&mut encoder, &mut belt, 8, &expected[2..]);
    assert_eq!(
        instance
            .generate_report()
            .unwrap()
            .hub
            .buffers
            .num_kept_from_user,
        1
    );
    assert_contents(&device, &queue, &buffer, &mut belt, encoder, &expected);

    // Shrinking the active prefix must not discard valid trailing bytes.
    buffer.resize(&device, 3);
    let mut encoder = device.create_command_encoder(&Default::default());
    expected[2] = 92;
    let _ = buffer.write(&mut encoder, &mut belt, 0, &expected[..3]);
    let _ = buffer.write(&mut encoder, &mut belt, 0, &expected);
    assert_contents(&device, &queue, &buffer, &mut belt, encoder, &expected);

    // A new GPU allocation must upload the formerly unchanged prefix too.
    expected.extend((8..(MAX_WRITE_SIZE / 4 + 33)).map(|value| value as u32));
    buffer.resize(&device, expected.len());
    let mut encoder = device.create_command_encoder(&Default::default());
    let _ = buffer.write(&mut encoder, &mut belt, 0, &expected);
    assert_contents(&device, &queue, &buffer, &mut belt, encoder, &expected);

    // Disjoint writes cannot accidentally mark an unknown gap as cached data.
    buffer.resize(&device, expected.len() * 4);
    let mut encoder = device.create_command_encoder(&Default::default());
    let _ = buffer.write(&mut encoder, &mut belt, 16, &expected[4..8]);
    let _ = buffer.write(&mut encoder, &mut belt, 0, &expected[..8]);
    assert_contents(&device, &queue, &buffer, &mut belt, encoder, &expected[..8]);

    drop(belt);
    drop(buffer);
    let _ = device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(
        instance
            .generate_report()
            .unwrap()
            .hub
            .buffers
            .num_kept_from_user,
        0
    );

    // Exercise the virtual-GPU buffer layout on every available Vulkan adapter,
    // including Windows and Linux, without starting another driver instance.
    check_padded_index_buffer(&device, &queue);
    let _ = device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(
        instance
            .generate_report()
            .unwrap()
            .hub
            .buffers
            .num_kept_from_user,
        0
    );
}

fn check_padded_index_buffer(device: &wgpu::Device, queue: &wgpu::Queue) {
    let mut adapter = device.adapter_info();
    adapter.backend = wgpu::Backend::Vulkan;
    adapter.driver = "MoltenVK".into();
    adapter.name = "Apple Paravirtual device".into();
    assert_eq!(index_buffer_offset(&adapter), WIDE_INDEX_OFFSET);
    adapter.name = "Apple M1".into();
    assert_eq!(index_buffer_offset(&adapter), 0);
    adapter.name = "Apple Paravirtual device".into();
    adapter.driver = "other driver".into();
    assert_eq!(index_buffer_offset(&adapter), 0);
    adapter.driver = "MoltenVK".into();
    adapter.backend = wgpu::Backend::Metal;
    assert_eq!(index_buffer_offset(&adapter), 0);

    let mut buffer = Buffer::<u32>::with_offset(
        device,
        "padded index buffer test",
        4,
        wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        WIDE_INDEX_OFFSET,
    );
    let mut belt = wgpu::util::StagingBelt::new(device.clone(), MAX_WRITE_SIZE as u64);
    for count in [4, MAX_WRITE_SIZE / 4 + 33] {
        let _ = buffer.resize(device, count);
        let mut expected: Vec<u32> = (0..count as u32).collect();
        let mut encoder = device.create_command_encoder(&Default::default());
        let _ = buffer.write(&mut encoder, &mut belt, 0, &expected);
        expected[1] = 0x1234_5678;
        let _ = buffer.write(&mut encoder, &mut belt, 4, &expected[1..2]);

        assert_eq!(buffer.slice(..).offset(), WIDE_INDEX_OFFSET);
        assert_eq!(
            buffer.slice(..).size().get(),
            buffer.raw.size() - WIDE_INDEX_OFFSET
        );
        assert_eq!(buffer.slice(..=7).size().get(), 8);
        assert_eq!(buffer.slice(4..).offset(), WIDE_INDEX_OFFSET + 4);
        assert_eq!(buffer.range(1, 4).offset(), WIDE_INDEX_OFFSET + 4);
        assert_eq!(buffer.range(1, 4).size().get(), 12);

        // Check the physical allocation too: writes must preserve the padding
        // and land at the same shifted address as the index-buffer binding.
        let mut physical = vec![0_u32; WIDE_INDEX_OFFSET as usize / 4];
        physical.extend(expected);
        assert_raw_contents(device, queue, &buffer.raw, &mut belt, encoder, &physical);
    }
}

fn assert_contents(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buffer: &Cached<u32>,
    belt: &mut wgpu::util::StagingBelt,
    encoder: wgpu::CommandEncoder,
    expected: &[u32],
) {
    assert_raw_contents(device, queue, &buffer.buffer.raw, belt, encoder, expected);
}

fn assert_raw_contents(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    belt: &mut wgpu::util::StagingBelt,
    mut encoder: wgpu::CommandEncoder,
    expected: &[u32],
) {
    let size = std::mem::size_of_val(expected) as u64;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    encoder.copy_buffer_to_buffer(buffer, 0, &readback, 0, size);
    belt.finish();
    let _ = queue.submit([encoder.finish()]);
    belt.recall();
    let (sender, receiver) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });
    let _ = device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    receiver.recv().unwrap().unwrap();
    let bytes = readback.slice(..).get_mapped_range();
    assert_eq!(&*bytes, bytemuck::cast_slice(expected));
    drop(bytes);
    readback.unmap();
}
