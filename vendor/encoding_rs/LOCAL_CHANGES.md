# encoding_rs provenance and local customizations

Upstream: https://github.com/hsivonen/encoding_rs

Original revision: `229d34374bde30c8b9603a03654d7c308ade5df1`.

Localized on 2026-09-22. This source is tracked directly in the application
repository and retains its upstream licenses and copyright notices.

The application exposes an `oem` module through `src/lib.rs`. `src/oem.rs` adds
OEM code-page encoding/decoding, labels, and tables for CP437, CP720, CP737,
CP775, CP850, CP852, CP855, CP857, CP858, CP860, CP861, CP862, CP863, CP865,
CP866, and CP869. These extensions are retained from the previous local patch.

Future changes are ordinary source edits; no patch export or application step
is required. See [the vendor update process](../../DEVELOPMENT.md#vendored-dependencies).
