use be_block::{
    BlockContent, BlockMetadata, ChildChange, ContentError, LiveEdit, Loads, Merge, MergeParts,
    Part, PartReader, PartRef, Root, Stored, Undo,
};
use be_block_commit::MergeResult;
use std::collections::BTreeMap;

use be_block_model::{
    Anchor, BlockRef, Change, Document, Edit, LatestMap, List, Model, ObjectId, Place, Series,
    Step, Touched,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::segment;
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
    /// The keys of edits and reaction records a later one replaced (`MessageEdit::previous`,
    /// `Reaction::previous`): a record whose key was ever written here, whatever its value
    /// says now, was put back after it was replaced, so it is shown as forged.
    pub superseded: LatestMap<MessageHash, ()>,
    /// The closed segments, in time order (a merge keeps it: `order_segments`). None is closed
    /// yet.
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
    /// The key of the edit this one replaces (`MessageEdit::key`), none for a message's
    /// first: signed, so each edit names the one before it.
    pub previous: Option<MessageHash>,
    pub signature: Signature,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Deletion {
    pub author: Uuid,
    pub time: i64,
    pub signature: Signature,
    /// The message before it when it was deleted, so a place kept by message id (where
    /// someone read up to) is found by order once it is gone, and a forged deletion's row is
    /// placed. Signed.
    pub after: Option<ObjectId>,
}

/// One person's reaction to a message, kept or taken back (`Reaction::kept`). `reaction` names
/// a Lucide icon.
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
    /// False once its author took it back: a taken-back reaction stays as a signed record, so
    /// nobody else can remove one unnoticed.
    pub kept: bool,
    /// The key of the record of this key it replaces (`Reaction::key`), none for the first:
    /// signed, so an older record put back over a newer one is caught.
    pub previous: Option<MessageHash>,
    pub signature: Signature,
}

/// A closed segment's entry in the index: its first and last message, their times, how many
/// messages it holds, and what its part must be (its length and hash), so a read of it is
/// checked and an editor finds it by range without reading it. `members` lets a reader tell
/// whether a message may be in it (a Bloom filter, `Segment::may_hold`).
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(default)]
pub struct Segment {
    pub first: ObjectId,
    pub last: ObjectId,
    pub from: i64,
    pub to: i64,
    pub count: u64,
    /// The part's length in bytes.
    pub length: u64,
    /// SHA-256 of the part's bytes.
    #[serde(with = "serde_bytes")]
    pub hash: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub members: Vec<u8>,
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
    /// for one message never verifies on another that took its id), author, claimed time,
    /// the new body's hash and the edit it replaces.
    pub fn signed_bytes(&self, chat: Uuid, message: ObjectId, of: MessageHash) -> Vec<u8> {
        Signed::new("be3.chat.edit", chat, message)
            .seen(Some(of))
            .uuid(self.author)
            .time(self.time)
            .body(&self.body)
            .seen(self.previous)
            .finish()
    }

    /// What names this edit of `message` in `superseded` and in the next edit's `previous`:
    /// the hash of its fields, not of its signed bytes, so an editor checks it before it knows
    /// the block signatures bind to.
    pub fn key(&self, message: ObjectId) -> MessageHash {
        MessageHash::of(
            &Signed::new("be3.chat.edit.key", Uuid::nil(), message)
                .uuid(self.author)
                .time(self.time)
                .body(&self.body)
                .seen(self.previous)
                .finish(),
        )
    }
}

impl Deletion {
    pub fn signed_bytes(&self, chat: Uuid, message: ObjectId) -> Vec<u8> {
        Signed::new("be3.chat.deletion", chat, message)
            .uuid(self.author)
            .time(self.time)
            .reply(self.after)
            .finish()
    }
}

impl ReactionKey {
    pub fn signed_bytes(&self, chat: Uuid, reaction: &Reaction) -> Vec<u8> {
        Signed::new("be3.chat.reaction", chat, self.message)
            .uuid(self.author)
            .time(reaction.time)
            .text(&self.reaction)
            .flag(reaction.kept)
            .seen(reaction.previous)
            .finish()
    }
}

impl Reaction {
    /// What names this record of `key` in `superseded` and in the next record's `previous`,
    /// as `MessageEdit::key` does for an edit.
    pub fn key(&self, key: &ReactionKey) -> MessageHash {
        MessageHash::of(
            &Signed::new("be3.chat.reaction.key", Uuid::nil(), key.message)
                .uuid(key.author)
                .text(&key.reaction)
                .time(self.time)
                .flag(self.kept)
                .seen(self.previous)
                .finish(),
        )
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
/// header document, the only part a replica loads (`Loads::FirstPart`), and the segments after
/// it, which are the other parts. It forwards `LiveEdit`, `Merge` and `Undo` to the header.
#[derive(Clone, Debug, Default)]
pub struct ChatContent {
    header: Document<ChatHeader>,
    /// Each closed segment's part, in the order of the header's index.
    held: Vec<Held>,
}

impl PartialEq for ChatContent {
    fn eq(&self, other: &Self) -> bool {
        self.header == other.header
    }
}

impl Eq for ChatContent {}

/// A closed segment's part: its bytes, when this replica closed it or saw it closed and has not
/// saved it yet; the part a save reuses; or neither, for an editor that was sent only the header.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Held {
    Bytes(Vec<u8>),
    Saved(PartRef),
    Absent,
}

/// An operation on a chat.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ChatOp {
    /// A change to the header: a message sent, edited or deleted, a reaction.
    Edit(Edit),
    /// The oldest recent messages become a segment: the header's change (they leave its
    /// messages, the index gains an entry) and the segment's bytes. Every replica that applies
    /// it holds the bytes, so an owner that takes the session over can save the part.
    Close(Closing),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Closing {
    pub edit: Edit,
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
}

impl From<Edit> for ChatOp {
    fn from(edit: Edit) -> Self {
        Self::Edit(edit)
    }
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

    fn superseded(&self) -> LatestMap<MessageHash, ()> {
        self.header.field(ObjectId::ROOT, ChatHeader::SUPERSEDED)
    }

    /// The latest edit of `message` if `author` made it, for a message recent or closed.
    pub fn edit_by(&self, message: ObjectId, author: Uuid) -> Option<MessageEdit> {
        self.edits()
            .get(&message)
            .filter(|edit| edit.author == author)
            .cloned()
    }

    /// Whether the edit or reaction record `key` names was replaced: shown again, it was put
    /// back.
    pub fn replaced(&self, key: MessageHash) -> bool {
        self.superseded().written(&key)
    }

    /// Whether `message`'s edit record was written and is not an edit by `author` now: removed
    /// outright, or overwritten by someone else's, which no signature of the author's covers.
    pub fn edit_removed(&self, message: ObjectId, author: Uuid) -> bool {
        let edits = self.edits();
        edits.written(&message) && edits.get(&message).is_none_or(|edit| edit.author != author)
    }

    /// Whether `message`'s deletion was written and then removed, which brings back a message
    /// in a closed segment with no signature covering it.
    pub fn deletion_removed(&self, message: ObjectId) -> bool {
        let deletions = self.deletions();
        deletions.written(&message) && !deletions.contains_key(&message)
    }

    /// Every deletion, by the id of the message it deleted.
    pub fn deletions_made(&self) -> Vec<(ObjectId, Deletion)> {
        self.deletions()
            .iter()
            .map(|(message, deletion)| (*message, deletion.clone()))
            .collect()
    }

    /// Every reaction record to `message`: kept, taken back, or removed (none), which its
    /// author's signature cannot cover.
    pub fn reaction_records(&self, message: ObjectId) -> Vec<(ReactionKey, Option<Reaction>)> {
        self.reaction_map()
            .written_entries()
            .filter(|(key, _)| key.message == message)
            .map(|(key, reaction)| (key.clone(), reaction.cloned()))
            .collect()
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
        let recent: Vec<(ObjectId, Message)> = self
            .messages()
            .into_iter()
            .filter_map(|id| Some((id, self.message(id)?)))
            .collect();
        self.shown_of(recent)
    }

    /// `messages` (recent ones, or those a closed segment holds) as an editor shows them: what
    /// each says now and who reacted how, leaving out those deleted since they were closed.
    pub fn shown_of(&self, messages: Vec<(ObjectId, Message)>) -> Vec<Shown> {
        let edits = self.edits();
        let deletions = self.deletions();
        let mut reactions = grouped_reactions(&self.reaction_map(), &deletions);
        messages
            .into_iter()
            .filter(|(id, _)| !deletions.contains_key(id))
            .map(|(id, message)| {
                let edit = edits.get(&id).filter(|edit| edit.author == message.author);
                Shown {
                    id,
                    body: edit.map_or_else(|| message.body.clone(), |edit| edit.body.clone()),
                    edited: edit.is_some(),
                    reactions: reactions.remove(&id).unwrap_or_default(),
                    message,
                }
            })
            .collect()
    }

    /// The messages `operation` sends: what an editor marks as sending while the server lacks
    /// the edit (`ContentProjection::lacking`).
    pub fn sent_in(operation: &ChatOp) -> Vec<ObjectId> {
        let ChatOp::Edit(operation) = operation else {
            return Vec::new();
        };
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
    /// `Deletion` it left; none of them when it may be in a closed segment. None when this
    /// replica knows nothing of it: another device read a message not yet here, so everything
    /// here is read.
    fn read_through(&self, messages: &[ObjectId], read: ObjectId) -> Option<usize> {
        let deletions = self.deletions();
        let mut at = read;
        for _ in 0..=deletions.len() {
            if let Some(index) = messages.iter().position(|id| *id == at) {
                return Some(index + 1);
            }
            // A message in a closed segment is before every recent one: a replica does not read
            // the segment to count what is unread after it, so only the recent messages count.
            let Some(deletion) = deletions.get(&at) else {
                return self.may_be_closed(at).then_some(0);
            };
            match deletion.after {
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

    /// An unsigned edit of `message` (`next_edit`, then `put_edit`).
    pub fn edit(
        &self,
        message: ObjectId,
        author: Uuid,
        time: i64,
        body: impl Into<String>,
    ) -> Edit {
        self.put_edit(message, &self.next_edit(message, author, time, body))
    }

    /// The edit of `message` that replaces its latest, for its author to sign
    /// (`MessageEdit::signed_bytes`) before `put_edit` writes it.
    pub fn next_edit(
        &self,
        message: ObjectId,
        author: Uuid,
        time: i64,
        body: impl Into<String>,
    ) -> MessageEdit {
        let previous = self
            .edits()
            .get(&message)
            .filter(|edit| edit.author == author)
            .map(|edit| edit.key(message));
        MessageEdit {
            author,
            time,
            body: body.into(),
            previous,
            signature: Signature::default(),
        }
    }

    /// Makes `edit` the latest of `message`, signed or not, and records the edit it replaces as
    /// replaced.
    pub fn put_edit(&self, message: ObjectId, edit: &MessageEdit) -> Edit {
        let at = stamp_time(edit.time);
        let stamp = self.edits().next(&message, at, edit.author);
        let mut changes = vec![ChatHeader::EDITS.put(ObjectId::ROOT, &message, Some(edit), stamp)];
        changes.extend(self.supersede(edit.previous, at, edit.author));
        changes.into_iter().collect()
    }

    /// Records `previous` as replaced, if there is one.
    fn supersede(&self, previous: Option<MessageHash>, at: u64, author: Uuid) -> Option<Change> {
        let previous = previous?;
        let stamp = self.superseded().next(&previous, at, author);
        Some(ChatHeader::SUPERSEDED.put(ObjectId::ROOT, &previous, Some(&()), stamp))
    }

    /// An unsigned deletion of `message` (`deletion`, then `put_deletion`).
    pub fn delete(&self, message: ObjectId, author: Uuid, time: i64) -> Edit {
        self.put_deletion(message, &self.deletion(message, author, time))
    }

    /// The deletion of `message`, for its author to sign (`Deletion::signed_bytes`) before
    /// `put_deletion` writes it.
    pub fn deletion(&self, message: ObjectId, author: Uuid, time: i64) -> Deletion {
        let messages = self.messages();
        let after = messages
            .iter()
            .position(|id| *id == message)
            .and_then(|at| at.checked_sub(1))
            .map(|before| messages[before]);
        Deletion {
            author,
            time,
            signature: Signature::default(),
            after,
        }
    }

    /// Removes `message` with its edit and reactions, and records `deletion`, signed or not.
    pub fn put_deletion(&self, message: ObjectId, deletion: &Deletion) -> Edit {
        let (author, time) = (deletion.author, deletion.time);
        let at = stamp_time(time);
        let (edits, deletions, reactions) = (self.edits(), self.deletions(), self.reaction_map());
        let mut changes = vec![ChatHeader::DELETIONS.put(
            ObjectId::ROOT,
            &message,
            Some(deletion),
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

    /// An unsigned reaction to `message` (`reaction`, then `put_reaction`).
    pub fn react(
        &self,
        message: ObjectId,
        author: Uuid,
        reaction: impl Into<String>,
        time: i64,
    ) -> Edit {
        let (key, reaction) = self.reaction(message, author, reaction, time, true);
        self.put_reaction(&key, &reaction)
    }

    /// An unsigned taking back of a reaction to `message`.
    pub fn unreact(
        &self,
        message: ObjectId,
        author: Uuid,
        reaction: impl Into<String>,
        time: i64,
    ) -> Edit {
        let (key, reaction) = self.reaction(message, author, reaction, time, false);
        self.put_reaction(&key, &reaction)
    }

    /// A reaction record, kept or taken back, naming the record it replaces, for its author
    /// to sign (`ReactionKey::signed_bytes`) before `put_reaction` writes it.
    pub fn reaction(
        &self,
        message: ObjectId,
        author: Uuid,
        reaction: impl Into<String>,
        time: i64,
        kept: bool,
    ) -> (ReactionKey, Reaction) {
        let key = ReactionKey {
            message,
            author,
            reaction: reaction.into(),
        };
        let previous = self
            .reaction_map()
            .get(&key)
            .map(|held| held.key(&key));
        let reaction = Reaction {
            time,
            kept,
            previous,
            signature: Signature::default(),
        };
        (key, reaction)
    }

    /// Writes a reaction record, signed or not, and records the one it replaces as replaced.
    pub fn put_reaction(&self, key: &ReactionKey, reaction: &Reaction) -> Edit {
        let at = stamp_time(reaction.time);
        let stamp = self.reaction_map().next(key, at, key.author);
        let mut changes = vec![ChatHeader::REACTIONS.put(ObjectId::ROOT, key, Some(reaction), stamp)];
        changes.extend(self.supersede(reaction.previous, at, key.author));
        changes.into_iter().collect()
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
        .iter()
        .filter(|(key, reaction)| reaction.kept && !deletions.contains_key(&key.message))
        .map(|(key, _)| key)
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

impl ChatContent {
    fn from_header(header: Document<ChatHeader>, held: Vec<Held>) -> Self {
        let mut content = Self { header, held };
        content.align();
        content
    }

    /// Keeps one part per index entry: a replica that has no part for an entry holds none.
    fn align(&mut self) {
        let count = self.segments().len();
        self.held.resize(count, Held::Absent);
    }

    /// The closed segments' index entries, in time order.
    pub fn segments(&self) -> Vec<Segment> {
        self.header
            .field(ObjectId::ROOT, ChatHeader::SEGMENTS)
            .into_vec()
    }

    /// Where each segment's part is in the stored form: after the header, in the order of the
    /// index. What an editor reads by range (`ContentProjection::live().read_range`); it takes
    /// the header's length from the header it holds, since the length a snapshot gave goes stale
    /// as soon as a segment closes.
    pub fn segment_ranges(&self) -> Vec<std::ops::Range<u64>> {
        let mut start = self.header.to_bytes().len() as u64;
        self.segments()
            .iter()
            .map(|segment| {
                let range = start..start.saturating_add(segment.length);
                start = range.end;
                range
            })
            .collect()
    }

    /// The oldest recent messages as a segment to close, when the recent ones pass a size: the
    /// operation that closes them (an editor proposes it only while nothing of its device is
    /// unconfirmed, guides/chat.md, "Segments"). At least `segment::SMALLEST` bytes of messages close at once, and at least
    /// `segment::KEPT_RECENT` stay recent.
    pub fn closing(&self) -> Option<ChatOp> {
        let recent: Vec<(ObjectId, Message)> = self
            .messages()
            .into_iter()
            .map_while(|id| Some((id, self.message(id)?)))
            .collect();
        let sizes: Vec<usize> = recent
            .iter()
            .map(|(id, message)| segment::size_of(*id, message))
            .collect();
        let total: usize = sizes.iter().sum();
        if total < segment::SMALLEST + segment::KEPT_RECENT {
            return None;
        }
        let mut taken = 0;
        let mut count = 0;
        // Whether the next message would pass the largest segment: what was taken closes alone
        // then, even under the smallest, so a large message never stalls closing.
        let mut next_too_large = false;
        for size in &sizes {
            if count > 0 && taken + size > segment::LARGEST {
                next_too_large = true;
                break;
            }
            taken += size;
            count += 1;
            if taken >= segment::SMALLEST {
                break;
            }
        }
        if count == 0
            || (taken < segment::SMALLEST && !next_too_large)
            || total - taken < segment::KEPT_RECENT
        {
            return None;
        }
        let closed = &recent[..count];
        let bytes = segment::encode(closed);
        let entry = Segment::of(closed, &bytes);
        let held = self.header.series(ObjectId::ROOT, ChatHeader::SEGMENTS)?;
        let mut changes: Vec<Change> = closed.iter().map(|(id, _)| Change::remove(*id)).collect();
        changes.push(ChatHeader::SEGMENTS.append(ObjectId::ROOT, held, &[entry])?);
        Some(ChatOp::Close(Closing {
            edit: changes.into_iter().collect(),
            bytes,
        }))
    }

    /// Whether closing `closing` applies here: the messages its bytes hold are the oldest recent
    /// ones, as written, and its edit does exactly this much to the header: it removes them from
    /// the messages and adds the index entry those bytes give (their count, first and last
    /// message, length and hash), so an index never describes bytes it was not made from.
    fn closes_here(&self, closing: &Closing) -> bool {
        let Some(closed) = segment::decode(&closing.bytes) else {
            return false;
        };
        let recent = self.messages();
        if closed.is_empty()
            || closed.len() > recent.len()
            || !closed.iter().zip(&recent).all(|((id, message), recent)| {
                id == recent && self.message(*id).as_ref() == Some(message)
            })
        {
            return false;
        }
        let mut after = self.header.clone();
        LiveEdit::apply(&mut after, &closing.edit);
        let mut expected = self.segments();
        expected.push(Segment::of(&closed, &closing.bytes));
        let now = Self {
            header: after,
            held: Vec::new(),
        };
        now.segments() == expected && now.messages() == recent[closed.len()..]
    }

    fn close(&mut self, closing: &Closing) {
        if !self.closes_here(closing) {
            return;
        }
        LiveEdit::apply(&mut self.header, &closing.edit);
        self.align();
        if let Some(held) = self.held.last_mut() {
            *held = Held::Bytes(closing.bytes.clone());
        }
    }

    /// The part of `segment`, as one side of a merge holds it: matched by the whole entry
    /// (which opens at the same message as another when two closes overlap, but not in count,
    /// last message, length and hash).
    fn held_of(&self, segment: &Segment) -> Option<&Held> {
        let at = self.segments().iter().position(|held| held == segment)?;
        self.held.get(at)
    }

    /// The messages of every closed segment this replica holds as bytes (not those it saved
    /// and holds as parts), in index order, repeating a message that two overlapping segments hold.
    #[cfg(test)]
    pub(crate) fn closed_messages_held_as_bytes(&self) -> Vec<ObjectId> {
        let segments = self.segments();
        self.held
            .iter()
            .zip(&segments)
            .filter_map(|(held, segment)| match held {
                Held::Bytes(bytes) => segment.read(bytes),
                _ => None,
            })
            .flatten()
            .map(|(id, _)| id)
            .collect()
    }

    /// The messages of segments read back (each in the order of its part), as one run of messages
    /// in order, each once, in time linear in the messages: segments closed apart can overlap, and an unseen message goes after
    /// the message before it in its own segment (or before the first of its segment already
    /// there), so the run keeps every segment's order.
    pub fn weave(segments: &[&[(ObjectId, Message)]]) -> Vec<(ObjectId, Message)> {
        // A doubly linked run: placing a message after another is constant time, so the whole
        // weave is linear in the messages.
        type Link = Option<ObjectId>;
        let mut next: std::collections::HashMap<Link, Link> = std::collections::HashMap::new();
        let mut prev: std::collections::HashMap<ObjectId, Link> = std::collections::HashMap::new();
        let mut held: std::collections::HashMap<ObjectId, &Message> =
            std::collections::HashMap::new();
        next.insert(None, None);
        let mut tail: Link = None;
        for segment in segments {
            // Its unseen messages go after the one before its first message already there, or at
            // the end when none is.
            let mut place: Link = match segment.iter().find(|(id, _)| held.contains_key(id)) {
                Some((first, _)) => prev[first],
                None => tail,
            };
            for (id, message) in *segment {
                if held.contains_key(id) {
                    place = Some(*id);
                    continue;
                }
                let following = next[&place];
                next.insert(place, Some(*id));
                next.insert(Some(*id), following);
                prev.insert(*id, place);
                match following {
                    Some(following) => {
                        prev.insert(following, Some(*id));
                    }
                    None => tail = Some(*id),
                }
                held.insert(*id, message);
                place = Some(*id);
            }
        }
        let mut run = Vec::with_capacity(held.len());
        let mut at = next[&None];
        while let Some(id) = at {
            run.push((id, held[&id].clone()));
            at = next[&Some(id)];
        }
        run
    }

    /// Whether `message` may be in a closed segment, rather than recent or unknown here.
    pub fn may_be_closed(&self, message: ObjectId) -> bool {
        self.segments()
            .iter()
            .any(|segment| segment.may_hold(message))
    }

    /// The messages of the segment at `index` as its part's `bytes` hold them, if they are its
    /// part (length and hash match the index).
    pub fn read_segment(&self, index: usize, bytes: &[u8]) -> Option<Vec<(ObjectId, Message)>> {
        self.segments().get(index)?.read(bytes)
    }
}

impl BlockContent for ChatContent {
    const CONTENT_TYPE: Uuid = ChatHeader::CONTENT_TYPE;

    const LOADS: Loads = Loads::FirstPart;

    /// `encode` is the header: the closed segments are parts after it.
    const WHOLE_BYTES: bool = false;

    fn schema() -> String {
        format!("chat 1\n{}", ChatHeader::kind().describe())
    }

    /// The header, the only part a replica sends its editors.
    fn encode(&self) -> Vec<u8> {
        self.header.to_bytes()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        Document::from_bytes(bytes)
            .map(|header| Self::from_header(header, Vec::new()))
            .map_err(|_| ContentError::Malformed("chat header"))
    }

    /// The header, then each closed segment: its bytes while this replica holds them, or the
    /// part it was saved as.
    fn parts(&self) -> Part<'_> {
        if self.held.contains(&Held::Absent) {
            // A replica that lacks a segment's bytes (a chat rebuilt from the header alone) saves
            // the header alone, never a placeholder part: what it loads back still lacks them,
            // and a merge sees that.
            return Part::node(vec![self.header.vault_parts()]);
        }
        let mut parts = vec![self.header.vault_parts()];
        parts.extend(self.held.iter().map(|held| match held {
            Held::Bytes(bytes) => Part::from(bytes),
            Held::Saved(part) => Part::Reused(*part),
            Held::Absent => unreachable!("checked above"),
        }));
        Part::node(parts)
    }

    fn from_stored(stored: &Stored) -> Result<Self, ContentError> {
        let root = stored.root.as_ref();
        let header = match root.and_then(|root| root.children.first()) {
            Some(first) => Document::from_stored(&Stored {
                bytes: stored.bytes_of(first).to_vec(),
                root: Some(first.clone()),
            }),
            None => Document::from_bytes(&stored.bytes),
        };
        let held = root
            .map(|root| {
                root.children
                    .iter()
                    .skip(1)
                    .map(|child| Held::Saved(child.reference))
                    .collect()
            })
            .unwrap_or_default();
        header
            .map(|header| Self::from_header(header, held))
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
    type Op = ChatOp;

    fn apply(&mut self, operation: &ChatOp) {
        match operation {
            ChatOp::Edit(edit) => LiveEdit::apply(&mut self.header, edit),
            ChatOp::Close(closing) => self.close(closing),
        }
    }

    /// Each message the operation sends, as the notification the chat's other members get once
    /// the server holds it: its body, naming the author of the message it replies to, whom a
    /// member who wants only mentions is told for. Keyed by the message, so a send made again
    /// (a replica restarted with the edit) tells once.
    fn announcements(&self, operation: &ChatOp) -> Vec<be_block::Announcement> {
        Self::sent_in(operation)
            .into_iter()
            .filter_map(|id| {
                let message = self.message(id)?;
                let mentions = message
                    .reply_to
                    .and_then(|quoted| self.message(quoted))
                    .map(|quoted| quoted.author)
                    .into_iter()
                    .collect();
                Some(be_block::Announcement {
                    key: id.to_string(),
                    text: self.body(id)?,
                    several: "{count} new messages".to_owned(),
                    mentions,
                })
            })
            .collect()
    }

    fn apply_touching(&mut self, operation: &ChatOp, touched: &mut Vec<Touched>) {
        match operation {
            ChatOp::Edit(edit) => self.header.apply_touching(edit, touched),
            ChatOp::Close(closing) => {
                self.close(closing);
                touched.push(Touched::Everything);
            }
        }
    }

    fn apply_beneath(
        &mut self,
        confirmed: &mut Self,
        operation: &ChatOp,
        local: &[ChatOp],
        touched: &mut Vec<Touched>,
    ) {
        let edits: Option<Vec<Edit>> = local
            .iter()
            .map(|operation| match operation {
                ChatOp::Edit(edit) => Some(edit.clone()),
                ChatOp::Close(_) => None,
            })
            .collect();
        match (operation, edits) {
            (ChatOp::Edit(edit), Some(local)) => {
                self.header
                    .apply_beneath(&mut confirmed.header, edit, &local, touched);
            }
            _ => {
                confirmed.apply(operation);
                *self = confirmed.clone();
                for pending in local {
                    self.apply(pending);
                }
                touched.push(Touched::Everything);
            }
        }
    }

    fn child_operations(&self, change: ChildChange) -> Option<Vec<ChatOp>> {
        self.header
            .child_operations(change)
            .map(|edits| edits.into_iter().map(ChatOp::Edit).collect())
    }

    fn absorb_operation(operation: &mut ChatOp, next: ChatOp) -> Option<ChatOp> {
        match (operation, next) {
            (ChatOp::Edit(operation), ChatOp::Edit(next)) => {
                <Document<ChatHeader> as LiveEdit>::absorb_operation(operation, next)
                    .map(ChatOp::Edit)
            }
            (_, next) => Some(next),
        }
    }

    fn carry_over(operation: &ChatOp, before: &Self, after: &Self, client: u64) -> Option<ChatOp> {
        match operation {
            ChatOp::Edit(edit) => <Document<ChatHeader> as LiveEdit>::carry_over(
                edit,
                &before.header,
                &after.header,
                client,
            )
            .map(ChatOp::Edit),
            ChatOp::Close(closing) => after.closes_here(closing).then(|| operation.clone()),
        }
    }

    fn session_state(&self) -> Vec<u8> {
        self.header.session_state()
    }

    fn adopt_session_state(&mut self, state: &[u8]) -> Result<(), ContentError> {
        LiveEdit::adopt_session_state(&mut self.header, state)
    }
}

/// The bytes of closed segments a merge read: whether the side that holds the segment is theirs,
/// its index entry and its part.
type SegmentBytes = Vec<(bool, Segment, Vec<u8>)>;

impl ChatContent {
    /// The segments each side closed since `base`, as what a merge settles: ours that stand and
    /// theirs that lose. A segment both sides closed is one. When we closed any segment they did
    /// not, ours stand and theirs that differ lose, since their messages may overlap ours.
    fn closes_to_settle(base: &Self, ours: &Self, theirs: &Self) -> (Vec<Segment>, Vec<Segment>) {
        let added = |side: &Self| -> Vec<Segment> {
            let known = base.segments();
            side.segments()
                .into_iter()
                .filter(|segment| !known.contains(segment))
                .collect()
        };
        let ours_added = added(ours);
        let demoted = added(theirs)
            .into_iter()
            .filter(|segment| !ours_added.is_empty() && !ours_added.contains(segment))
            .collect();
        (ours_added, demoted)
    }

    /// The parts `merged_with` needs: those of the segments `closes_to_settle` names, as the side
    /// that holds each has them (`true` for theirs).
    fn settling(base: &Self, ours: &Self, theirs: &Self) -> Vec<(bool, Segment)> {
        let (ours_added, demoted) = Self::closes_to_settle(base, ours, theirs);
        ours_added
            .into_iter()
            .map(|segment| (false, segment))
            .chain(demoted.into_iter().map(|segment| (true, segment)))
            .collect()
    }

    /// The merge, given the parts `settling` names (none: a plain union, which keeps overlapping
    /// entries, each with its own part). The header merges as a document, its conflicts resolved
    /// as it resolves them (a register both sides changed keeps ours; a message object that
    /// differs keeps ours, and messages are immutable). A segment both sides closed is one. A
    /// segment only they closed that differs from ours is dropped from the index and its messages
    /// that are not in ours' segments (and not deleted) go back to the recent ones, so no two
    /// index entries overlap and no message is lost or repeated. A chat merge never conflicts.
    fn merged_with(base: &Self, ours: &Self, theirs: &Self, bytes: &SegmentBytes) -> Self {
        let header = match <Document<ChatHeader> as Merge>::merge3(
            &base.header,
            &ours.header,
            &theirs.header,
        ) {
            MergeResult::Clean(header) | MergeResult::Conflicted { value: header, .. } => header,
        };
        let mut value = Self {
            header,
            held: Vec::new(),
        };
        let (ours_added, demoted) = Self::closes_to_settle(base, ours, theirs);
        let part_of = |is_theirs: bool, segment: &Segment| {
            bytes
                .iter()
                .find(|(theirs, held, _)| *theirs == is_theirs && held == segment)
                .and_then(|(_, _, bytes)| segment.read(bytes))
        };
        let ours_closed: Option<std::collections::HashSet<ObjectId>> = ours_added
            .iter()
            .map(|segment| part_of(false, segment))
            .collect::<Option<Vec<_>>>()
            .map(|read| read.into_iter().flatten().map(|(id, _)| id).collect());
        let recent: std::collections::HashSet<ObjectId> = value.messages().into_iter().collect();
        let mut dropped: Vec<Segment> = Vec::new();
        let mut restored: Vec<(ObjectId, Message)> = Vec::new();
        if let Some(ours_closed) = &ours_closed {
            for segment in &demoted {
                let Some(messages) = part_of(true, segment) else {
                    continue;
                };
                dropped.push(segment.clone());
                for (id, message) in messages {
                    if !recent.contains(&id)
                        && !ours_closed.contains(&id)
                        && value.deletion_of(id).is_none()
                        && !restored.iter().any(|(held, _)| *held == id)
                    {
                        restored.push((id, message));
                    }
                }
            }
        }
        let segments = value.segments();
        let leaving: Vec<usize> = (0..segments.len())
            .filter(|index| {
                segments[..*index].contains(&segments[*index])
                    || dropped.contains(&segments[*index])
            })
            .collect();
        let mut changes: Vec<Change> = Vec::new();
        for index in leaving.into_iter().rev() {
            let Some(held) = value.header.series(ObjectId::ROOT, ChatHeader::SEGMENTS) else {
                continue;
            };
            if let Some(op) = held.delete(index..index + 1) {
                changes.push(ChatHeader::SEGMENTS.edit(ObjectId::ROOT, op));
            }
        }
        // Their messages go back where they were: before the recent ones, in order.
        let mut after: Option<ObjectId> = None;
        for (id, message) in &restored {
            let anchor = after.map_or(Anchor::Start, Anchor::After);
            changes.push(ChatHeader::MESSAGES.insert_as(*id, ObjectId::ROOT, anchor, message));
            after = Some(*id);
        }
        if !changes.is_empty() {
            let edit: Edit = changes.into_iter().collect();
            LiveEdit::apply(&mut value.header, &edit);
        }
        value.order_segments(base);
        value.held = value
            .segments()
            .iter()
            .map(|segment| {
                [ours, theirs, base]
                    .into_iter()
                    .filter_map(|side| side.held_of(segment))
                    .find(|held| **held != Held::Absent)
                    .cloned()
                    .unwrap_or(Held::Absent)
            })
            .collect();
        value
    }
}

impl ChatContent {
    /// Puts the index in time order. Each side's new entries land in the merged index as one
    /// block, either first, so the entries new since `base` are sorted by where the message each
    /// ends at and opens at stood among the recent messages of `base` (unknown ones, sent since,
    /// last); the entries `base` had stay in front, as they were.
    fn order_segments(&mut self, base: &Self) {
        let known: std::collections::HashSet<Segment> = base.segments().into_iter().collect();
        let position: std::collections::HashMap<ObjectId, usize> = base
            .messages()
            .into_iter()
            .enumerate()
            .map(|(at, id)| (id, at))
            .collect();
        let key = |segment: &Segment| {
            (
                position.get(&segment.last).copied().unwrap_or(usize::MAX),
                position.get(&segment.first).copied().unwrap_or(usize::MAX),
            )
        };
        let segments = self.segments();
        let (mut wanted, mut added): (Vec<Segment>, Vec<Segment>) = segments
            .iter()
            .cloned()
            .partition(|segment| known.contains(segment));
        added.sort_by_key(key);
        wanted.extend(added);
        if wanted == segments {
            return;
        }
        // Replaces only what differs from the first entry that moves, so the entries `base` had
        // keep their ids and a merge leaves no tombstone for them.
        let Some(held) = self.header.series(ObjectId::ROOT, ChatHeader::SEGMENTS) else {
            return;
        };
        if let Some(change) = ChatHeader::SEGMENTS.update(ObjectId::ROOT, held, &wanted) {
            let edit: Edit = [change].into_iter().collect();
            LiveEdit::apply(&mut self.header, &edit);
        }
    }
}

impl Merge for ChatContent {
    /// The merge without reading any part (`merged_with` with none): overlapping entries stay,
    /// each with its own part, so nothing is lost; `merge_parts` settles them. Never a conflict.
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> MergeResult<Self> {
        MergeResult::Clean(Self::merged_with(base, ours, theirs, &Vec::new()))
    }
}

impl MergeParts for ChatContent {
    /// The merge reading the parts it settles (`settling`): those a side holds as bytes, or
    /// saved (read as the host fetches them, and the merge waits for them). A segment whose part
    /// no side holds at all is not settled: the union keeps its entries.
    fn merge_parts(
        base: Self,
        ours: Self,
        theirs: Self,
        parts: PartReader,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<MergeResult<Self>, String>>>> {
        Box::pin(async move {
            let mut bytes = SegmentBytes::new();
            for (is_theirs, segment) in Self::settling(&base, &ours, &theirs) {
                // The side that closed it first, then the others: a segment one side holds only
                // as a view can be held as bytes or saved by another.
                let sides = match is_theirs {
                    true => [&theirs, &ours, &base],
                    false => [&ours, &theirs, &base],
                };
                let held = sides
                    .into_iter()
                    .filter_map(|side| side.held_of(&segment))
                    .find(|held| **held != Held::Absent);
                let read = match held {
                    Some(Held::Bytes(held)) => held.clone(),
                    Some(Held::Saved(part)) => parts.read_all(*part).await?,
                    // A segment whose part no side holds (a chat rebuilt from its header) is not
                    // settled: its entries stay as they are.
                    _ => continue,
                };
                bytes.push((is_theirs, segment, read));
            }
            Ok(MergeResult::Clean(Self::merged_with(
                &base, &ours, &theirs, &bytes,
            )))
        })
    }
}

impl Undo for ChatContent {
    type Step = Step;

    fn step(&self, operation: &ChatOp) -> Option<Step> {
        match operation {
            ChatOp::Edit(edit) => Undo::step(&self.header, edit),
            ChatOp::Close(_) => None,
        }
    }

    fn absorb(previous: &mut Step, next: Step) -> Result<(), Step> {
        <Document<ChatHeader> as Undo>::absorb(previous, next)
    }

    fn then(previous: &mut Step, next: Step) {
        <Document<ChatHeader> as Undo>::then(previous, next);
    }

    fn revert(&self, step: &Step) -> Vec<ChatOp> {
        self.header
            .revert(step)
            .into_iter()
            .map(ChatOp::Edit)
            .collect()
    }

    fn reapply(&self, step: &Step) -> Vec<ChatOp> {
        self.header
            .reapply(step)
            .into_iter()
            .map(ChatOp::Edit)
            .collect()
    }
}
