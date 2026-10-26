mod image;

pub use image::{ImageContent, ImageFile, ImageHeader, ImageOp};

block_type_guest::be_block_types![ImageContent { file }];

#[cfg(feature = "module")]
block_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
