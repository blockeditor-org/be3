mod chat;
mod signed;

pub use chat::{
    ChatContent, ChatHeader, Deletion, Message, MessageEdit, Reaction, ReactionKey, Segment, Shown,
    Signature,
};
pub use signed::MessageHash;

be_sdk_type_guest::be_block_types![ChatContent { undo }];

#[cfg(feature = "module")]
be_sdk_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
