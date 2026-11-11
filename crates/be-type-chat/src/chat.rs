use be_block::{
    BlockContent, BlockMetadata, ChildChange, ContentError, LiveEdit, Loads, Merge, Part, Root,
    Stored, Undo,
};
use be_block_commit::MergeResult;
use std::collections::BTreeMap;

use be_block_model::{
    Anchor, BlockRef, Change, Document, Edit, LatestMap, List, Model, ObjectId, Place, Series,
    Step, Touched,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::signed::{MessageHash, Signed};

/// The chat's header, the first part of its stored form and the only one 1a writes: the
/// recent messages, and every edit, deletion and reaction, kept apart from the messages and
/// keyed by message id, so a message that moves into a segment keeps them. Those three are
/// `LatestMap`s: the later write of a key wins whichever side made it, so an edit, deletion or
/// reaction made offline on two devices never conflicts.
#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
#[model(kind = "chat")]
pub struct ChatHeader {
    /// When a device merges what it wrote offline onto the published head, the published
    /// messages keep their place and its own follow them.
    #[model(merge_appends)]
    pub messages: List<Message>,
    /// The latest edit of each message, by its id.
    pub edits: LatestMap<ObjectId, MessageEdit>,
    /// Each message deleted, by its id. The message itself is removed (it stays in version
    /// history); this keeps who deleted it and when, which a message kept in a segment needs.
    pub deletions: LatestMap<ObjectId, Deletion>,
    pub reactions: LatestMap<ReactionKey, Reaction>,
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
    /// The message before it when it was deleted, so a place kept by message id (where
    /// someone read up to) is found by order once it is gone. Not signed: a forged one, from
    /// anyone who may write the chat, only miscounts what is unread.
    pub after: Option<ObjectId>,
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
    /// What an edit's signature covers: the chat, the message and its hash (so an edit signed
    /// for one message never verifies on another that took its id), author, claimed time and
    /// the new body's hash.
    pub fn signed_bytes(&self, chat: Uuid, message: ObjectId, of: MessageHash) -> Vec<u8> {
        Signed::new("be3.chat.edit", chat, message)
            .seen(Some(of))
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

    /// Sending and deleting are not undone: a message others may have read is taken back by
    /// deleting it, which leaves a `Deletion`, and once it is in a segment it could not be
    /// put back. Edits, deletions and reactions are `LatestMap` stamps, which never undo.
    fn undoable(change: &Change) -> bool {
        !matches!(
            change,
            Change::Insert { .. }
                | Change::Remove { .. }
                | Change::RemoveIf { .. }
                | Change::Move { .. }
                | Change::MoveRange { .. }
        )
    }
}

/// A chat block's content, written by hand rather than derived, as `Blob` is: it wraps the
/// header document, the only part a replica loads (`Loads::FirstPart`), and forwards
/// `LiveEdit`, `Merge` and `Undo` to it. The segments (plans/chat.md, step 1d) will be the
/// parts after it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ChatContent {
    header: Document<ChatHeader>,
}

/// A message as an editor shows it: what it says now and who reacted how.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    pub id: ObjectId,
    pub message: Message,
    pub body: String,
    pub edited: bool,
    /// Each reaction's name with who reacted, by name.
    pub reactions: Vec<(String, Vec<Uuid>)>,
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

    fn edits(&self) -> LatestMap<ObjectId, MessageEdit> {
        self.header.field(ObjectId::ROOT, ChatHeader::EDITS)
    }

    fn deletions(&self) -> LatestMap<ObjectId, Deletion> {
        self.header.field(ObjectId::ROOT, ChatHeader::DELETIONS)
    }

    fn reaction_map(&self) -> LatestMap<ReactionKey, Reaction> {
        self.header.field(ObjectId::ROOT, ChatHeader::REACTIONS)
    }

    /// The latest edit of `message` by its own author: an edit by anyone else is not one (a
    /// signature would not verify), and a deleted message has none.
    pub fn edit_of(&self, message: ObjectId) -> Option<MessageEdit> {
        let author = self.message(message)?.author;
        self.edits()
            .get(&message)
            .filter(|edit| edit.author == author)
            .cloned()
    }

    pub fn deletion_of(&self, message: ObjectId) -> Option<Deletion> {
        self.deletions().get(&message).cloned()
    }

    /// What a message says now: its latest edit's body, or what it was sent with.
    pub fn body(&self, message: ObjectId) -> Option<String> {
        let sent = self.message(message)?;
        Some(self.edit_of(message).map_or(sent.body, |edit| edit.body))
    }

    /// Each reaction to `message` with who reacted, by the reactions' names.
    pub fn reactions(&self, message: ObjectId) -> Vec<(String, Vec<Uuid>)> {
        let mut grouped = grouped_reactions(&self.reaction_map(), &self.deletions());
        grouped.remove(&message).unwrap_or_default()
    }

    /// Every recent message as an editor shows it, reading each map once.
    pub fn shown(&self) -> Vec<Shown> {
        let edits = self.edits();
        let mut reactions = grouped_reactions(&self.reaction_map(), &self.deletions());
        self.messages()
            .into_iter()
            .filter_map(|id| {
                let message = self.message(id)?;
                let edit = edits.get(&id).filter(|edit| edit.author == message.author);
                Some(Shown {
                    id,
                    body: edit.map_or_else(|| message.body.clone(), |edit| edit.body.clone()),
                    edited: edit.is_some(),
                    reactions: reactions.remove(&id).unwrap_or_default(),
                    message,
                })
            })
            .collect()
    }

    /// The messages `operation` sends: what an editor marks as sending while the server lacks
    /// the edit (`ContentProjection::lacking`).
    pub fn sent_in(operation: &Edit) -> Vec<ObjectId> {
        let messages = Place {
            object: ObjectId::ROOT,
            field: ChatHeader::MESSAGES.index(),
        };
        operation
            .0
            .iter()
            .flat_map(|change| match change {
                Change::Insert { place, objects, .. } if *place == messages => {
                    objects.iter().map(|(id, _)| *id).collect()
                }
                _ => Vec::new(),
            })
            .collect()
    }

    /// The newest message, as where someone who read it has read up to.
    pub fn newest(&self) -> Option<ObjectId> {
        self.messages().last().copied()
    }

    /// How many of `messages` (the recent ones, in order) someone who read up to `read` has
    /// read: up to it while it is here; once deleted, up to the message before it, by the
    /// `Deletion` it left. None when this replica knows nothing of it: another device read a
    /// message not yet here, so everything here is read.
    fn read_through(&self, messages: &[ObjectId], read: ObjectId) -> Option<usize> {
        let deletions = self.deletions();
        let mut at = read;
        for _ in 0..=deletions.len() {
            if let Some(index) = messages.iter().position(|id| *id == at) {
                return Some(index + 1);
            }
            match deletions.get(&at)?.after {
                Some(before) => at = before,
                None => return Some(0),
            }
        }
        Some(0)
    }

    /// Whether `read` is further on than `than`: what reading up to it would move forward.
    pub fn reads_further(&self, read: ObjectId, than: Option<ObjectId>) -> bool {
        let messages = self.messages();
        let Some(read) = self.read_through(&messages, read) else {
            return false;
        };
        than.is_none_or(|than| {
            self.read_through(&messages, than)
                .is_some_and(|than| read > than)
        })
    }

    /// Whether `place` is known here and not further on than `than`: a place reading up to
    /// `than` makes it count for nothing.
    pub fn behind(&self, place: ObjectId, than: ObjectId) -> bool {
        let messages = self.messages();
        match (
            self.read_through(&messages, place),
            self.read_through(&messages, than),
        ) {
            (Some(place), Some(than)) => place <= than,
            _ => false,
        }
    }

    /// How many messages others sent after the furthest of `reads` (where each device read up
    /// to) and after `me`'s own newest message, since answering is reading: every one of others'
    /// messages when nothing was read and `me` sent none.
    pub fn unread(&self, reads: impl IntoIterator<Item = ObjectId>, me: Uuid) -> usize {
        let messages = self.messages();
        let mut from = 0;
        for read in reads {
            match self.read_through(&messages, read) {
                Some(through) => from = from.max(through),
                None => return 0,
            }
        }
        let authors: Vec<Uuid> = messages[from..]
            .iter()
            .map(|id| self.header.field(*id, Message::AUTHOR))
            .collect();
        let after_mine = authors
            .iter()
            .rposition(|author| *author == me)
            .map_or(0, |mine| mine + 1);
        authors[after_mine..]
            .iter()
            .filter(|author| **author != me)
            .count()
    }

    pub fn hash(&self, chat: Uuid, message: ObjectId) -> Option<MessageHash> {
        Some(self.message(message)?.hash(chat, message))
    }

    /// Sends `message` after every message there is, noting the last one as what its author
    /// saw.
    pub fn send(&self, chat: Uuid, message: Message) -> (ObjectId, Edit) {
        let (id, message) = self.draft(chat, message);
        (id, Self::send_as(id, &message))
    }

    /// The id and message `send` would send, for its author to sign
    /// (`Message::signed_bytes`) before `send_as` sends it.
    pub fn draft(&self, chat: Uuid, message: Message) -> (ObjectId, Message) {
        let seen = self
            .messages()
            .last()
            .and_then(|last| self.hash(chat, *last));
        (ObjectId::new(), Message { seen, ..message })
    }

    /// Sends `message` as `id`, after every message there is.
    pub fn send_as(id: ObjectId, message: &Message) -> Edit {
        ChatHeader::MESSAGES
            .insert_as(id, ObjectId::ROOT, Anchor::End, message)
            .into()
    }

    pub fn edit(
        &self,
        message: ObjectId,
        author: Uuid,
        time: i64,
        body: impl Into<String>,
    ) -> Edit {
        let edit = MessageEdit {
            author,
            time,
            body: body.into(),
            signature: Signature::default(),
        };
        self.put_edit(message, &edit)
    }

    /// Makes `edit` the latest of `message`, signed or not.
    pub fn put_edit(&self, message: ObjectId, edit: &MessageEdit) -> Edit {
        let stamp = self
            .edits()
            .next(&message, stamp_time(edit.time), edit.author);
        ChatHeader::EDITS
            .put(ObjectId::ROOT, &message, Some(edit), stamp)
            .into()
    }

    /// Removes `message` with its edit and reactions, and records who deleted it.
    pub fn delete(&self, message: ObjectId, author: Uuid, time: i64) -> Edit {
        let messages = self.messages();
        let after = messages
            .iter()
            .position(|id| *id == message)
            .and_then(|at| at.checked_sub(1))
            .map(|before| messages[before]);
        let deletion = Deletion {
            author,
            time,
            signature: Signature::default(),
            after,
        };
        let at = stamp_time(time);
        let (edits, deletions, reactions) = (self.edits(), self.deletions(), self.reaction_map());
        let mut changes = vec![ChatHeader::DELETIONS.put(
            ObjectId::ROOT,
            &message,
            Some(&deletion),
            deletions.next(&message, at, author),
        )];
        if self.message(message).is_some() {
            changes.push(Change::remove(message));
        }
        if edits.contains_key(&message) {
            let stamp = edits.next(&message, at, author);
            changes.push(ChatHeader::EDITS.put(ObjectId::ROOT, &message, None, stamp));
        }
        changes.extend(
            reactions
                .keys()
                .filter(|key| key.message == message)
                .map(|key| {
                    let stamp = reactions.next(key, at, author);
                    ChatHeader::REACTIONS.put(ObjectId::ROOT, key, None, stamp)
                }),
        );
        changes.into_iter().collect()
    }

    pub fn react(
        &self,
        message: ObjectId,
        author: Uuid,
        reaction: impl Into<String>,
        time: i64,
    ) -> Edit {
        let key = ReactionKey {
            message,
            author,
            reaction: reaction.into(),
        };
        let stamp = self.reaction_map().next(&key, stamp_time(time), author);
        let reaction = Reaction {
            time,
            signature: Signature::default(),
        };
        ChatHeader::REACTIONS
            .put(ObjectId::ROOT, &key, Some(&reaction), stamp)
            .into()
    }

    pub fn unreact(
        &self,
        message: ObjectId,
        author: Uuid,
        reaction: impl Into<String>,
        time: i64,
    ) -> Edit {
        let key = ReactionKey {
            message,
            author,
            reaction: reaction.into(),
        };
        let stamp = self.reaction_map().next(&key, stamp_time(time), author);
        ChatHeader::REACTIONS
            .put(ObjectId::ROOT, &key, None, stamp)
            .into()
    }
}

fn stamp_time(time: i64) -> u64 {
    u64::try_from(time).unwrap_or(0)
}

/// Reactions grouped by message and then by name, leaving out those to deleted messages.
fn grouped_reactions(
    reactions: &LatestMap<ReactionKey, Reaction>,
    deletions: &LatestMap<ObjectId, Deletion>,
) -> BTreeMap<ObjectId, Vec<(String, Vec<Uuid>)>> {
    let mut grouped: BTreeMap<ObjectId, BTreeMap<String, Vec<Uuid>>> = BTreeMap::new();
    for key in reactions
        .keys()
        .filter(|key| !deletions.contains_key(&key.message))
    {
        grouped
            .entry(key.message)
            .or_default()
            .entry(key.reaction.clone())
            .or_default()
            .push(key.author);
    }
    grouped
        .into_iter()
        .map(|(message, names)| (message, names.into_iter().collect()))
        .collect()
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
