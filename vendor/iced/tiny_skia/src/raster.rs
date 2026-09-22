use crate::core::image as raster;
use crate::core::{Rectangle, Size};
use crate::graphics;

use rustc_hash::{FxHashMap, FxHashSet};
use std::cell::RefCell;
use std::collections::hash_map;

#[derive(Debug)]
pub struct Pipeline {
    cache: RefCell<Cache>,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            cache: RefCell::new(Cache::default()),
        }
    }

    pub fn load(&self, handle: &raster::Handle) -> Result<raster::Allocation, raster::Error> {
        let mut cache = self.cache.borrow_mut();
        let image = cache.allocate(handle)?;

        #[allow(unsafe_code)]
        Ok(unsafe { raster::allocate(handle, Size::new(image.width(), image.height())) })
    }

    pub fn dimensions(&self, handle: &raster::Handle) -> Option<Size<u32>> {
        let mut cache = self.cache.borrow_mut();
        let image = cache.allocate(handle).ok()?;

        Some(Size::new(image.width(), image.height()))
    }

    pub fn draw(
        &mut self,
        handle: &raster::Handle,
        filter_method: raster::FilterMethod,
        bounds: Rectangle,
        opacity: f32,
        snap: bool,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        transform: tiny_skia::Transform,
        clip_mask: Option<&tiny_skia::Mask>,
    ) {
        let mut cache = self.cache.borrow_mut();

        let Ok((image_width, image_height)) = cache
            .allocate(handle)
            .map(|image| (image.width(), image.height()))
        else {
            return;
        };

        let bounds = if snap {
            physical_pixel_aligned(bounds, transform)
        } else {
            bounds
        };
        let width_scale = bounds.width / image_width as f32;
        let height_scale = bounds.height / image_height as f32;

        if snap
            && filter_method == raster::FilterMethod::Linear
            && !transform.has_skew()
            && let Some(physical_bounds) = physical_pixel_bounds(bounds, transform)
            && let Some(resampled) = cache.resample_straight_alpha(
                handle,
                Size::new(physical_bounds.width, physical_bounds.height),
            )
        {
            pixels.draw_pixmap(
                physical_bounds.x as i32,
                physical_bounds.y as i32,
                resampled.as_ref(),
                &tiny_skia::PixmapPaint {
                    quality: tiny_skia::FilterQuality::Nearest,
                    opacity,
                    ..Default::default()
                },
                tiny_skia::Transform::identity(),
                clip_mask,
            );

            return;
        }

        let Ok(image) = cache.allocate(handle) else {
            return;
        };

        let transform = transform
            .pre_translate(bounds.x, bounds.y)
            .pre_scale(width_scale, height_scale);

        let quality = match filter_method {
            raster::FilterMethod::Linear => tiny_skia::FilterQuality::Bilinear,
            raster::FilterMethod::Nearest => tiny_skia::FilterQuality::Nearest,
        };

        pixels.draw_pixmap(
            0,
            0,
            image,
            &tiny_skia::PixmapPaint {
                quality,
                opacity,
                ..Default::default()
            },
            transform,
            clip_mask,
        );
    }

    pub fn trim_cache(&mut self) {
        self.cache.borrow_mut().trim();
    }
}

#[derive(Debug, Default)]
struct Cache {
    entries: FxHashMap<raster::Id, Option<Entry>>,
    hits: FxHashSet<raster::Id>,
    resampled: FxHashMap<(raster::Id, u32, u32), ResampledEntry>,
    resampled_pixels: usize,
    clock: u64,
    oversized: Option<tiny_skia::Pixmap>,
}

#[derive(Debug)]
struct ResampledEntry {
    pixmap: tiny_skia::Pixmap,
    last_used: u64,
}

impl Cache {
    pub fn allocate(
        &mut self,
        handle: &raster::Handle,
    ) -> Result<tiny_skia::PixmapRef<'_>, raster::Error> {
        let id = handle.id();

        if let hash_map::Entry::Vacant(entry) = self.entries.entry(id) {
            let image = match graphics::image::load(handle) {
                Ok(image) => image,
                Err(error) => {
                    let _ = entry.insert(None);

                    return Err(error);
                }
            };

            if image.width() == 0 || image.height() == 0 {
                return Err(raster::Error::Empty);
            }

            let mut buffer = vec![0u32; image.width() as usize * image.height() as usize];
            let mut rgba = Vec::with_capacity(image.width() as usize * image.height() as usize * 4);

            for (i, pixel) in image.pixels().enumerate() {
                let [r, g, b, a] = pixel.0;

                rgba.extend([r, g, b, a]);
                buffer[i] = bytemuck::cast(tiny_skia::ColorU8::from_rgba(b, g, r, a).premultiply());
            }

            let _ = entry.insert(Some(Entry {
                width: image.width(),
                height: image.height(),
                rgba,
                pixels: buffer,
            }));
        }

        let _ = self.hits.insert(id);

        Ok(self
            .entries
            .get(&id)
            .unwrap()
            .as_ref()
            .map(|entry| {
                tiny_skia::PixmapRef::from_bytes(
                    bytemuck::cast_slice(&entry.pixels),
                    entry.width,
                    entry.height,
                )
                .expect("Build pixmap from image bytes")
            })
            .expect("Image should be allocated"))
    }

    fn trim(&mut self) {
        self.entries.retain(|key, _| self.hits.contains(key));
        self.hits.clear();
        self.oversized = None;
        self.resampled
            .retain(|(id, _, _), _| self.entries.contains_key(id));
        self.resampled_pixels = self
            .resampled
            .values()
            .map(|entry| entry.pixmap.width() as usize * entry.pixmap.height() as usize)
            .sum();
    }

    fn resample_straight_alpha(
        &mut self,
        handle: &raster::Handle,
        size: Size<u32>,
    ) -> Option<&tiny_skia::Pixmap> {
        const MAX_ENTRIES: usize = 128;
        const MAX_PIXELS: usize = 4 * 1024 * 1024;
        let key = (handle.id(), size.width, size.height);
        let pixels = size.width as usize * size.height as usize;
        self.clock = self.clock.wrapping_add(1);
        if !self.resampled.contains_key(&key) {
            let entry = self.entries.get(&handle.id())?.as_ref()?;
            let pixmap = Self::resample_entry(entry, size)?;
            if pixels > MAX_PIXELS {
                self.oversized = Some(pixmap);
                return self.oversized.as_ref();
            }
            while !self.resampled.is_empty()
                && (self.resampled.len() >= MAX_ENTRIES
                    || self.resampled_pixels.saturating_add(pixels) > MAX_PIXELS)
            {
                let oldest = self
                    .resampled
                    .iter()
                    .min_by_key(|(_, entry)| entry.last_used)
                    .map(|(key, _)| *key)?;
                if let Some(removed) = self.resampled.remove(&oldest) {
                    self.resampled_pixels -=
                        removed.pixmap.width() as usize * removed.pixmap.height() as usize;
                }
            }
            self.resampled_pixels += pixels;
            let _ = self.resampled.insert(
                key,
                ResampledEntry {
                    pixmap,
                    last_used: self.clock,
                },
            );
        }
        let entry = self.resampled.get_mut(&key)?;
        entry.last_used = self.clock;
        Some(&entry.pixmap)
    }

    fn resample_entry(entry: &Entry, size: Size<u32>) -> Option<tiny_skia::Pixmap> {
        let width = size.width;
        let height = size.height;

        if width == 0 || height == 0 {
            return None;
        }

        let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
        let source_width = entry.width as usize;
        let source_height = entry.height as usize;

        for y in 0..height {
            let source_y = ((y as f32 + 0.5) * entry.height as f32 / height as f32) - 0.5;
            let y0 = source_y.floor() as i32;
            let y1 = y0 + 1;
            let wy = source_y - source_y.floor();

            for x in 0..width {
                let source_x = ((x as f32 + 0.5) * entry.width as f32 / width as f32) - 0.5;
                let x0 = source_x.floor() as i32;
                let x1 = x0 + 1;
                let wx = source_x - source_x.floor();

                let sample = |x: i32, y: i32| -> [f32; 4] {
                    let x = x.clamp(0, source_width as i32 - 1) as usize;
                    let y = y.clamp(0, source_height as i32 - 1) as usize;
                    let offset = (y * source_width + x) * 4;

                    [
                        entry.rgba[offset] as f32,
                        entry.rgba[offset + 1] as f32,
                        entry.rgba[offset + 2] as f32,
                        entry.rgba[offset + 3] as f32,
                    ]
                };

                let top_left = sample(x0, y0);
                let top_right = sample(x1, y0);
                let bottom_left = sample(x0, y1);
                let bottom_right = sample(x1, y1);
                let mut rgba = [0.0; 4];

                for channel in 0..4 {
                    let top = lerp(top_left[channel], top_right[channel], wx);
                    let bottom = lerp(bottom_left[channel], bottom_right[channel], wx);

                    rgba[channel] = lerp(top, bottom, wy);
                }

                let color = premultiply_sampled_rgba(rgba)?;

                pixels.extend(bytemuck::bytes_of(&color));
            }
        }

        tiny_skia::Pixmap::from_vec(pixels, tiny_skia::IntSize::from_wh(width, height)?)
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

fn premultiply_sampled_rgba(rgba: [f32; 4]) -> Option<tiny_skia::PremultipliedColorU8> {
    let alpha = to_unorm8(rgba[3]);
    let red = to_unorm8(rgba[0] * rgba[3] / 255.0);
    let green = to_unorm8(rgba[1] * rgba[3] / 255.0);
    let blue = to_unorm8(rgba[2] * rgba[3] / 255.0);

    tiny_skia::PremultipliedColorU8::from_rgba(blue, green, red, alpha)
}

fn to_unorm8(value: f32) -> u8 {
    value.clamp(0.0, 255.0).round() as u8
}

#[derive(Debug)]
struct Entry {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    pixels: Vec<u32>,
}

fn physical_pixel_aligned(bounds: Rectangle, transform: tiny_skia::Transform) -> Rectangle {
    let (scale_x, scale_y) = transform.get_scale();

    if scale_x == 0.0 || scale_y == 0.0 {
        return bounds;
    }

    let left = (bounds.x * scale_x).round();
    let top = (bounds.y * scale_y).round();
    let right = ((bounds.x + bounds.width) * scale_x).round();
    let bottom = ((bounds.y + bounds.height) * scale_y).round();

    Rectangle {
        x: left / scale_x,
        y: top / scale_y,
        width: (right - left) / scale_x,
        height: (bottom - top) / scale_y,
    }
}

fn physical_pixel_bounds(
    bounds: Rectangle,
    transform: tiny_skia::Transform,
) -> Option<Rectangle<u32>> {
    let (scale_x, scale_y) = transform.get_scale();

    if scale_x == 0.0 || scale_y == 0.0 {
        return None;
    }

    let x = (bounds.x * scale_x + transform.tx).round();
    let y = (bounds.y * scale_y + transform.ty).round();
    let right = ((bounds.x + bounds.width) * scale_x + transform.tx).round();
    let bottom = ((bounds.y + bounds.height) * scale_y + transform.ty).round();
    let width = right - x;
    let height = bottom - y;

    if x < 0.0 || y < 0.0 || width < 1.0 || height < 1.0 {
        return None;
    }

    Some(Rectangle {
        x: x as u32,
        y: y as u32,
        width: width as u32,
        height: height as u32,
    })
}

#[cfg(all(test, feature = "image"))]
mod tests {
    use super::*;

    #[test]
    fn resample_cache_matches_original_pixels_for_scales_transparency_and_hits() {
        let rgba = vec![
            255, 0, 0, 0, 0, 255, 0, 64, 0, 0, 255, 127, 255, 255, 255, 255,
        ];
        let handle = raster::Handle::from_rgba(2, 2, rgba);
        let mut cache = Cache::default();
        let _ = cache.allocate(&handle).unwrap();
        for size in [
            Size::new(1, 1),
            Size::new(2, 2),
            Size::new(3, 5),
            Size::new(8, 8),
        ] {
            let reference = Cache::resample_entry(
                cache.entries.get(&handle.id()).unwrap().as_ref().unwrap(),
                size,
            )
            .unwrap();
            let miss = cache.resample_straight_alpha(&handle, size).unwrap();
            assert_eq!(miss.data(), reference.data());
            let allocation = miss.data().as_ptr();
            let hit = cache.resample_straight_alpha(&handle, size).unwrap();
            assert_eq!(hit.data(), reference.data());
            assert_eq!(
                hit.data().as_ptr(),
                allocation,
                "cache hit reuses the exact raster"
            );
        }
    }

    #[test]
    fn resample_cache_limits_entries_and_discards_unreferenced_sources() {
        let handle = raster::Handle::from_rgba(4, 4, source_rgba());
        let mut cache = Cache::default();
        let _ = cache.allocate(&handle).unwrap();
        for width in 1..=160 {
            let _ = cache
                .resample_straight_alpha(&handle, Size::new(width, 1))
                .unwrap();
        }
        assert_eq!(cache.resampled.len(), 128);
        assert!(cache.resampled_pixels <= 4 * 1024 * 1024);
        assert!(!cache.resampled.contains_key(&(handle.id(), 1, 1)));
        cache.trim();
        assert_eq!(cache.resampled.len(), 128);
        cache.trim();
        assert!(cache.resampled.is_empty());
        assert_eq!(cache.resampled_pixels, 0);
    }

    #[test]
    fn resample_cache_separates_sources_and_enforces_pixel_budget() {
        let red = raster::Handle::from_rgba(1, 1, vec![255, 0, 0, 127]);
        let blue = raster::Handle::from_rgba(1, 1, vec![0, 0, 255, 127]);
        let mut cache = Cache::default();
        let _ = cache.allocate(&red).unwrap();
        let _ = cache.allocate(&blue).unwrap();
        let red_bytes = cache
            .resample_straight_alpha(&red, Size::new(1, 1))
            .unwrap()
            .data()
            .to_vec();
        let blue_bytes = cache
            .resample_straight_alpha(&blue, Size::new(1, 1))
            .unwrap()
            .data()
            .to_vec();
        assert_ne!(red_bytes, blue_bytes);
        for width in [1024, 1025, 1026, 1027, 1028] {
            let _ = cache
                .resample_straight_alpha(&red, Size::new(width, 1024))
                .unwrap();
            assert!(cache.resampled_pixels <= 4 * 1024 * 1024);
        }
    }

    fn source_rgba() -> Vec<u8> {
        vec![
            255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 0, 0, 0, 255, 255,
            255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 255, 255,
            255, 255, 255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255,
            255, 255,
        ]
    }

    fn tiny_skia_pixmap_bytes(rgba: &[u8]) -> Vec<u32> {
        rgba.chunks_exact(4)
            .map(|pixel| {
                let [r, g, b, a] = [pixel[0], pixel[1], pixel[2], pixel[3]];

                bytemuck::cast(tiny_skia::ColorU8::from_rgba(b, g, r, a).premultiply())
            })
            .collect()
    }

    fn draw_reference(bounds: Rectangle, legacy_integer_source_offset: bool) -> tiny_skia::Pixmap {
        let rgba = source_rgba();
        let source_pixels = tiny_skia_pixmap_bytes(&rgba);
        let source = tiny_skia::PixmapRef::from_bytes(bytemuck::cast_slice(&source_pixels), 4, 4)
            .expect("create source pixmap");
        let mut output = tiny_skia::Pixmap::new(10, 10).expect("create output pixmap");
        let bounds = if legacy_integer_source_offset {
            bounds
        } else {
            physical_pixel_aligned(bounds, tiny_skia::Transform::identity())
        };

        let width_scale = bounds.width / source.width() as f32;
        let height_scale = bounds.height / source.height() as f32;
        let quality = tiny_skia::FilterQuality::Bilinear;

        let (x, y, transform) = if legacy_integer_source_offset {
            (
                (bounds.x / width_scale) as i32,
                (bounds.y / height_scale) as i32,
                tiny_skia::Transform::identity().pre_scale(width_scale, height_scale),
            )
        } else {
            (
                0,
                0,
                tiny_skia::Transform::identity()
                    .pre_translate(bounds.x, bounds.y)
                    .pre_scale(width_scale, height_scale),
            )
        };

        output.as_mut().draw_pixmap(
            x,
            y,
            source,
            &tiny_skia::PixmapPaint {
                quality,
                ..Default::default()
            },
            transform,
            None,
        );

        output
    }

    #[test]
    fn fractional_linear_render_matches_physical_pixel_aligned_reference() {
        let bounds = Rectangle {
            x: 2.6,
            y: 1.4,
            width: 3.0,
            height: 3.0,
        };
        let handle = raster::Handle::from_rgba(4, 4, source_rgba());
        let mut pipeline = Pipeline::new();
        let mut actual = tiny_skia::Pixmap::new(10, 10).expect("create actual pixmap");

        pipeline.draw(
            &handle,
            raster::FilterMethod::Linear,
            bounds,
            1.0,
            true,
            &mut actual.as_mut(),
            tiny_skia::Transform::identity(),
            None,
        );

        let expected = draw_reference(bounds, false);
        let legacy = draw_reference(bounds, true);

        assert_eq!(actual.data(), expected.data());
        assert_ne!(
            actual.data(),
            legacy.data(),
            "fractional image bounds must be rounded in physical destination space"
        );
    }
}
