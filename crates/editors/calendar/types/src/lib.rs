mod calendar;

pub use calendar::*;

block_type_guest::be_block_types![CalendarContent { undo }];

#[cfg(feature = "module")]
block_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
