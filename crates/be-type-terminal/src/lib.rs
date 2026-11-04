mod terminal;

pub use terminal::*;

be_sdk_type_guest::be_block_types![TerminalContent];

#[cfg(feature = "module")]
be_sdk_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
