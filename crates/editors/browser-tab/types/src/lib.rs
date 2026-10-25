mod browser_tab;

pub use browser_tab::*;

block_type_guest::be_block_types![BrowserTabContent];

#[cfg(feature = "module")]
block_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
