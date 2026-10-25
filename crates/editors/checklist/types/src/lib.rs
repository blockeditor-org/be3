mod checklist;

pub use checklist::{Checklist, ChecklistContent, ChecklistItem};

block_type_guest::be_block_types![ChecklistContent { undo }];

#[cfg(feature = "module")]
block_type_guest::abi::guest!(BlockTypes);

#[cfg(test)]
mod tests;
