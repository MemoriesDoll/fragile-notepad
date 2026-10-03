# Vulkan rendering

Hardware uses explicit wgpu Vulkan and portability features across platforms.
Software startup creates no Vulkan instance, pipeline, or GPU buffer.
The [hybrid handoff](SEAMLESS_HYBRID_RENDERING.md#handoff) owns preparation,
warm-up, presentation, and rollback.

## Resources

- About's WGSL trail uses one engine pipeline and 16-byte immediate parameters.
  Devices without sufficient `IMMEDIATES` support reuse a uniform/binding per
  widget. Widget and recorded-frame ownership retain resources until both end.
- Quad transforms use 80-byte immediates and image transforms use 64 bytes when
  device limits allow; each pipeline otherwise uses uniforms. Devices request
  supported immediates up to 128 bytes. Image sampler bindings are engine-wide.
- Quad buffers allocate on first use and grow for actual batches. Mesh, MSAA,
  gradient, and image pipelines initialize lazily and are shared across engine clones.
- Image caches and workers initialize on demand. Small rasters use a lazy
  256-square atlas; larger images/SVGs use 1024-square pages. Atlas growth respects
  device limits; failed fragmented reservations roll back. Full pools spill to
  another pool or independent textures; impossible uploads report an error.
- Uploads over 100 KiB use temporary mapped buffers retained through GPU completion.
  Staging belts start at 4 KiB and grow with writes.
- Quad, image, and glyph buffers retain CPU data and upload changed spans.
  Growth invalidates retained copies. Glyph atlas entries remain marked in use;
  pending draws retain old buffer handles.

Windows paints an offscreen initial frame during the native opening fade.
Synchronous window-size results update the viewport even if the backend emits
no later resize event. Animated images preserve fractional physical positions.

## Runtime and maintenance

Windows uses the graphics driver's Vulkan runtime. Linux needs a Vulkan loader
and driver; CI uses Lavapipe. macOS uses the loader and MoltenVK packaged by
[scripts/package-macos.sh](scripts/package-macos.sh).

See [development checks](DEVELOPMENT.md#checks-and-previews),
[packaging](PACKAGING.md), and [Iced patches](vendor/iced/LOCAL_CHANGES.md).
