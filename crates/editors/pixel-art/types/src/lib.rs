mod pixel_art;

pub use pixel_art::*;

block_type_guest::be_block_types![PixelArtContent { undo }];

#[cfg(feature = "module")]
block_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
