use be_block_commit::MergeResult;
use be_block_content::{
    BlockContent, BlockMetadata, ChildChange, ContentError, LiveEdit, LogItem, Merge, Root, Undo,
};
use be_block_model::{BlockRef, Change, Document, Edit, LatestMap, Model, ObjectId, Step, Touched};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// What a chat keeps as content: what anyone with Edit may rewrite. Its messages, and their
/// edits, reactions and deletions, are its signed log's (`ChatEntry`), each its author's; the
/// content keeps only the blocks the chat lists as its children, which a block's children
/// always come from: the blocks made for a message not sent yet, and those sent messages
/// carry. Both are `LatestMap`s, so two devices' changes never conflict.
#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
#[model(kind = "chat")]
pub struct ChatHeader {
    /// The blocks made for a message not sent yet (decisions/chat_attachments_are_references.md),
    /// by block. Sending the message that carries one moves it to `carried`, and deleting it as
    /// a child takes it out.
    pub drafts: LatestMap<Uuid, Draft>,
    /// The blocks sent messages carry, by block, with the message's entry id: listed while the
    /// message is, and taken out with its deletion.
    pub carried: LatestMap<Uuid, Carried>,
}

/// A block made in a chat for its author's next message.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Draft {
    pub block: BlockRef,
    pub author: Uuid,
}

/// A block a sent message carries.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Carried {
    pub block: BlockRef,
    /// The id of the entry that sent the message.
    pub message: Uuid,
    pub author: Uuid,
}

impl Root for ChatHeader {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6368_6174_2d62_6c6f_636b_2d74_7970_6531);

    /// Drafts and what messages carry are stamps, which never undo: a message is taken back by
    /// deleting it.
    fn undoable(_: &Change) -> bool {
        false
    }

    /// A chat's only removable children are its drafts: a block a message carries stays
    /// listed until the message is deleted.
    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        let ChildChange::Delete(block) = change else {
            return None;
        };
        let draft = self.drafts.get(&block)?;
        let stamp = self.drafts.next(&block, 0, draft.author);
        Some(
            ChatHeader::DRAFTS
                .put(ObjectId::ROOT, &block, None, stamp)
                .into(),
        )
    }
}

/// A chat block's content: its header document, written by hand rather than as a
/// `Document<ChatHeader>` so the chat's log traits (`LogFold`, `Unread`) can be its own. It
/// forwards everything else to the header.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ChatContent {
    header: Document<ChatHeader>,
}

impl ChatContent {
    pub const fn header(&self) -> &Document<ChatHeader> {
        &self.header
    }

    fn drafts(&self) -> LatestMap<Uuid, Draft> {
        self.header.field(ObjectId::ROOT, ChatHeader::DRAFTS)
    }

    fn carried(&self) -> LatestMap<Uuid, Carried> {
        self.header.field(ObjectId::ROOT, ChatHeader::CARRIED)
    }

    /// Lists `block`, made in the chat, as a draft of `author`'s next message.
    pub fn make_draft(&self, block: Uuid, author: Uuid, time: i64) -> Edit {
        let stamp = self.drafts().next(&block, stamp_time(time), author);
        let draft = Draft {
            block: BlockRef(block),
            author,
        };
        ChatHeader::DRAFTS
            .put(ObjectId::ROOT, &block, Some(&draft), stamp)
            .into()
    }

    /// Takes `block` out of the drafts, which deletes it from the chat; none unless it is a
    /// draft (`Root::child_edit`).
    pub fn drop_draft(&self, block: Uuid) -> Option<Edit> {
        self.header.root().child_edit(ChildChange::Delete(block))
    }

    /// The blocks `author` made for a message not sent yet.
    pub fn drafts_of(&self, author: Uuid) -> Vec<Uuid> {
        self.drafts()
            .values()
            .filter(|draft| draft.author == author)
            .map(|draft| draft.block.0)
            .collect()
    }

    /// What sending the message `message` (its entry's id), which carries `attachments`, does
    /// to the content: what it carries is listed as the message's, and what of it `author`
    /// drafted leaves the drafts (someone else's draft the message links stays theirs). None
    /// for a message that carries nothing.
    pub fn send(
        &self,
        message: Uuid,
        author: Uuid,
        time: i64,
        attachments: &[BlockRef],
    ) -> Option<Edit> {
        let drafts = self.drafts();
        let carried = self.carried();
        let changes: Vec<Change> = attachments
            .iter()
            .flat_map(|attachment| {
                let block = attachment.0;
                let listed = Carried {
                    block: *attachment,
                    message,
                    author,
                };
                let stamp = carried.next(&block, stamp_time(time), author);
                let carry = ChatHeader::CARRIED.put(ObjectId::ROOT, &block, Some(&listed), stamp);
                let undraft = drafts
                    .get(&block)
                    .filter(|draft| draft.author == author)
                    .map(|draft| {
                        let stamp = drafts.next(&block, stamp_time(time), draft.author);
                        ChatHeader::DRAFTS.put(ObjectId::ROOT, &block, None, stamp)
                    });
                std::iter::once(carry).chain(undraft)
            })
            .collect();
        (!changes.is_empty()).then(|| changes.into_iter().collect())
    }

    /// What deleting the message `message` does to the content: what it carries is no longer
    /// listed. None for a message that carries nothing.
    pub fn delete(&self, message: Uuid, author: Uuid, time: i64) -> Option<Edit> {
        let carried = self.carried();
        let changes: Vec<Change> = carried
            .iter()
            .filter(|(_, listed)| listed.message == message)
            .map(|(block, _)| {
                let stamp = carried.next(block, stamp_time(time), author);
                ChatHeader::CARRIED.put(ObjectId::ROOT, block, None, stamp)
            })
            .collect();
        (!changes.is_empty()).then(|| changes.into_iter().collect())
    }
}

fn stamp_time(time: i64) -> u64 {
    u64::try_from(time).unwrap_or(0)
}

impl BlockContent for ChatContent {
    const CONTENT_TYPE: Uuid = ChatHeader::CONTENT_TYPE;

    fn schema() -> String {
        format!("chat 2\n{}", ChatHeader::kind().describe())
    }

    fn encode(&self) -> Vec<u8> {
        self.header.to_bytes()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        Document::from_bytes(bytes)
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

    fn child_operations(&self, change: ChildChange, client: u64) -> Option<Vec<Edit>> {
        self.header.child_operations(change, client)
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
        match Merge::merge3(&base.header, &ours.header, &theirs.header) {
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

/// The chat's log is folded by its editors (`ChatLog`), never by its content: a chat's
/// children are its content's, and its description its name. Each message is a searchable
/// document, which its author's edit replaces and deletion drops.
impl be_block_content::LogFold for ChatContent {
    const FOLDS: bool = false;

    fn fold(&mut self, _: &LogItem) {}

    fn search(item: &LogItem) -> Option<be_block_content::LogSearch> {
        let (place, text) = match crate::ChatEntry::decode(item.payload.as_deref()?)? {
            crate::ChatEntry::Message { body, .. } => (item.seq, Some(body)),
            crate::ChatEntry::Edit { message, body, .. } => (message, Some(body)),
            crate::ChatEntry::Delete { message, .. } => (message, None),
            crate::ChatEntry::React { .. } => return None,
        };
        Some(be_block_content::LogSearch { place, text })
    }
}
