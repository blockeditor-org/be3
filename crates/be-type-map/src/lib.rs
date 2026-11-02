mod map;

pub use map::*;

be_sdk_type_guest::be_block_types![MapContent { undo }];

#[cfg(feature = "module")]
be_sdk_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
