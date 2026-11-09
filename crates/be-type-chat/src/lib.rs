mod chat;
mod reads;
mod signed;

pub use chat::{
    ChatContent, ChatHeader, Deletion, Message, MessageEdit, Reaction, ReactionKey, Segment, Shown,
    Signature,
};
pub use reads::{ChatReads, ChatReadsContent};
pub use signed::MessageHash;

be_sdk_type_guest::be_block_types![ChatContent { undo }, ChatReadsContent];

#[cfg(feature = "module")]
be_sdk_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
