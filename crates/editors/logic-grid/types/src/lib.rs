mod logic_grid;
mod logic_game;
mod compiled_logic;

pub use logic_grid::*;
pub use logic_game::*;
pub use compiled_logic::*;

block_type_guest::be_block_types![LogicGridContent { undo }, LogicGameContent { undo }, CompiledLogicContent];

#[cfg(feature = "module")]
block_type_guest::abi::guest!(BlockTypes);

#[cfg(test)]
mod tests;
