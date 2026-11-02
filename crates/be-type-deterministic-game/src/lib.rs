mod game;

pub use game::*;

be_sdk_type_guest::be_block_types![DeterministicGameContent { undo }, GameModuleContent];

#[cfg(feature = "module")]
be_sdk_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
