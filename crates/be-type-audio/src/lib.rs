mod audio;

pub use audio::{AudioContent, AudioFile, AudioHeader};

be_sdk_type_guest::be_block_types![AudioContent { file }];

#[cfg(feature = "module")]
be_sdk_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
