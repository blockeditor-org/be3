pub mod mapping;
pub mod reach;
pub mod scancode;
pub mod synth;

#[cfg(windows)]
pub mod inject;

pub use mapping::{PixelPoint, PixelRect, absolute};
pub use reach::{InjectError, Target, target};
pub use synth::{Event, INJECTED, Planned, PointerAction, Synth, plan};

#[cfg(windows)]
pub use inject::{inject, virtual_desktop};
