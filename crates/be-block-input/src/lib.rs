//! The input settings block (pointers, keyboards, pasting) and the input remapper block, which
//! the settings app edits and the hosts apply. Apart from be-block, since no editor but the
//! settings app reads them.

pub mod input_remapper;
pub mod input_settings;

pub use input_remapper::{InputRemapper, InputRemapperContent};
pub use input_settings::{InputSettings, InputSettingsContent};

#[cfg(test)]
mod tests;
