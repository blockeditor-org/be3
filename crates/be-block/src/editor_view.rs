use be_model::{Document, Edit, LatestMap, Model, ObjectId, Stamp};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::{ChildChange, Root};

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct EditorView {
    pub editor: Uuid,
    pub content: Option<Uuid>,
    pub state: LatestMap<String, ViewState>,
    pub desktop: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ViewState {
    pub bytes: Vec<u8>,
    pub refs: Vec<Uuid>,
}

impl ViewState {
    pub fn new<T: Serialize>(value: &T, refs: Vec<Uuid>) -> Self {
        Self {
            bytes: postcard::to_stdvec(value).unwrap_or_default(),
            refs,
        }
    }

    pub fn value<T: DeserializeOwned>(&self) -> Option<T> {
        postcard::from_bytes(&self.bytes).ok()
    }
}

pub fn now_milliseconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

impl EditorView {
    pub fn new(editor: Uuid, content: Option<Uuid>) -> Self {
        Self {
            editor,
            content,
            state: LatestMap::default(),
            desktop: false,
        }
    }

    pub fn document(editor: Uuid, content: Option<Uuid>) -> EditorViewContent {
        Document::new(&Self::new(editor, content))
    }

    pub fn session_document(editor: Uuid, desktop: bool) -> EditorViewContent {
        Document::new(&Self {
            desktop,
            ..Self::new(editor, None)
        })
    }

    pub fn state(&self, key: &str) -> Option<&ViewState> {
        self.state.get(key)
    }

    pub fn set_state(&self, key: &str, state: Option<&ViewState>, time: u64, origin: Uuid) -> Edit {
        let key = key.to_owned();
        let stamp = self.state.next(&key, time, origin);
        Self::STATE.put(ObjectId::ROOT, &key, state, stamp).into()
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
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6564_6974_6f72_2d76_6965_772d_626c_6b31);

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
            ChildChange::Add(_) => None,
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
