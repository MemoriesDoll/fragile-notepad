# Local Cryoglyph changes

Glyph preparations reuse CPU vertices and upload changed spans. Empty draws and
atlas exhaustion invalidate draw state; lookups still mark atlas entries in use.
Buffer growth releases handles without destroying buffers retained by pending draws.

Run `cargo test --locked -p cryoglyph --lib` from the application root.
