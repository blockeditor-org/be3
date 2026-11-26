mod chat;
mod log;
mod reads;

pub use chat::{Carried, ChatContent, ChatHeader, Draft};
pub use log::{ChatEntry, ChatLog, History, Shown, least_trusted};
pub use reads::{ChatReads, ChatReadsContent};

be_sdk_type_guest::be_block_types![ChatContent { undo, log, unread }, ChatReadsContent];

#[cfg(feature = "module")]
be_sdk_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
