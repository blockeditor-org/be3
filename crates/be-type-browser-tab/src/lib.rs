mod browser_tab;

pub use browser_tab::*;

be_sdk_type_guest::be_block_types![BrowserTabContent];

#[cfg(feature = "module")]
be_sdk_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
