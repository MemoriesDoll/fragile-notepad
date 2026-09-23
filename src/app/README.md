# Application boundaries

`app.rs` declares application composition and the public Iced entry points.
`bootstrap.rs` constructs it; `subscriptions.rs` supplies runtime events.

## Commands and events

The catalog in `message.rs` declares each external message once, including its
feature and shutdown delivery policy. It generates the public `Message` API,
typed feature inputs, and exhaustive routing. Commands have one handler. Iced
continues to own asynchronous execution and delivers results as messages.

`bus.rs` is a generic, synchronous message bus with no App, Iced, global state,
locks, or callback registry. `events.rs` defines the application event vocabulary.
Subscribers are wired explicitly in `update.rs`; each receives its own state and
necessary inputs, never `&mut App`. Adding a subscriber is a composition change.
No component discovers subscribers or calls their handlers.

An update admits and dispatches its command, collects mutation events, delivers
them FIFO, and executes requested reactions in dependency order. Subscriber
mutations are collected again before selecting the next reaction. The update
returns only after synchronous reactions settle. Async tasks remain concurrent;
preparation order does not imply asynchronous completion order.

| Event | Reactions |
| --- | --- |
| Document opened | Apply settings; refresh loading state, find, outline, analysis, syntax; schedule recovery persistence |
| Document closed | Abort file/outline workers, remove recovery metadata, cancel affected deferred searches, schedule recovery persistence |
| Active tab changed | Refresh find and workers; schedule recovery persistence |
| Content changed | Refresh active find/outline/analysis/syntax; schedule recovery persistence |
| Preview changed | Debounce loading find refresh; do not persist incomplete preview text |
| Load state changed | Apply recovered metadata on completion, resume/cancel deferred search, refresh workers and loading state, schedule recovery persistence |
| Selection, scroll, viewport, or folds changed | Reprioritize syntax; schedule recovery persistence |
| Path, encoding, clean state, or tab order changed | Schedule recovery persistence |
| Settings changed | Apply document display settings; refresh affected workers |
| Analysis completed | Restore recovered folds; refresh syntax |
| Worker available | Schedule the next batch, including after stale results |
| Session ready | Schedule persistence for edits made before initialization finished |

Explicit workflows such as save-then-close remain commands with task sequencing.
The bus handles notifications and derived work; it does not make command delivery
or task completion order implicit.

## Mutation ownership

Workspace storage and active-tab selection are private. Structural operations
record ordered events. Mutable document access records a small scalar stamp on
first access; the journal compares only touched documents at the update boundary.
Use Document editing/setter methods to maintain content and metadata revisions;
construct restored documents before inserting them. Document IDs are immutable
identities: mutable document access must preserve them.

The journal does not copy text, scan fold ranges, clone paths, or inspect every tab
on every UI message. An ID index keeps document lookup and journal collection
independent of tab count for individual edits. Bulk edits visit each affected
document once; structural reordering rebuilds the ID index.

File operations own load handles and save/close/reload queues. Session state owns
recovery metadata and persistence bookkeeping. Search subscribers receive only
workspace and search inputs. Analysis, outline, and syntax own worker state and
stale-result rejection. Menu state owns its active menu and submenu path.

Settings persistence records intentional early edits at the mutation site,
including resets to existing defaults. Menu and keyboard commands use the same
handler. Frequent zoom/display changes compare Copy fields; applying the full
settings dialog compares the complete settings. Session saving observes actual
workspace changes, with no classification of incoming UI messages.

## Delivery and scheduling

Lifecycle events preserve every occurrence. Invalidation events coalesce only
while queued, using typed keys. Keys are removed before delivery, allowing a later
mutation to publish the same invalidation again. Queue and key storage are reused.
Events carry IDs and facts, not document snapshots or large buffers.

Reactions use a bit set so repeated requests run once against current state.
The generic dependency scheduler validates one cached execution order. File state
settles before deferred search; search settles before derived workers and session
persistence; analysis preparation precedes syntax. An idle UI message requests no
worker or persistence work. Callbacks cannot synchronously re-enter the dispatcher.

During shutdown, lifecycle rejects new user commands and explicitly rejects IPC
admission receipts. Background messages (timers, chunks, completions, window
notifications) are queued losslessly. Successful shutdown discards them. Failed
shutdown replays each through `App::update` in arrival order, with its own update
boundary. Pending reactions are retained while exiting and resume after failure.
New asynchronous messages must declare their delivery policy in the catalog.

## Validation

Application integration scenarios use `App::update`. Bus tests cover FIFO
ordering, lossless delivery, coalescing, republishing, and storage reuse. Journal
tests cover no-op access, batched edits, metadata, structural changes, reordering,
and streamed previews. Application tests cover subscriber mutations, closing
workers, settings propagation, deferred search, and shutdown recovery.

`bus::tests::benchmark_transport` is an ignored manual microbenchmark. It measures
only transport (three publishes, two deliveries, one coalesced event per batch),
not application latency. It has no hardware-dependent pass threshold.
