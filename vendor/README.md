# Local dependencies

These directories are source owned and tracked by the Fragile Notepad repository.
Cargo uses local path dependencies. No vendor clone, submodule initialization,
patch application, or upstream access is needed to obtain these sources.
Other Cargo dependencies still follow the application lockfile.

| Dependency | Upstream | Original revision | License |
| --- | --- | --- | --- |
| Iced | https://github.com/iced-rs/iced | `ddd7c42a9ba625b219e5e8062ff9be83eea467c5` | [MIT](iced/LICENSE) |
| encoding_rs | https://github.com/hsivonen/encoding_rs | `229d34374bde30c8b9603a03654d7c308ade5df1` | [MIT](encoding_rs/LICENSE-MIT) or [Apache-2.0](encoding_rs/LICENSE-APACHE), with [WHATWG data notices](encoding_rs/LICENSE-WHATWG) |

Localized on 2026-09-22 with all existing application customizations preserved.
The revisions identify the original source, not an automatic update target.
Upstream copyright notices, licenses, tests, examples, and supporting source
remain in each directory. Iced's embedded Druid-derived layout code retains its
[notice](iced/core/src/layout/DRUID_LICENSE); encoding_rs includes additional
[copyright details](encoding_rs/COPYRIGHT).

See [Iced changes](iced/LOCAL_CHANGES.md) and
[encoding_rs changes](encoding_rs/LOCAL_CHANGES.md) for customization records.
Edit and commit these sources directly with the application. For a deliberate
upstream update, use a separate checkout for comparison, port the selected
changes, update these records, and run the relevant checks described in
[DEVELOPMENT.md](../DEVELOPMENT.md#vendored-dependencies).
