mod pdf;

pub use pdf::{PdfContent, PdfFile, PdfHeader};

block_type_guest::be_block_types![PdfContent { file }];

#[cfg(feature = "module")]
block_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
