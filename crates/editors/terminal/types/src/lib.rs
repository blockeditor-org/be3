mod terminal;

pub use terminal::*;

block_type_guest::be_block_types![TerminalContent];

#[cfg(feature = "module")]
block_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
