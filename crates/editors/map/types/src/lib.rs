mod map;

pub use map::*;

block_type_guest::be_block_types![MapContent { undo }];

#[cfg(feature = "module")]
block_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
