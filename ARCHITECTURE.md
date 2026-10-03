# Architecture

Fragile Notepad is one Rust crate. Iced messages enter `App`, which updates
models, starts service tasks, and projects state into borrowed UI inputs.

| Module | Responsibility |
| --- | --- |
| `core/` | Documents, encoding, dirty state, workspace, settings, sessions |
| `editor/` | Buffer, history, selection, movement, wrapping, folding, syntax, outline |
| `editor/widget/` | Drawing and input translation into `EditorAction` |
| `services/` | File I/O, dialogs, streaming loads, atomic persistence |
| `app/` | Lifecycle, workflows, worker scheduling, event routing, presentation |
| `ui/` | Widgets and `WorkbenchView`; emit commands from borrowed state |

Services return typed results without importing application messages.
The application converts service events into `Message` and owns task cancellation.
Menu and keyboard commands share editor handlers. `ClosePrompt` owns its animation
and pending decision; `OutlineParsing` owns cache metadata and abort handles.
Import contracts from their owning modules.

See [application routing](src/app/README.md) for update ordering and shutdown.

## Analysis

Highlighting, folding, and outline parsing have separate consumers. Folding uses
`folding-hints.xml`; the function list uses [outline XML](assets/syntax/README.md).

The outline pipeline compiles XML into a registry, indexes lexical masks, tokens,
and delimiter pairs for an immutable snapshot, discovers declarations, then
builds the tree and function entries. Ranges use original UTF-8 byte offsets with
exclusive ends. Overlapping declarations are deduplicated before containment.
Language rules come from XML; adapter names are metadata.

Workers reject results with stale document, revision, language, registry, or
cache-generation metadata. UI widgets consume results without parsing text.
