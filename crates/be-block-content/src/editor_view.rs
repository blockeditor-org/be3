//! The view block: which block an editor shows, and what the editor keeps about how it shows
//! it (guides/blocks/workspaces.md, View state).
//!
//! Each kind of view state is a type implementing `ViewStateKind`, kept under its id as its own
//! entry, so two devices writing different kinds merge without overwriting each other. A value
//! names blocks through `ViewRef`s, stored beside its bytes as the entry's `refs`, so the type can
//! repoint or blank them (`EditorView::child_edit`) without knowing the value's type.

use std::cell::RefCell;

use be_block_model::{Document, Edit, LatestMap, Model, ObjectId, Stamp};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};
use uuid::Uuid;

use crate::{ChildChange, Root};

pub const VIEW_TYPE: Uuid = Uuid::from_u128(0x6564_6974_6f72_2d76_6965_772d_626c_6b31);

/// A kind of view state, kept in the view block under `ID`.
pub trait ViewStateKind: Serialize + DeserializeOwned + PartialEq + 'static {
    const ID: Uuid;
}

/// A block named by a view state. It is stored as an index into the entry's `refs`, which the
/// type repoints when the block is replaced by a copy and blanks when it is deleted; a blanked
/// ref reads back with no block.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewRef(Uuid);

impl ViewRef {
    pub fn new(block: Uuid) -> Self {
        Self(block)
    }

    /// The block, none once it was deleted.
    pub fn block(self) -> Option<Uuid> {
        (!self.0.is_nil()).then_some(self.0)
    }
}

thread_local! {
    /// The refs of the view state being written or read, while it is.
    static REFS: RefCell<Option<Vec<Uuid>>> = const { RefCell::new(None) };
}

/// Runs `run` with `refs` as the refs of the view state being written or read, and gives them
/// back with what it returned. The refs held before are put back even if `run` panics.
fn with_refs<T>(refs: Vec<Uuid>, run: impl FnOnce() -> T) -> (T, Vec<Uuid>) {
    struct Restore(Option<Option<Vec<Uuid>>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            if let Some(outer) = self.0.take() {
                REFS.with(|held| held.replace(outer));
            }
        }
    }
    let mut restore = Restore(Some(REFS.with(|held| held.replace(Some(refs)))));
    let result = run();
    let outer = restore.0.take().unwrap_or_default();
    let refs = REFS.with(|held| held.replace(outer)).unwrap_or_default();
    (result, refs)
}

impl Serialize for ViewRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let index = REFS.with(|held| {
            let mut held = held.borrow_mut();
            let refs = held.as_mut()?;
            let index = refs.iter().position(|id| *id == self.0).unwrap_or_else(|| {
                refs.push(self.0);
                refs.len() - 1
            });
            u32::try_from(index).ok()
        });
        match index {
            Some(index) => index.serialize(serializer),
            None => Err(serde::ser::Error::custom(
                "a ViewRef is only written as part of a view state",
            )),
        }
    }
}

impl<'de> Deserialize<'de> for ViewRef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let index = u32::deserialize(deserializer)?;
        REFS.with(|held| {
            held.borrow()
                .as_ref()
                .and_then(|refs| refs.get(index as usize).copied())
        })
        .map(Self)
        .ok_or_else(|| serde::de::Error::custom("a ViewRef past the view state's refs"))
    }
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct EditorView {
    pub content: Option<Uuid>,
    pub state: LatestMap<Uuid, ViewState>,
}

/// A view state as it is stored: the value's postcard bytes, and the blocks its `ViewRef`s
/// index. Read and written only through `EditorView::view_state` and `set_view_state`; the
/// fields are public, and hidden, so a test can store a value that cannot be read.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ViewState {
    #[doc(hidden)]
    #[serde(with = "be_block_model::bytes")]
    pub bytes: Vec<u8>,
    #[doc(hidden)]
    pub refs: Vec<Uuid>,
}

impl ViewState {
    fn encode<K: ViewStateKind>(value: &K) -> Result<Self, String> {
        let (bytes, refs) = with_refs(Vec::new(), || postcard::to_stdvec(value));
        Ok(Self {
            bytes: bytes.map_err(|error| error.to_string())?,
            refs,
        })
    }

    fn decode<K: ViewStateKind>(&self) -> Result<K, String> {
        with_refs(self.refs.clone(), || postcard::from_bytes(&self.bytes))
            .0
            .map_err(|error| error.to_string())
    }
}

impl EditorView {
    pub fn new(content: Option<Uuid>) -> Self {
        Self {
            content,
            state: LatestMap::default(),
        }
    }

    pub fn document(content: Option<Uuid>) -> EditorViewContent {
        Document::new(&Self::new(content))
    }

    /// The `K` kept here: none when nothing is, an error when what is kept cannot be read.
    pub fn view_state<K: ViewStateKind>(&self) -> Option<Result<K, String>> {
        self.state.get(&K::ID).map(ViewState::decode)
    }

    /// The edit that keeps `value` as the `K` here (none removes it); none when that is what is
    /// kept already, and an error, with nothing written, when `value` cannot be encoded.
    pub fn set_view_state<K: ViewStateKind>(
        &self,
        value: Option<&K>,
        time: u64,
        origin: Uuid,
    ) -> Result<Option<Edit>, String> {
        let stored = value.map(ViewState::encode).transpose()?;
        Ok((self.state.get(&K::ID) != stored.as_ref())
            .then(|| self.put_stored(K::ID, stored.as_ref(), time, origin)))
    }

    /// The view state kept under `id` as it is stored, for tests.
    #[doc(hidden)]
    pub fn stored(&self, id: Uuid) -> Option<&ViewState> {
        self.state.get(&id)
    }

    /// The edit that keeps `state` under `id` as it is: for a test storing a value that cannot
    /// be read. Editors write with `set_view_state`.
    #[doc(hidden)]
    pub fn put_stored(&self, id: Uuid, state: Option<&ViewState>, time: u64, origin: Uuid) -> Edit {
        let stamp = self.state.next(&id, time, origin);
        Self::STATE.put(ObjectId::ROOT, &id, state, stamp).into()
    }

    fn rewrite_refs(&self, rewrite: impl Fn(Uuid) -> Uuid) -> Option<Edit> {
        let edit: Edit = self
            .state
            .iter()
            .filter_map(|(key, state)| {
                let refs: Vec<Uuid> = state.refs.iter().map(|id| rewrite(*id)).collect();
                (refs != state.refs).then(|| {
                    let stamp: Stamp = self.state.next(key, 0, Uuid::nil());
                    let rewritten = ViewState {
                        bytes: state.bytes.clone(),
                        refs,
                    };
                    Self::STATE.put(ObjectId::ROOT, key, Some(&rewritten), stamp)
                })
            })
            .collect();
        (!edit.0.is_empty()).then_some(edit)
    }
}

impl Root for EditorView {
    const CONTENT_TYPE: Uuid = VIEW_TYPE;

    fn references(&self) -> Vec<Uuid> {
        let mut references: Vec<Uuid> = self
            .content
            .into_iter()
            .chain(
                self.state
                    .values()
                    .flat_map(|state| state.refs.iter().copied()),
            )
            .filter(|id| !id.is_nil())
            .collect();
        references.sort_unstable();
        references.dedup();
        references
    }

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        match change {
            ChildChange::Add(_) | ChildChange::Reorder { .. } => None,
            ChildChange::Delete(old) => {
                self.rewrite_refs(|id| if id == old { Uuid::nil() } else { id })
            }
            ChildChange::Replace { old, new } => {
                self.rewrite_refs(|id| if id == old { new } else { id })
            }
        }
    }
}

pub type EditorViewContent = Document<EditorView>;
