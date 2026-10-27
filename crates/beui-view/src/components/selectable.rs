use crate::actions::Chord;
use crate::types::Key;

pub use crate::core::selectable::{Selectable, SelectableState, copy_selection, select_all};

pub const COPY_KEY: Chord = Chord::ctrl(Key::C);
pub const SELECT_ALL_KEY: Chord = Chord::ctrl(Key::A);
