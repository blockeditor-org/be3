use be_block::{
    BlockContent, BlockMetadata, ChildChange, ContentError, LiveEdit, Loads, Merge, Part, Root,
    Stored, Undo,
};
use be_block_commit::MergeResult;
use be_block_model::{
    Anchor, BlockRef, Change, Document, Edit, List, Map, Model, ObjectId, Series, Step, Touched,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::signed::{MessageHash, Signed};

/// The chat's header, the first part of its stored form and the only one 1a writes: the
/// recent messages, and every edit, deletion and reaction, kept apart from the messages and
/// keyed by message id, so a message that moves into a segment keeps them.
#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
#[model(kind = "chat")]
pub struct ChatHeader {
    /// Each side's messages stay together when two replicas merge, ours first.
    #[model(merge_appends)]
    pub messages: List<Message>,
    /// The latest edit of each message, by its id.
    pub edits: Map<ObjectId, MessageEdit>,
    /// Each message deleted, by its id. The message itself is removed (it stays in version
    /// history); this keeps who deleted it and when, which a message kept in a segment needs.
    pub deletions: Map<ObjectId, Deletion>,
    pub reactions: Map<ReactionKey, Reaction>,
    /// The closed segments, oldest first. None is closed yet.
    pub segments: Series<Segment>,
}

/// A message: an object of registers, not text, so two people's messages never merge into
/// one. Its id is its `ObjectId`.
#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
#[model(kind = "chat.message")]
pub struct Message {
    /// The account that wrote it.
    pub author: Uuid,
    /// When its author says it was written, in milliseconds since the Unix epoch.
    pub time: i64,
    pub body: String,
    pub reply_to: Option<ObjectId>,
    /// Child blocks of the chat.
    pub attachments: Vec<BlockRef>,
    /// The hash of the last message its author saw when writing it.
    pub seen: Option<MessageHash>,
    pub signature: Signature,
}

/// A signature over a record's signed bytes, empty until messages are signed
/// (plans/authorship.md).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Signature(#[serde(with = "serde_bytes")] pub Vec<u8>);

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct MessageEdit {
    pub author: Uuid,
    pub time: i64,
    pub body: String,
    pub signature: Signature,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Deletion {
    pub author: Uuid,
    pub time: i64,
    pub signature: Signature,
}

/// One person's reaction to a message: present while they keep it. `reaction` names a Lucide
/// icon.
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(default)]
pub struct ReactionKey {
    pub message: ObjectId,
    pub author: Uuid,
    pub reaction: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Reaction {
    pub time: i64,
    pub signature: Signature,
}

/// A closed segment's entry in the index: its first and last message, their times and how
/// many messages it holds.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Segment {
    pub first: ObjectId,
    pub last: ObjectId,
    pub from: i64,
    pub to: i64,
    pub count: u64,
}

impl Message {
    pub fn new(author: Uuid, time: i64, body: impl Into<String>) -> Self {
        Self {
            author,
            time,
            body: body.into(),
            ..Self::default()
        }
    }

    pub fn replying_to(self, message: Option<ObjectId>) -> Self {
        Self {
            reply_to: message,
            ..self
        }
    }

    /// What its author's signature covers: the chat, the message, author, claimed time, the
    /// body's hash, what it replies to, its attachments and the last message its author saw.
    pub fn signed_bytes(&self, chat: Uuid, id: ObjectId) -> Vec<u8> {
        Signed::new("be3.chat.message", chat, id)
            .uuid(self.author)
            .time(self.time)
            .body(&self.body)
            .reply(self.reply_to)
            .blocks(&self.attachments)
            .seen(self.seen)
            .finish()
    }

    pub fn hash(&self, chat: Uuid, id: ObjectId) -> MessageHash {
        MessageHash::of(&self.signed_bytes(chat, id))
    }
}

impl MessageEdit {
    pub fn signed_bytes(&self, chat: Uuid, message: ObjectId) -> Vec<u8> {
        Signed::new("be3.chat.edit", chat, message)
            .uuid(self.author)
            .time(self.time)
            .body(&self.body)
            .finish()
    }
}

impl Deletion {
    pub fn signed_bytes(&self, chat: Uuid, message: ObjectId) -> Vec<u8> {
        Signed::new("be3.chat.deletion", chat, message)
            .uuid(self.author)
            .time(self.time)
            .finish()
    }
}

impl ReactionKey {
    pub fn signed_bytes(&self, chat: Uuid, reaction: &Reaction) -> Vec<u8> {
        Signed::new("be3.chat.reaction", chat, self.message)
            .uuid(self.author)
            .time(reaction.time)
            .text(&self.reaction)
            .finish()
    }
}

impl Root for ChatHeader {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6368_6174_2d62_6c6f_636b_2d74_7970_6531);
}

/// A chat block's content, written by hand rather than derived, as `Blob` is: it wraps the
/// header document, the only part a replica loads (`Loads::FirstPart`), and forwards
/// `LiveEdit`, `Merge` and `Undo` to it. The segments (plans/chat.md, step 1d) will be the
/// parts after it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ChatContent {
    header: Document<ChatHeader>,
}

impl ChatContent {
    pub const fn header(&self) -> &Document<ChatHeader> {
        &self.header
    }

    /// The recent messages' ids, oldest first.
    pub fn messages(&self) -> Vec<ObjectId> {
        self.header.ids(ObjectId::ROOT, ChatHeader::MESSAGES)
    }

    pub fn message(&self, id: ObjectId) -> Option<Message> {
        self.header.read::<Message>(id)
    }

    pub fn edit_of(&self, message: ObjectId) -> Option<MessageEdit> {
        self.header
            .field(ObjectId::ROOT, ChatHeader::EDITS)
            .get(&message)
            .cloned()
    }

    pub fn deletion_of(&self, message: ObjectId) -> Option<Deletion> {
        self.header
            .field(ObjectId::ROOT, ChatHeader::DELETIONS)
            .get(&message)
            .cloned()
    }

    /// What a message says now: its latest edit's body, or what it was sent with.
    pub fn body(&self, message: ObjectId) -> Option<String> {
        let sent = self.message(message)?;
        Some(self.edit_of(message).map_or(sent.body, |edit| edit.body))
    }

    /// Each reaction to `message` with who reacted, in the order of the reactions' names.
    pub fn reactions(&self, message: ObjectId) -> Vec<(String, Vec<Uuid>)> {
        let mut grouped: Vec<(String, Vec<Uuid>)> = Vec::new();
        for key in self.reaction_keys(message) {
            match grouped.iter_mut().find(|(name, _)| *name == key.reaction) {
                Some((_, authors)) => authors.push(key.author),
                None => grouped.push((key.reaction, vec![key.author])),
            }
        }
        grouped.sort_by(|(left, _), (right, _)| left.cmp(right));
        grouped
    }

    fn reaction_keys(&self, message: ObjectId) -> Vec<ReactionKey> {
        self.header
            .field(ObjectId::ROOT, ChatHeader::REACTIONS)
            .keys()
            .filter(|key| key.message == message)
            .cloned()
            .collect()
    }

    pub fn hash(&self, chat: Uuid, message: ObjectId) -> Option<MessageHash> {
        Some(self.message(message)?.hash(chat, message))
    }

    /// Sends `message` after every message there is, noting the last one as what its author
    /// saw.
    pub fn send(&self, chat: Uuid, message: Message) -> (ObjectId, Edit) {
        let seen = self
            .messages()
            .last()
            .and_then(|last| self.hash(chat, *last));
        let message = Message { seen, ..message };
        let (id, change) = ChatHeader::MESSAGES.insert(ObjectId::ROOT, Anchor::End, &message);
        (id, change.into())
    }

    pub fn edit(message: ObjectId, author: Uuid, time: i64, body: impl Into<String>) -> Edit {
        let edit = MessageEdit {
            author,
            time,
            body: body.into(),
            signature: Signature::default(),
        };
        ChatHeader::EDITS
            .put(ObjectId::ROOT, &message, Some(&edit))
            .into()
    }

    /// Removes `message` with its edit and reactions, and records who deleted it.
    pub fn delete(&self, message: ObjectId, author: Uuid, time: i64) -> Edit {
        let deletion = Deletion {
            author,
            time,
            signature: Signature::default(),
        };
        let mut changes =
            vec![ChatHeader::DELETIONS.put(ObjectId::ROOT, &message, Some(&deletion))];
        if self.message(message).is_some() {
            changes.push(Change::remove(message));
        }
        if self.edit_of(message).is_some() {
            changes.push(ChatHeader::EDITS.put(ObjectId::ROOT, &message, None));
        }
        changes.extend(
            self.reaction_keys(message)
                .iter()
                .map(|key| ChatHeader::REACTIONS.put(ObjectId::ROOT, key, None)),
        );
        changes.into_iter().collect()
    }

    pub fn react(message: ObjectId, author: Uuid, reaction: impl Into<String>, time: i64) -> Edit {
        let key = ReactionKey {
            message,
            author,
            reaction: reaction.into(),
        };
        let reaction = Reaction {
            time,
            signature: Signature::default(),
        };
        ChatHeader::REACTIONS
            .put(ObjectId::ROOT, &key, Some(&reaction))
            .into()
    }

    pub fn unreact(message: ObjectId, author: Uuid, reaction: impl Into<String>) -> Edit {
        let key = ReactionKey {
            message,
            author,
            reaction: reaction.into(),
        };
        ChatHeader::REACTIONS.put(ObjectId::ROOT, &key, None).into()
    }
}

impl BlockContent for ChatContent {
    const CONTENT_TYPE: Uuid = ChatHeader::CONTENT_TYPE;

    const LOADS: Loads = Loads::FirstPart;

    fn schema() -> String {
        format!("chat 1\n{}", ChatHeader::kind().describe())
    }

    fn encode(&self) -> Vec<u8> {
        self.header.to_bytes()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        Document::from_bytes(bytes)
            .map(|header| Self { header })
            .map_err(|_| ContentError::Malformed("chat header"))
    }

    /// The header as the first part; the segments will follow it.
    fn parts(&self) -> Part<'_> {
        Part::node(vec![self.header.vault_parts()])
    }

    fn from_stored(stored: &Stored) -> Result<Self, ContentError> {
        let header = match stored.root.as_ref().and_then(|root| root.children.first()) {
            Some(first) => Document::from_stored(&Stored {
                bytes: stored.bytes_of(first).to_vec(),
                root: Some(first.clone()),
            }),
            None => Document::from_bytes(&stored.bytes),
        };
        header
            .map(|header| Self { header })
            .map_err(|_| ContentError::Malformed("chat header"))
    }

    fn references(&self) -> Vec<Uuid> {
        self.header.references()
    }

    fn references_in(&self, organization: Uuid) -> Vec<Uuid> {
        self.header.references_in(organization)
    }

    fn name(&self) -> Option<String> {
        BlockContent::name(&self.header)
    }

    fn describe(&self, metadata: &mut BlockMetadata) {
        BlockContent::describe(&self.header, metadata);
    }
}

impl LiveEdit for ChatContent {
    type Op = Edit;

    fn apply(&mut self, operation: &Edit) {
        LiveEdit::apply(&mut self.header, operation);
    }

    fn apply_touching(&mut self, operation: &Edit, touched: &mut Vec<Touched>) {
        self.header.apply_touching(operation, touched);
    }

    fn apply_beneath(
        &mut self,
        confirmed: &mut Self,
        operation: &Edit,
        local: &[Edit],
        touched: &mut Vec<Touched>,
    ) {
        self.header
            .apply_beneath(&mut confirmed.header, operation, local, touched);
    }

    fn child_operations(&self, change: ChildChange) -> Option<Vec<Edit>> {
        self.header.child_operations(change)
    }

    fn absorb_operation(operation: &mut Edit, next: Edit) -> Option<Edit> {
        <Document<ChatHeader> as LiveEdit>::absorb_operation(operation, next)
    }

    fn carry_over(operation: &Edit, before: &Self, after: &Self, client: u64) -> Option<Edit> {
        <Document<ChatHeader> as LiveEdit>::carry_over(
            operation,
            &before.header,
            &after.header,
            client,
        )
    }

    fn session_state(&self) -> Vec<u8> {
        self.header.session_state()
    }

    fn adopt_session_state(&mut self, state: &[u8]) -> Result<(), ContentError> {
        LiveEdit::adopt_session_state(&mut self.header, state)
    }
}

impl Merge for ChatContent {
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> MergeResult<Self> {
        match <Document<ChatHeader> as Merge>::merge3(&base.header, &ours.header, &theirs.header) {
            MergeResult::Clean(header) => MergeResult::Clean(Self { header }),
            MergeResult::Conflicted { value, conflicts } => MergeResult::Conflicted {
                value: Self { header: value },
                conflicts,
            },
        }
    }
}

impl Undo for ChatContent {
    type Step = Step;

    fn step(&self, operation: &Edit) -> Option<Step> {
        Undo::step(&self.header, operation)
    }

    fn absorb(previous: &mut Step, next: Step) -> Result<(), Step> {
        <Document<ChatHeader> as Undo>::absorb(previous, next)
    }

    fn then(previous: &mut Step, next: Step) {
        <Document<ChatHeader> as Undo>::then(previous, next);
    }

    fn revert(&self, step: &Step) -> Vec<Edit> {
        self.header.revert(step)
    }

    fn reapply(&self, step: &Step) -> Vec<Edit> {
        self.header.reapply(step)
    }
}
