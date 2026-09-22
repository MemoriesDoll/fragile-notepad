# Local changes

Glyph vertices reuse their CPU vector across preparations. Only changed vertex
ranges are uploaded; empty draws and atlas-full failures invalidate the draw
state. Glyph lookup still marks atlas entries in use on every preparation.

Growing the vertex buffer releases its handle without explicitly destroying
buffers referenced by pending draws. Vulkan regressions cover unchanged and
edited text, clipping, empty draws, growth, and recovery after atlas exhaustion.

Run `cargo test --locked -p cryoglyph --lib` from the application root.
