//! UI-facing icon widgets and handles.

pub const ICON_SIZE: u32 = 22;

pub mod hero;
mod mask;
pub mod shortcut;
pub mod tango;

pub use hero::{HeroIcon, IconTone};
pub use shortcut::ShortcutIcon;
pub use tango::TangoIcon;
