use crate::core::Size;
use crate::core::image;
use crate::graphics;
use crate::image::atlas::{self, Atlas};

use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::{Arc, Weak};

pub type Image = graphics::image::Buffer;

/// Entry in cache corresponding to an image handle
#[derive(Debug)]
pub enum Memory {
    /// Image data on host
    Host(Image),
    /// Storage entry
    Device {
        entry: atlas::Entry,
        storage: Storage,
        allocation: Option<Weak<image::Memory>>,
    },
    Error(image::Error),
}

/// Shared atlas ownership stays explicit so growth and eviction always use
/// the same pool. Worker uploads retain their independent texture bindings.
#[derive(Debug)]
pub enum Storage {
    Main,
    Icons,
    Dedicated(Arc<wgpu::BindGroup>),
}

impl Storage {
    pub fn upload(
        main: &mut Atlas,
        icons: &mut Atlas,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        image: &Image,
    ) -> Option<(atlas::Entry, Self)> {
        let storage = Self::for_size(image.width(), image.height());
        if let Some(entry) = storage.atlas(main, icons).upload(
            device,
            encoder,
            belt,
            image.width(),
            image.height(),
            image,
        ) {
            return Some((entry, storage));
        }
        if matches!(storage, Self::Icons) {
            if let Some(entry) =
                main.upload(device, encoder, belt, image.width(), image.height(), image)
            {
                return Some((entry, Self::Main));
            }
        }
        // Array-layer exhaustion is not device-memory exhaustion. Keep a
        // separately owned texture when the image fits the device by itself.
        let mut standalone = main.standalone(device, image.width().max(image.height()));
        let entry =
            standalone.upload(device, encoder, belt, image.width(), image.height(), image)?;
        Some((entry, Self::Dedicated(standalone.bind_group().clone())))
    }

    pub fn for_size(width: u32, height: u32) -> Self {
        if width <= atlas::ICON_SIZE / 2 && height <= atlas::ICON_SIZE / 2 {
            Self::Icons
        } else {
            Self::Main
        }
    }

    pub fn atlas<'a>(&self, main: &'a mut Atlas, icons: &'a mut Atlas) -> &'a mut Atlas {
        match self {
            Self::Main => main,
            Self::Icons => icons,
            Self::Dedicated(_) => unreachable!("worker-owned texture has no shared atlas"),
        }
    }

    pub fn bind_group<'a>(&'a self, main: &'a Atlas, icons: &'a Atlas) -> &'a Arc<wgpu::BindGroup> {
        match self {
            Self::Main => main.bind_group(),
            Self::Icons => icons.bind_group(),
            Self::Dedicated(binding) => binding,
        }
    }
}

impl Memory {
    pub fn load(handle: &image::Handle) -> Self {
        match graphics::image::load(handle) {
            Ok(image) => Self::Host(image),
            Err(error) => Self::Error(error),
        }
    }

    pub fn dimensions(&self) -> Size<u32> {
        match self {
            Memory::Host(image) => {
                let (width, height) = image.dimensions();

                Size::new(width, height)
            }
            Memory::Device { entry, .. } => entry.size(),
            Memory::Error(_) => Size::new(1, 1),
        }
    }

    pub fn host(&self) -> Option<Image> {
        match self {
            Memory::Host(image) => Some(image.clone()),
            Memory::Device { .. } | Memory::Error(_) => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct Cache {
    map: FxHashMap<image::Id, Memory>,
    hits: FxHashSet<image::Id>,
    should_trim: bool,
}

impl Cache {
    pub fn get_mut(&mut self, handle: &image::Handle) -> Option<&mut Memory> {
        let _ = self.hits.insert(handle.id());

        self.map.get_mut(&handle.id())
    }

    pub fn insert(&mut self, handle: &image::Handle, memory: Memory) {
        let _ = self.map.insert(handle.id(), memory);
        let _ = self.hits.insert(handle.id());

        self.should_trim = true;
    }

    pub fn contains(&self, handle: &image::Handle) -> bool {
        self.map.contains_key(&handle.id())
    }

    pub fn trim(
        &mut self,
        atlas: &mut Atlas,
        icons: &mut Atlas,
        on_drop: impl Fn(Arc<wgpu::BindGroup>),
    ) {
        // Only trim if new entries have landed in the `Cache`
        if !self.should_trim {
            return;
        }

        let hits = &self.hits;

        self.map.retain(|id, memory| {
            // Retain active allocations
            if let Memory::Device { allocation, .. } = memory
                && allocation
                    .as_ref()
                    .is_some_and(|allocation| allocation.strong_count() > 0)
            {
                return true;
            }

            let retain = hits.contains(id);

            if !retain {
                log::debug!("Dropping image allocation: {id:?}");

                if let Memory::Device { entry, storage, .. } = memory {
                    match std::mem::replace(storage, Storage::Main) {
                        Storage::Main => atlas.remove(entry),
                        Storage::Icons => icons.remove(entry),
                        Storage::Dedicated(binding) => on_drop(binding),
                    }
                }
            }

            retain
        });

        self.hits.clear();
        self.should_trim = false;
    }
}
