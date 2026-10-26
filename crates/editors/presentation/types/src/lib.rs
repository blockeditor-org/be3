mod presentation;

pub use presentation::{Presentation, PresentationContent, Slide};

block_type_guest::be_block_types![PresentationContent { undo }];

#[cfg(feature = "module")]
block_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
