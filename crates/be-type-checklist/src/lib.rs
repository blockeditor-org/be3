mod checklist;

pub use checklist::{Checklist, ChecklistContent, ChecklistItem};

be_sdk_type_guest::be_block_types![ChecklistContent { undo }];

#[cfg(feature = "module")]
be_sdk_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
