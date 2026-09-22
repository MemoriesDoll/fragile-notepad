use std::marker::PhantomData;
use std::num::NonZeroU64;
use std::ops::RangeBounds;

#[cfg(test)]
mod tests;

pub const MAX_WRITE_SIZE: usize = 100 * 1024;
pub const STAGING_CHUNK_SIZE: u64 = 4 * 1024;

const MAX_WRITE_SIZE_U64: NonZeroU64 =
    NonZeroU64::new(MAX_WRITE_SIZE as u64).expect("MAX_WRITE_SIZE must be non-zero");

#[derive(Debug)]
pub struct Buffer<T> {
    label: &'static str,
    size: u64,
    usage: wgpu::BufferUsages,
    pub(crate) raw: wgpu::Buffer,
    type_: PhantomData<T>,
}

impl<T: bytemuck::Pod> Buffer<T> {
    pub fn new(
        device: &wgpu::Device,
        label: &'static str,
        amount: usize,
        usage: wgpu::BufferUsages,
    ) -> Self {
        let size = next_copy_size::<T>(amount);

        let raw = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage,
            mapped_at_creation: false,
        });

        Self {
            label,
            size,
            usage,
            raw,
            type_: PhantomData,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, new_count: usize) -> bool {
        let new_size = next_copy_size::<T>(new_count);

        if self.size < new_size {
            self.raw = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(self.label),
                size: new_size,
                usage: self.usage,
                mapped_at_creation: false,
            });

            self.size = new_size;

            true
        } else {
            false
        }
    }

    /// Returns the size of the written bytes.
    pub fn write(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        offset: usize,
        contents: &[T],
    ) -> usize {
        let bytes: &[u8] = bytemuck::cast_slice(contents);
        self.write_bytes(encoder, belt, offset, bytes)
    }

    fn write_bytes(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        offset: usize,
        bytes: &[u8],
    ) -> usize {
        let mut bytes_written = 0;

        // Split write into multiple chunks if necessary
        while bytes_written + MAX_WRITE_SIZE < bytes.len() {
            belt.write_buffer(
                encoder,
                &self.raw,
                (offset + bytes_written) as u64,
                MAX_WRITE_SIZE_U64,
            )
            .copy_from_slice(&bytes[bytes_written..bytes_written + MAX_WRITE_SIZE]);

            bytes_written += MAX_WRITE_SIZE;
        }

        // There will always be some bytes left, since the previous
        // loop guarantees `bytes_written < bytes.len()`
        let bytes_left = ((bytes.len() - bytes_written) as u64)
            .try_into()
            .expect("non-empty write");

        // Write them
        belt.write_buffer(
            encoder,
            &self.raw,
            (offset + bytes_written) as u64,
            bytes_left,
        )
        .copy_from_slice(&bytes[bytes_written..]);

        bytes.len()
    }

    pub fn slice(&self, bounds: impl RangeBounds<wgpu::BufferAddress>) -> wgpu::BufferSlice<'_> {
        self.raw.slice(bounds)
    }

    pub fn range(&self, start: usize, end: usize) -> wgpu::BufferSlice<'_> {
        self.slice(
            start as u64 * std::mem::size_of::<T>() as u64
                ..end as u64 * std::mem::size_of::<T>() as u64,
        )
    }
}

/// Instance data retained across draws. Copies only the changed, aligned span.
/// Like the staging belt, this tracks encoded writes: callers must submit them
/// in order before drawing with this buffer again.
#[derive(Debug)]
pub struct Cached<T> {
    buffer: Buffer<T>,
    shadow: Vec<u8>,
}

impl<T: bytemuck::Pod> Cached<T> {
    pub fn new(
        device: &wgpu::Device,
        label: &'static str,
        amount: usize,
        usage: wgpu::BufferUsages,
    ) -> Self {
        Self {
            buffer: Buffer::new(device, label, amount, usage),
            shadow: Vec::new(),
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, count: usize) {
        if self.buffer.resize(device, count) {
            self.shadow.clear();
        }
    }

    /// Returns the logical byte count, including bytes already on the GPU.
    pub fn write(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        offset: usize,
        contents: &[T],
    ) -> usize {
        let bytes: &[u8] = bytemuck::cast_slice(contents);
        let previous = self.shadow.get(offset..).unwrap_or_default();
        let changed = changed_range(previous, bytes);

        if !changed.is_empty() {
            let _ = self
                .buffer
                .write_bytes(encoder, belt, offset + changed.start, &bytes[changed]);

            // A gap has unknown contents. Keep only the known contiguous prefix.
            if offset <= self.shadow.len() {
                self.shadow
                    .resize(self.shadow.len().max(offset + bytes.len()), 0);
                self.shadow[offset..offset + bytes.len()].copy_from_slice(bytes);
            }
        }

        bytes.len()
    }

    pub fn slice(&self, bounds: impl RangeBounds<wgpu::BufferAddress>) -> wgpu::BufferSlice<'_> {
        self.buffer.slice(bounds)
    }
}

fn changed_range(previous: &[u8], bytes: &[u8]) -> std::ops::Range<usize> {
    let alignment = wgpu::COPY_BUFFER_ALIGNMENT as usize;
    assert_eq!(bytes.len() % alignment, 0);
    if previous.get(..bytes.len()) == Some(bytes) {
        return 0..0;
    }

    let start = previous
        .chunks_exact(alignment)
        .zip(bytes.chunks_exact(alignment))
        .take_while(|(old, new)| old == new)
        .count()
        * alignment;
    let suffix = previous.get(start..bytes.len()).map_or(0, |previous| {
        previous
            .rchunks_exact(alignment)
            .zip(bytes[start..].rchunks_exact(alignment))
            .take_while(|(old, new)| old == new)
            .count()
            * alignment
    });

    start..bytes.len() - suffix
}

fn next_copy_size<T>(amount: usize) -> u64 {
    let align_mask = wgpu::COPY_BUFFER_ALIGNMENT - 1;

    (((std::mem::size_of::<T>() * amount).next_power_of_two() as u64 + align_mask) & !align_mask)
        .max(wgpu::COPY_BUFFER_ALIGNMENT)
}
