mod audio;

pub use audio::{AudioContent, AudioFile, AudioHeader};

block_type_guest::be_block_types![AudioContent { file }];

#[cfg(feature = "module")]
block_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
