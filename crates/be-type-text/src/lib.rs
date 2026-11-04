mod text;

pub use text::{
    TextBlock, TextContent, TextIndentation, TextLanguage, language_of, text_file_type,
};

be_sdk_type_guest::be_block_types![TextContent { undo, file }];

#[cfg(feature = "module")]
be_sdk_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
