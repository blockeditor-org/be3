mod compiled_logic;
mod logic_game;
mod logic_grid;

pub use compiled_logic::*;
pub use logic_game::*;
pub use logic_grid::*;

block_type_guest::be_block_types![
    LogicGridContent { undo },
    LogicGameContent { undo },
    CompiledLogicContent
];

#[cfg(feature = "module")]
block_type_guest::abi::guest!(BlockTypes);

#[cfg(test)]
mod tests;
