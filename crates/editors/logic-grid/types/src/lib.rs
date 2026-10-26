mod compiled_logic;
pub mod hotbar;
mod logic_game;
mod logic_grid;

pub use compiled_logic::*;
pub use hotbar::{Hotbar, HotbarContent, HotbarSlot, SlotKind};
pub use logic_game::*;
pub use logic_grid::*;

block_type_guest::be_block_types![
    LogicGridContent { undo },
    LogicGameContent { undo },
    CompiledLogicContent,
    HotbarContent { undo },
];

#[cfg(feature = "module")]
block_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
