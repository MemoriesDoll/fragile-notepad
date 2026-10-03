# Application routing

`app.rs` composes state; `bootstrap.rs` initializes it and `subscriptions.rs`
supplies runtime events. The catalog in `message.rs` declares each message's
feature and shutdown policy and generates exhaustive routing.

An update dispatches one command, collects mutation events, delivers them FIFO,
and runs reactions in dependency order until synchronous work settles.
Iced executes async tasks; preparation order does not determine completion order.

## Ownership

Workspace structural operations journal events. Mutable document access compares
scalar stamps only for touched documents at the update boundary. Use document
editing/setter methods to maintain revisions and preserve immutable document IDs.
The journal avoids copying text or scanning every tab; structural reorderings
rebuild the ID index.

File workflows own load handles and save/close/reload queues. Session state owns
recovery and persistence. Analysis, outline, and syntax own their workers and
stale-result checks. Subscribers receive their state and required inputs, never
`&mut App`; wiring belongs in `update.rs`.

Settings writes record intentional edits at mutation sites, including early
startup edits. Session persistence observes workspace changes.

## Scheduling and shutdown

`bus.rs` preserves lifecycle events and coalesces invalidations only while queued.
Removing a key before delivery permits later republication. Events carry IDs and
facts rather than snapshots; queue storage is reused.

Reactions merge repeated requests. File state settles before deferred search;
search precedes workers and session persistence; analysis precedes syntax.
Save-then-close remains an explicitly sequenced command.

Shutdown rejects new user commands and IPC admission receipts. Background messages
queue losslessly: successful exit discards them; failed exit replays them in order
through `App::update`, each with its own update boundary, then resumes reactions.
New async messages must declare their delivery policy.
