mod keys;
mod screen;
mod view;

pub use keys::{TerminalInput, key_event, text_event};
pub use screen::{Emulator, Row, Span, TerminalScreen};
pub use view::{Fit, TerminalView};

#[cfg(test)]
mod tests;
