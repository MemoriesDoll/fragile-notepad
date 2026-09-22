//! Configure a [`Backend`](crate::core::Backend) at runtime.
use crate::Task;
use crate::core::backend;
use crate::futures::futures::channel::oneshot;
use crate::task;

/// An backend operation.
#[derive(Debug)]
pub enum Action {
    /// Switches the [`backend::Settings`] of the current application.
    Configure(
        backend::Settings,
        oneshot::Sender<Result<(), backend::Error>>,
    ),
    /// Prepares a replacement [`backend::Settings`] and commits it after a
    /// successful present boundary.
    PrepareWarmAndCommit(
        backend::Settings,
        oneshot::Sender<backend::StrictHandoffOutcome>,
    ),
}

/// Returns a [`Task`] that switches the [`backend::Settings`] of the current application.
///
/// This can be leveraged to switch renderers at runtime.
pub fn configure(settings: backend::Settings) -> Task<Result<(), backend::Error>> {
    task::oneshot(|sender| crate::Action::Backend(Action::Configure(settings, sender)))
}

/// Returns a [`Task`] that prepares a replacement backend and commits it after
/// a successful present boundary.
///
/// This keeps the current renderer active while the replacement compositor is
/// created. It does not imply a full offscreen warm-up unless the selected
/// backend implements that internally.
pub fn prepare_warm_and_commit(settings: backend::Settings) -> Task<backend::StrictHandoffOutcome> {
    task::oneshot(|sender| crate::Action::Backend(Action::PrepareWarmAndCommit(settings, sender)))
}
