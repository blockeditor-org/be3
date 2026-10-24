mod counter;

pub use counter::*;

block_type_guest::be_block_types![CounterContent { undo }];

#[cfg(feature = "module")]
block_type_guest::abi::guest!(BlockTypes);

#[cfg(test)]
mod tests;
