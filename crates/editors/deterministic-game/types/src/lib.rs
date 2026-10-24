mod game;

pub use game::*;

block_type_guest::be_block_types![DeterministicGameContent { undo }, GameModuleContent];

#[cfg(feature = "module")]
block_type_guest::abi::guest!(BlockTypes);

#[cfg(test)]
mod tests;
