# Local Iced changes

Base: `ddd7c42a9ba625b219e5e8062ff9be83eea467c5`.
Upstream: https://github.com/iced-rs/iced. License: [MIT](LICENSE).

## Backports

| Upstream commits | Change |
| --- | --- |
| `b54f2c599`, `8caf9e44f` | Drop expired redraw deadlines when merging event-loop control flow |
| `8e05eade2`, `40339edb7` | Throttle surface-error recovery; preserve strict handoff rollback |
| `ca79fdb70` | Preserve remaining events and clear layout when an overlay disappears |
| `7c6ce8789` | Preserve stronger mouse interaction across nested overlays |
| `3c81aac2e` | Retain scrollbar interaction across widget rebuilds |
| `d8dabb4ab` | Suppress content cursor while dragging a scrollbar |

## Application patches

- Software-first renderer replacement with async prepare, offscreen warm-up,
  commit, first-presentation checks, and rollback.
- Bounded software damage/scroll matching, retained frames, resampling caches,
  and reusable clipping storage.
- Vulkan feature selection, optional immediate parameters, shared lazy pipelines,
  demand-sized buffers, bounded atlases, and changed-span uploads.
- Initial Windows offscreen frame painting during native opening fade.
- Apply synchronous window-size results to viewport and redraw state.
- `Image::snap(false)` supports fractional image bounds in both renderers.

Details: [hybrid rendering](../../SEAMLESS_HYBRID_RENDERING.md),
[Vulkan resources](../../VULKAN_RENDERING.md),
[update process](../../DEVELOPMENT.md#vendored-dependencies).
