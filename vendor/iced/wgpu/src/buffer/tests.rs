use super::{Cached, MAX_WRITE_SIZE, changed_range};

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
    let adapter =
        futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference:
                wgpu::PowerPreference::from_env().unwrap_or(wgpu::PowerPreference::HighPerformance),
            ..Default::default()
        }))
        .unwrap();
    eprintln!("RETAINED_ADAPTER {:?}", adapter.get_info());
    let (device, queue) =
        futures::executor::block_on(adapter.request_device(&Default::default())).unwrap();
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
}

fn assert_contents(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buffer: &Cached<u32>,
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
    encoder.copy_buffer_to_buffer(&buffer.buffer.raw, 0, &readback, 0, size);
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
