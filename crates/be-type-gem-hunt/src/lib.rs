mod hunt;

pub use hunt::{CELLS, GEMS, Hunt, HuntContent, SIDE};

be_sdk_type_guest::be_block_types![HuntContent { undo }];

#[cfg(feature = "module")]
be_sdk_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
