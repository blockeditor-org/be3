use std::collections::{BTreeMap, BTreeSet, VecDeque};

use be_block_content::{LiveEdit, Settling, Touched};
use be_sdk_type_api::ClientId;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

/// What a `FromEditor::Gesture` does to the gesture it names.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Gesture {
    /// Opens an undo group: its edits are written as they happen and undo as one step.
    Begin,
    /// Opens a tentative gesture: its edits are shown at once but held from peers and the
    /// journal until it commits (decisions/tentative_edits_wait_for_release.md).
    BeginTentative,
    Commit,
    Cancel,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FromEditor {
    /// An edit, made in the editor's open gesture `gesture`, or outside any.
    Operate {
        gesture: Option<u64>,
        #[serde(with = "serde_bytes")]
        operation: Vec<u8>,
    },
    /// Begins or ends the editor's gesture `id`. An editor picks its own ids and may hold
    /// several gestures open at once (two fingers, two controllers), each ending on its own;
    /// beginning one under the name of one still open commits that first.
    Gesture {
        id: u64,
        gesture: Gesture,
    },
    /// Ends the editor's current undo step without opening a gesture, so its next edit starts
    /// one of its own however soon it comes: a text editor's paste, or typing after a caret
    /// jump.
    Break,
    Replace(#[serde(with = "serde_bytes")] Vec<u8>),
    ShowPresence {
        kind: Uuid,
        #[serde(with = "serde_bytes")]
        value: Option<Vec<u8>>,
    },
    SendToPeers {
        to: Option<ClientId>,
        channel: Uuid,
        #[serde(with = "serde_bytes")]
        payload: Vec<u8>,
    },
    Resend,
    /// Says the editor was shown a snapshot made after `reloads` reloads (`Snapshot::reloads`):
    /// what it sends after this was made on what that snapshot shows, and what it sent before,
    /// on what it showed until then, which the replica carries over the reload.
    Shown {
        reloads: u64,
    },
    /// Asks the replica to follow `ToEditor::Lacking`: what the server lacks of the edits made
    /// on this device, whichever editor made them.
    FollowLacking,
    /// Lets an edit the server refused (`Lacked::failed`) go: the person sent it again, or
    /// gave it up.
    DismissLacking(u64),
    /// Asks for `length` bytes of the block's stored form from `offset`, as the replica holds
    /// it: what a type that loads only its first part left in the store comes this way. The
    /// replica answers `ToEditor::Range` with the same `token`.
    Read {
        token: u64,
        offset: u64,
        length: u64,
    },
    /// Asks the replica to put `bytes` as a part of the block (`host.put-part`), so an
    /// operation can name it by its hash instead of carrying it. The replica answers
    /// `ToEditor::PartPut` with the same `token` once the part is uploaded.
    PutPart {
        token: u64,
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
    },
    /// Appends `bytes` to the block's signed log (a type with a log): the host signs them as
    /// this app's, for the person, and the replica answers `ToEditor::Appended` with the same
    /// `token`. Every editor hears the entry as `ToEditor::Logged` once it is in the log.
    Append {
        token: u64,
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
        along: be_block_content::Along,
    },
    /// Asks for at most `limit` entries of the block's signed log from `from`; the replica
    /// answers `ToEditor::LogRead` with the same `token`.
    ReadLog {
        token: u64,
        from: u64,
        limit: u32,
    },
    /// Lets go of the entry `id` the server refused (`LogPage::refused`): sent again as a new
    /// one, or given up.
    LetGoRefused {
        id: Uuid,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Snapshot {
    pub content_type: Uuid,
    /// `encode()`: for a type that loads only its first part (`be_block_content::Loads::FirstPart`),
    /// only that part's bytes, the rest read with `FromEditor::Read`.
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
    /// The whole stored form's length.
    pub length: u64,
    #[serde(with = "serde_bytes")]
    pub session: Vec<u8>,
    #[serde(
        serialize_with = "be_wasm_ops::wire::serialize::<be_wasm_ops::wire::List<be_wasm_ops::wire::Bytes>, _, _>",
        deserialize_with = "be_wasm_ops::wire::deserialize::<be_wasm_ops::wire::List<be_wasm_ops::wire::Bytes>, _, _>"
    )]
    pub unsettled: Vec<Vec<u8>>,
    pub applied: u64,
    /// How many reloads the replica took before this: an editor says once it was shown a new
    /// count (`FromEditor::Shown`).
    pub reloads: u64,
}

/// An edit made on this device that the server lacks, as an editor reads it
/// (`EditorMirror::lacking`): `operation` is none for one too large to keep.
#[derive(Clone, Debug, PartialEq)]
pub struct LackingEdit<Op> {
    pub id: u64,
    pub operation: Option<Op>,
    pub failed: bool,
}

/// An edit made on this device that the server lacks: sequenced but not yet appended to the
/// server's journal, appended offline, in a commit kept in the outbox, or, on a follower, not
/// yet accepted by the owner. `failed` once the server refused the commit holding it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Lacked {
    pub id: u64,
    #[serde(with = "serde_bytes")]
    pub operation: Vec<u8>,
    pub failed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EditorOperation {
    #[serde(with = "serde_bytes")]
    pub operation: Vec<u8>,
    pub mine: bool,
    pub unsettled: u64,
    #[serde(
        serialize_with = "be_wasm_ops::wire::serialize::<be_wasm_ops::wire::Optional<be_wasm_ops::wire::List<be_wasm_ops::wire::Bytes>>, _, _>",
        deserialize_with = "be_wasm_ops::wire::deserialize::<be_wasm_ops::wire::Optional<be_wasm_ops::wire::List<be_wasm_ops::wire::Bytes>>, _, _>"
    )]
    pub beneath: Option<Vec<Vec<u8>>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PeerPresence {
    pub client: ClientId,
    pub kind: Uuid,
    #[serde(with = "serde_bytes")]
    pub value: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PeerMessage {
    pub from: ClientId,
    pub channel: Uuid,
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ToEditor {
    Snapshot(Snapshot),
    Operations(Vec<EditorOperation>),
    Presence(Vec<PeerPresence>),
    PeerMessages(Vec<PeerMessage>),
    Range {
        token: u64,
        #[serde(with = "range_result")]
        result: Result<Vec<u8>, String>,
    },
    PartPut {
        token: u64,
        result: Result<be_block_content::PartRef, String>,
    },
    /// Where an entry `FromEditor::Append` asked for went: its id, and its place and arrival,
    /// or none while it waits on this device for the server.
    Appended {
        token: u64,
        result: Result<be_block_content::LogAppended, be_block_content::LogNotAppended>,
    },
    /// Every entry this device appended that the server refused and keeps until let go, each
    /// id with its bytes and why, as a drain refused one.
    Refused(Vec<be_block_content::RefusedEntry>),
    /// Entries of the block's signed log, in order: what the replica heard appended, every
    /// editor's own too.
    Logged(Vec<be_block_content::LogItem>),
    /// The answer to `FromEditor::ReadLog`: the log's length, the entries asked for, and those
    /// this device appended that the server has not taken yet.
    LogRead {
        token: u64,
        result: Result<be_block_content::LogPage, String>,
    },
    /// Edits made on this device that the server lacks, for an editor that asked
    /// (`FromEditor::FollowLacking`): with `reset`, every one; otherwise those added or changed
    /// since, and the ids of those the server now holds.
    Lacking {
        reset: bool,
        lacking: Vec<Lacked>,
        held: Vec<u64>,
    },
    /// The replica could not open the block's content (it does not decode in this build, its
    /// type module failed, its parts cannot be read), and says why: until a snapshot arrives,
    /// the editor reads it as failed rather than loading.
    Failed(String),
    /// The replica ended the editor's gesture `id` itself: an undo cancelled its tentative
    /// layer, or the content its edits moved was replaced or did not carry over a restart. With
    /// `cancelled`, none of its edits are kept: the editor stops previewing it, sends no more
    /// edits in it, and answers with a `Gesture::Cancel` for it, which is what ends it for the
    /// host and the replica alike (only a commit sent before this arrived can cross it).
    /// Without, an undo or redo committed the undo group first: it is no longer open, the
    /// editor answers with a `Gesture::Commit`, which ends it for the host, and its later
    /// edits in it are each recorded outside it.
    GestureEnded {
        id: u64,
        cancelled: bool,
    },
}

mod range_result {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Deserialize, Serialize)]
    enum Read {
        Ok(#[serde(with = "serde_bytes")] Vec<u8>),
        Err(String),
    }

    pub fn serialize<S: Serializer>(
        result: &Result<Vec<u8>, String>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match result {
            Ok(bytes) => Read::Ok(bytes.clone()),
            Err(error) => Read::Err(error.clone()),
        }
        .serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Result<Vec<u8>, String>, D::Error> {
        Ok(match Read::deserialize(deserializer)? {
            Read::Ok(bytes) => Ok(bytes),
            Read::Err(error) => Err(error),
        })
    }
}

/// An entry not appended to a block's signed log, as an editor holds it until dismissed.
#[derive(Clone, Debug)]
struct NotSent {
    /// The id of one the server refused, which the device keeps until let go; none for one the
    /// device never made.
    id: Option<Uuid>,
    bytes: Vec<u8>,
    why: String,
}

/// How many entries of a block's signed log an editor holds at most, unless it paged back past
/// them: the latest it heard or read. Older ones it reads again (`FromEditor::ReadLog`), and
/// what it pages back to it keeps.
pub const LOG_HELD: usize = 1024;

/// What `take_logged` did with an entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Taken {
    /// It was held as it is, or as well as it is now told.
    Held,
    /// It was not held at this place before.
    New,
    /// It was held at this place, and is now told better: who signed it, once that could be
    /// told (`Unknown` to `Verified`), or its payload gone (`Redacted`).
    Changed,
    /// Its id is held at an earlier place: its place is skipped.
    Skipped,
    /// Its id was held at a later place, which is now skipped.
    Moved,
}

/// The entries of a block's signed log held by place: each id once, at its first place, so
/// one a server serves again under its id later in the log is never shown twice, and the places
/// such repeats take, which a walk of the log in order steps over.
#[derive(Clone, Debug, Default)]
pub(crate) struct HeldLog {
    pub(crate) items: BTreeMap<u64, be_block_content::LogItem>,
    ids: BTreeMap<Uuid, u64>,
    /// Places a repeat of an id held earlier takes.
    pub(crate) skipped: std::collections::BTreeSet<u64>,
    /// The earliest place the editor paged back to (a read from before what it held): what it
    /// holds from there on is kept, however many, so a page read back is never trimmed away.
    floor: Option<u64>,
}

impl HeldLog {
    /// Keeps `item`, unless its id is held at an earlier place: a log read in pieces keeps the
    /// first place of each id, and one heard at a later place first is moved back. One held at
    /// its place is replaced when told better, never by an `Unknown` verdict.
    pub(crate) fn take(&mut self, item: be_block_content::LogItem) -> Taken {
        match self.ids.get(&item.id).copied() {
            Some(seq) if seq < item.seq => {
                self.skipped.insert(item.seq);
                Taken::Skipped
            }
            Some(seq) if seq == item.seq => {
                let unchanged = self.items.get(&seq).is_none_or(|held| {
                    *held == item || item.signer == be_block_content::Signer::Unknown
                });
                if unchanged {
                    return Taken::Held;
                }
                self.items.insert(seq, item);
                Taken::Changed
            }
            held => {
                self.skipped.remove(&item.seq);
                self.ids.insert(item.id, item.seq);
                self.items.insert(item.seq, item);
                match held {
                    Some(later) => {
                        self.items.remove(&later);
                        self.skipped.insert(later);
                        Taken::Moved
                    }
                    None => Taken::New,
                }
            }
        }
    }

    /// Whether an entry `id` names is held.
    pub(crate) fn holds_id(&self, id: Uuid) -> bool {
        self.ids.contains_key(&id)
    }

    /// Whether the place `seq` is held: an entry, or a repeat skipped.
    pub(crate) fn holds(&self, seq: u64) -> bool {
        self.items.contains_key(&seq) || self.skipped.contains(&seq)
    }

    /// Notes a read from `from`: one from before every entry held pages back, and what it reads
    /// is kept.
    fn reading(&mut self, from: u64) {
        let first = self.items.keys().next().copied();
        if first.is_some_and(|first| from < first) {
            self.floor = Some(self.floor.map_or(from, |floor| floor.min(from)));
        }
    }

    /// Keeps only the latest `count` entries, and every one from where the editor paged back to.
    fn keep_latest(&mut self, count: usize) {
        while self.items.len() > count {
            let Some((&seq, _)) = self.items.first_key_value() else {
                break;
            };
            if self.floor.is_some_and(|floor| seq >= floor) {
                break;
            }
            if let Some((seq, oldest)) = self.items.pop_first() {
                self.ids.remove(&oldest.id);
                self.skipped = self.skipped.split_off(&(seq + 1));
            }
        }
    }
}

fn encode<T: Serialize>(message: &T) -> Vec<u8> {
    postcard::to_stdvec(message).expect("editor messages always encode")
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Option<T> {
    postcard::from_bytes(bytes).ok()
}

impl FromEditor {
    pub fn encode(&self) -> Vec<u8> {
        encode(self)
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        decode(bytes)
    }

    pub fn frame(&self) -> Vec<u8> {
        be_sdk_type_api::encode_frame(self)
    }

    pub fn from_frame(frame: &[u8]) -> Option<Self> {
        be_sdk_type_api::decode_frame(frame).ok()
    }
}

impl ToEditor {
    pub fn encode(&self) -> Vec<u8> {
        encode(self)
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        decode(bytes)
    }

    pub fn frame(&self) -> Vec<u8> {
        be_sdk_type_api::encode_frame(self)
    }

    pub fn from_frame(frame: &[u8]) -> Option<Self> {
        be_sdk_type_api::decode_frame(frame).ok()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Projected<Op> {
    Applied(Op),
    Rebuilt,
}

pub struct EditorMirror<C: LiveEdit> {
    confirmed: Option<Settling<C>>,
    visible: C,
    pending: VecDeque<C::Op>,
    acknowledged: u64,
    resyncing: bool,
    touched: Vec<Touched>,
    presence: Vec<PeerPresence>,
    peer_messages: Vec<PeerMessage>,
    revision: u64,
    projected: Option<Vec<Projected<C::Op>>>,
    length: u64,
    snapshots: u64,
    /// The reloads the last snapshot was made after (`Snapshot::reloads`).
    reloads: u64,
    reads: u64,
    ranges: Vec<(u64, Result<Vec<u8>, String>)>,
    puts: Vec<(u64, Result<be_block_content::PartRef, String>)>,
    /// What `ToEditor::Appended` answered, by token, until taken.
    appended: Vec<(u64, Result<be_block_content::LogAppended, String>)>,
    /// The entries of the block's signed log this editor heard or read, by place, each id once
    /// at its first place: the latest `LOG_HELD`, and all from where it paged back to.
    log: HeldLog,
    /// The log's length as last read.
    log_size: u64,
    /// What each `append` not answered yet appends, by token.
    appending: BTreeMap<u64, Vec<u8>>,
    /// The entries this device appended that the server has not taken yet, by id: those this
    /// editor appended, and those a read told of (`LogPage::kept`), which another editor, or
    /// this one before the editor opened again, appended.
    kept: Vec<be_block_content::KeptEntry>,
    /// The entries not appended, by the token of the `append` (or one this editor gave an entry
    /// a read or the replica told of): the bytes, why, and the id of one the server refused,
    /// which the device keeps until let go, until dismissed.
    refused: BTreeMap<u64, NotSent>,
    lacking: BTreeMap<u64, (Vec<u8>, bool)>,
    lacking_revision: u64,
    failed: Option<String>,
    /// The gestures this editor holds open, by id.
    gesturing: BTreeSet<u64>,
    /// Those the replica ended (`ToEditor::GestureEnded`), which this cancelled for the
    /// editor: what the editor sends in them is dropped until it ends them itself.
    voided: BTreeSet<u64>,
    /// The undo groups an undo or redo committed (`ToEditor::GestureEnded` without
    /// `cancelled`), which this answered with their commit: no longer open, and what the
    /// editor sends in them is written outside any gesture until it ends them itself, which
    /// sends nothing.
    committed: BTreeSet<u64>,
    /// Those the replica ended since `take_ended`.
    ended: Vec<u64>,
}

impl<C: LiveEdit + Clone + Default> Default for EditorMirror<C> {
    fn default() -> Self {
        Self {
            confirmed: None,
            visible: C::default(),
            pending: VecDeque::new(),
            acknowledged: 0,
            resyncing: false,
            touched: Vec::new(),
            presence: Vec::new(),
            peer_messages: Vec::new(),
            revision: 0,
            projected: None,
            length: 0,
            snapshots: 0,
            reloads: 0,
            reads: 0,
            ranges: Vec::new(),
            puts: Vec::new(),
            appended: Vec::new(),
            log: HeldLog::default(),
            appending: BTreeMap::new(),
            kept: Vec::new(),
            refused: BTreeMap::new(),
            log_size: 0,
            lacking: BTreeMap::new(),
            lacking_revision: 0,
            failed: None,
            gesturing: BTreeSet::new(),
            voided: BTreeSet::new(),
            committed: BTreeSet::new(),
            ended: Vec::new(),
        }
    }
}

impl<C: LiveEdit + Clone + Default> EditorMirror<C> {
    pub fn content(&self) -> Option<&C> {
        self.confirmed.as_ref().map(|_| &self.visible)
    }

    pub fn visible(&self) -> &C {
        &self.visible
    }

    pub fn is_loaded(&self) -> bool {
        self.confirmed.is_some()
    }

    /// Why the content could not be opened, until a snapshot that opens arrives: the replica
    /// said so (`ToEditor::Failed`), or the snapshot it sent does not decode in this build.
    pub fn failure(&self) -> Option<&str> {
        self.failed.as_deref()
    }

    /// Notes a failure to open, which readers hear of as a change of revision.
    fn fail(&mut self, reason: String) {
        if self.failed.as_deref() != Some(reason.as_str()) {
            self.failed = Some(reason);
            self.revision += 1;
        }
    }

    /// How many of its operations this editor was told landed, which a snapshot's `applied`
    /// is counted against.
    pub fn acknowledged(&self) -> u64 {
        self.acknowledged
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn record(&mut self) {
        self.projected.get_or_insert_with(Vec::new);
    }

    pub fn stop_recording(&mut self) {
        self.projected = None;
    }

    pub fn take_projected(&mut self) -> Vec<Projected<C::Op>> {
        self.projected
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }

    fn changed(&mut self, projected: Projected<C::Op>) {
        self.revision += 1;
        if let Some(log) = &mut self.projected {
            log.push(projected);
        }
    }

    /// The whole stored form's length as of the last snapshot, of which a type that loads
    /// only its first part holds the start.
    pub fn stored_length(&self) -> u64 {
        self.length
    }

    /// How many snapshots the mirror took: what it holds of a type that loads only its first
    /// part changes only with one, so a reader of the rest reads again when this moves.
    pub fn snapshots(&self) -> u64 {
        self.snapshots
    }

    /// Asks the replica for a range of the stored form; its answer comes back from `receive`
    /// and is taken with `take_ranges` under the token this returns.
    pub fn read_range(&mut self, offset: u64, length: u64) -> (u64, FromEditor) {
        self.reads += 1;
        let token = self.reads;
        (
            token,
            FromEditor::Read {
                token,
                offset,
                length,
            },
        )
    }

    pub fn take_ranges(&mut self) -> Vec<(u64, Result<Vec<u8>, String>)> {
        std::mem::take(&mut self.ranges)
    }

    /// Asks the replica to put `bytes` as a part of the block; the part's ref comes back from
    /// `receive` and is taken with `take_parts_put` under the token this returns, to put in an
    /// operation's payload.
    pub fn put_part(&mut self, bytes: Vec<u8>) -> (u64, FromEditor) {
        self.reads += 1;
        let token = self.reads;
        (token, FromEditor::PutPart { token, bytes })
    }

    pub fn take_parts_put(&mut self) -> Vec<(u64, Result<be_block_content::PartRef, String>)> {
        std::mem::take(&mut self.puts)
    }

    /// Asks the replica to append `bytes` to the block's signed log; where it went comes back
    /// from `receive` and is taken with `take_appended` under the token this returns, and the
    /// entry itself as it reaches the log (`log`).
    pub fn append(&mut self, bytes: Vec<u8>, along: be_block_content::Along) -> (u64, FromEditor) {
        self.reads += 1;
        let token = self.reads;
        self.appending.insert(token, bytes.clone());
        (
            token,
            FromEditor::Append {
                token,
                bytes,
                along,
            },
        )
    }

    pub fn take_appended(&mut self) -> Vec<(u64, Result<be_block_content::LogAppended, String>)> {
        std::mem::take(&mut self.appended)
    }

    /// Asks the replica for at most `limit` entries of the block's signed log from `from`,
    /// which join `log` as they come back.
    pub fn read_log(&mut self, from: u64, limit: u32) -> (u64, FromEditor) {
        self.log.reading(from);
        self.reads += 1;
        let token = self.reads;
        (token, FromEditor::ReadLog { token, from, limit })
    }

    /// The entries of the block's signed log this editor holds, in the log's order, each id
    /// once at its first place: the latest `LOG_HELD` it heard or read, and every one from where
    /// it paged back to (a read from before all it held), however many.
    pub fn log(&self) -> impl Iterator<Item = &be_block_content::LogItem> {
        self.log.items.values()
    }

    /// The log's length as this editor last heard it.
    pub fn log_size(&self) -> u64 {
        self.log_size
    }

    /// The entries this device appended that the server has not taken yet, in the order they
    /// were appended as far as this editor knows: each id and the bytes the app appended.
    pub fn kept(&self) -> impl Iterator<Item = (Uuid, &[u8])> {
        self.kept
            .iter()
            .map(|kept| (kept.id, kept.payload.as_slice()))
    }

    /// The entries not appended, by the token of the `append`, in the order they were refused:
    /// the bytes and why. Those the server refused the device keeps until let go, so they are
    /// here again after the editor or the host opens again, under tokens this editor gave them.
    pub fn refused(&self) -> impl Iterator<Item = (u64, &[u8], &str)> {
        self.refused
            .iter()
            .map(|(token, refused)| (*token, refused.bytes.as_slice(), refused.why.as_str()))
    }

    /// Lets go of a refused entry, sent again as a new one or given up: what to send the
    /// replica so the device lets go of it too, for one the server refused.
    pub fn dismiss_refused(&mut self, token: u64) -> Option<FromEditor> {
        let refused = self.refused.remove(&token)?;
        self.revision += 1;
        refused.id.map(|id| FromEditor::LetGoRefused { id })
    }

    /// Takes the entries the device keeps as refused, all of them: one let go elsewhere goes,
    /// one not held yet joins under a token of this editor's, and none is sending any more.
    fn take_refused(&mut self, entries: Vec<be_block_content::RefusedEntry>) {
        let ids: BTreeSet<Uuid> = entries.iter().map(|entry| entry.id).collect();
        self.refused
            .retain(|_, refused| refused.id.is_none_or(|id| ids.contains(&id)));
        self.kept.retain(|kept| !ids.contains(&kept.id));
        for be_block_content::RefusedEntry { id, payload, why } in entries {
            if self.refused.values().any(|refused| refused.id == Some(id)) {
                continue;
            }
            self.reads += 1;
            self.refused.insert(
                self.reads,
                NotSent {
                    id: Some(id),
                    bytes: payload,
                    why,
                },
            );
        }
        self.revision += 1;
    }

    fn take_log(&mut self, items: Vec<be_block_content::LogItem>) {
        for item in items {
            self.log_size = self.log_size.max(item.seq + 1);
            self.kept.retain(|kept| kept.id != item.id);
            self.log.take(item);
        }
        // An editor holds the latest entries it heard or read, and reads older ones again.
        self.log.keep_latest(LOG_HELD);
        self.revision += 1;
    }

    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// The edits made on this device that the server lacks (`ToEditor::Lacking`), each with its
    /// id and whether the server refused it, once `FromEditor::FollowLacking` was sent. An edit
    /// too large to keep comes without its operation.
    pub fn lacking(&self) -> Vec<LackingEdit<C::Op>> {
        self.lacking
            .iter()
            .map(|(id, (operation, failed))| LackingEdit {
                id: *id,
                operation: C::decode_operation(operation).ok(),
                failed: *failed,
            })
            .collect()
    }

    /// Moves whenever `lacking` changes.
    pub fn lacking_revision(&self) -> u64 {
        self.lacking_revision
    }

    pub fn presence(&self) -> &[PeerPresence] {
        &self.presence
    }

    pub fn take_peer_messages(&mut self) -> Vec<PeerMessage> {
        std::mem::take(&mut self.peer_messages)
    }

    pub fn take_touched(&mut self) -> Vec<Touched> {
        std::mem::take(&mut self.touched)
    }

    /// Applies `operation`, made in the editor's gesture `gesture` or outside any, and returns
    /// the message that sends it: none before the content arrives, or in a gesture that is not
    /// open (never begun here, ended by the editor, or ended by the replica), whose edits are
    /// dropped rather than written as they happen.
    pub fn operate(&mut self, gesture: Option<u64>, operation: C::Op) -> Option<FromEditor> {
        self.confirmed.as_ref()?;
        let gesture = gesture.filter(|gesture| !self.committed.contains(gesture));
        if gesture.is_some_and(|gesture| !self.gesturing.contains(&gesture)) {
            return None;
        }
        self.visible.apply_touching(&operation, &mut self.touched);
        self.revision += 1;
        let message = FromEditor::Operate {
            gesture,
            operation: C::encode_operation(&operation),
        };
        self.pending.push_back(operation);
        Some(message)
    }

    /// Begins or ends the editor's gesture `id` (`FromEditor::Gesture`), returning the message
    /// that does: none to end one the replica ended, which this already cancelled.
    pub fn gesture(&mut self, id: u64, gesture: Gesture) -> Option<FromEditor> {
        match gesture {
            Gesture::Begin | Gesture::BeginTentative => {
                self.voided.remove(&id);
                self.committed.remove(&id);
                self.gesturing.insert(id);
            }
            Gesture::Commit | Gesture::Cancel => {
                self.gesturing.remove(&id);
                if self.voided.remove(&id) | self.committed.remove(&id) {
                    return None;
                }
            }
        }
        Some(FromEditor::Gesture { id, gesture })
    }

    /// Whether the editor's gesture `id` is open: begun, and ended neither by the editor nor
    /// by the replica.
    pub fn gesture_open(&self, id: u64) -> bool {
        self.gesturing.contains(&id)
    }

    /// The editor's gestures the replica ended since this was last asked, in the order it did:
    /// each is cancelled already, and its editor stops it (a drag lets go of what it carried).
    pub fn take_ended(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.ended)
    }

    /// Takes what the replica sent. Its answer, when there is one, must reach the replica
    /// before anything the editor sends after: a `Resend` asks for a snapshot, and a
    /// `FromEditor::Shown` says which of its edits were made on what it showed before a
    /// reload.
    pub fn receive(&mut self, message: ToEditor) -> Option<FromEditor> {
        match message {
            ToEditor::Snapshot(snapshot) => {
                let reloads = snapshot.reloads;
                self.adopt(snapshot);
                (std::mem::replace(&mut self.reloads, reloads) != reloads)
                    .then_some(FromEditor::Shown { reloads })
            }
            ToEditor::Operations(operations) => {
                if self.confirmed.is_none() || self.resyncing {
                    return None;
                }
                for operation in operations {
                    if !self.take(operation) {
                        self.resyncing = true;
                        return Some(FromEditor::Resend);
                    }
                }
                None
            }
            ToEditor::Presence(presence) => {
                self.presence = presence;
                None
            }
            ToEditor::PeerMessages(messages) => {
                self.peer_messages.extend(messages);
                None
            }
            ToEditor::Range { token, result } => {
                self.ranges.push((token, result));
                None
            }
            ToEditor::PartPut { token, result } => {
                self.puts.push((token, result));
                None
            }
            ToEditor::Appended { token, result } => {
                if let Some(bytes) = self.appending.remove(&token) {
                    match &result {
                        // It waits on the device, unless the log already holds it.
                        Ok(appended) if appended.place.is_none() => {
                            let known = self.log.holds_id(appended.id)
                                || self.kept.iter().any(|kept| kept.id == appended.id);
                            if !known {
                                self.kept.push(be_block_content::KeptEntry {
                                    id: appended.id,
                                    payload: bytes,
                                });
                            }
                        }
                        Ok(_) => {}
                        // One the replica told of as refused already is held once.
                        Err(not)
                            if not.id.is_some()
                                && self.refused.values().any(|refused| refused.id == not.id) => {}
                        Err(not) => {
                            self.refused.insert(
                                token,
                                NotSent {
                                    id: not.id,
                                    bytes,
                                    why: not.why.clone(),
                                },
                            );
                        }
                    }
                }
                self.appended.push((token, result.map_err(|not| not.why)));
                self.revision += 1;
                None
            }
            ToEditor::Refused(entries) => {
                self.take_refused(entries);
                None
            }
            ToEditor::Logged(items) => {
                self.take_log(items);
                None
            }
            ToEditor::LogRead { token: _, result } => {
                if let Ok(page) = result {
                    self.log_size = self.log_size.max(page.size);
                    // The device's outbox as it was read: what is no longer in it was taken.
                    self.kept = page
                        .kept
                        .into_iter()
                        .filter(|kept| !self.log.holds_id(kept.id))
                        .collect();
                    self.take_refused(page.refused);
                    self.take_log(page.items);
                }
                None
            }
            ToEditor::Lacking {
                reset,
                lacking,
                held,
            } => {
                if reset {
                    self.lacking.clear();
                }
                for lacked in lacking {
                    self.lacking
                        .insert(lacked.id, (lacked.operation, lacked.failed));
                }
                for id in held {
                    self.lacking.remove(&id);
                }
                self.lacking_revision += 1;
                None
            }
            ToEditor::Failed(reason) => {
                self.fail(reason);
                None
            }
            // The replica dropped the gesture's edits, and drops what comes before it hears the
            // gesture end: the cancel answering is what ends it for the host and the replica,
            // so only a commit already on its way crosses it.
            // An undo or redo committed the undo group: it is closed for `gesture_open`, and
            // the commit answering ends it for the host too, so nothing waits on the editor
            // ending it.
            ToEditor::GestureEnded {
                id,
                cancelled: false,
            } => {
                if !self.gesturing.remove(&id) {
                    return None;
                }
                self.committed.insert(id);
                Some(FromEditor::Gesture {
                    id,
                    gesture: Gesture::Commit,
                })
            }
            ToEditor::GestureEnded { id, cancelled } => {
                if !cancelled || !self.gesturing.remove(&id) {
                    return None;
                }
                self.voided.insert(id);
                self.ended.push(id);
                Some(FromEditor::Gesture {
                    id,
                    gesture: Gesture::Cancel,
                })
            }
        }
    }

    fn adopt(&mut self, snapshot: Snapshot) {
        if snapshot.content_type != C::CONTENT_TYPE {
            return;
        }
        let mut settled = match C::decode(&snapshot.bytes) {
            Ok(settled) => settled,
            Err(error) => return self.fail(format!("The content could not be read: {error}")),
        };
        if let Err(error) = settled.adopt_session_state(&snapshot.session) {
            return self.fail(format!("The content's session could not be read: {error}"));
        }
        let Ok(unsettled) = snapshot
            .unsettled
            .iter()
            .map(|operation| C::decode_operation(operation))
            .collect::<Result<Vec<_>, _>>()
        else {
            return self.fail("The content's unsettled edits could not be read".to_owned());
        };
        self.failed = None;
        let taken = snapshot.applied.saturating_sub(self.acknowledged);
        self.acknowledged = snapshot.applied;
        for _ in 0..taken.min(self.pending.len() as u64) {
            self.pending.pop_front();
        }
        self.confirmed = Some(Settling::new(settled, unsettled));
        self.length = snapshot.length;
        self.snapshots += 1;
        self.resyncing = false;
        self.rebuild();
    }

    fn take(&mut self, landed: EditorOperation) -> bool {
        let Ok(operation) = C::decode_operation(&landed.operation) else {
            return false;
        };
        let Some(confirmed) = self.confirmed.as_mut() else {
            return false;
        };
        if let Some(local) = landed.beneath {
            let Ok(local) = local
                .iter()
                .map(|operation| C::decode_operation(operation))
                .collect::<Result<Vec<_>, _>>()
            else {
                return false;
            };
            let landed = confirmed.beneath(
                &mut self.visible,
                &operation,
                landed.unsettled,
                local,
                self.pending.iter().cloned(),
                &mut self.touched,
            );
            if landed {
                self.changed(Projected::Rebuilt);
            }
            return landed;
        }
        if landed.mine {
            // The replica may have taken it otherwise than it went (carried over a reload this
            // editor had not been shown, `FromEditor::Shown`): what it shows is then rebuilt.
            let taken_as_sent = self
                .pending
                .pop_front()
                .is_some_and(|sent| C::encode_operation(&sent) == landed.operation);
            self.acknowledged += 1;
            confirmed.push(operation, landed.unsettled);
            if !taken_as_sent {
                self.rebuild();
            }
            return true;
        }
        if self.pending.is_empty() {
            self.visible.apply_touching(&operation, &mut self.touched);
            confirmed.push(operation.clone(), landed.unsettled);
            self.changed(Projected::Applied(operation));
            return true;
        }
        if landed.unsettled == 0 {
            confirmed.settle_to(0);
            confirmed.beneath(
                &mut self.visible,
                &operation,
                0,
                Vec::new(),
                self.pending.iter().cloned(),
                &mut self.touched,
            );
            self.changed(Projected::Rebuilt);
            return true;
        }
        confirmed.push(operation, landed.unsettled);
        self.rebuild();
        true
    }

    fn rebuild(&mut self) {
        let Some(confirmed) = &self.confirmed else {
            return;
        };
        let mut visible = confirmed.visible();
        for operation in &self.pending {
            visible.apply(operation);
        }
        self.visible = visible;
        self.touched.push(Touched::Everything);
        self.changed(Projected::Rebuilt);
    }
}
