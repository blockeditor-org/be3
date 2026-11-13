mod chat;
mod reads;
mod segment;
mod signed;

pub use chat::{
    Carried, ChatContent, ChatHeader, ChatOp, Closing, Deletion, Draft, Message, MessageEdit,
    Reaction, ReactionKey, Segment, Shown, Signature,
};
pub use reads::{ChatReads, ChatReadsContent, ReadPlace};
pub use signed::MessageHash;

be_sdk_type_guest::be_block_types![ChatContent { undo, merge_parts }, ChatReadsContent];

#[cfg(feature = "module")]
be_sdk_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
