pub mod engine;
pub mod names;
pub mod protocol;
pub mod rules;
#[cfg(feature = "text")]
pub mod text;

pub use engine::{Engine, Input, Output, WHEEL_NOTCH};
pub use names::{Modifier, Source};
#[cfg(feature = "text")]
pub use protocol::Module;
pub use protocol::{Reply, Request};
pub use rules::{Action, Chord, Group, Held, Rule, Step, Trigger, Wheel, Window, WindowMatch};

#[cfg(all(test, feature = "text"))]
mod tests;
