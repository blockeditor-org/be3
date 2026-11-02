mod presentation;

pub use presentation::{Presentation, PresentationContent, Slide};

be_sdk_type_guest::be_block_types![PresentationContent { undo }];

#[cfg(feature = "module")]
be_sdk_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
