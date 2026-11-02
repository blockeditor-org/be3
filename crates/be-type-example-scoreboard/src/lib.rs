mod scoreboard;

pub use scoreboard::*;

be_sdk_type_guest::be_block_types![ScoreboardContent { undo }];

#[cfg(feature = "module")]
be_sdk_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
