use be_model::{Document, Edit, LatestMap, Model, ObjectId, Stamp};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::{ChildChange, Root, VIEW_TYPE};

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct EditorView {
    pub content: Option<Uuid>,
    pub state: LatestMap<String, ViewState>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ViewState {
    #[serde(with = "be_model::bytes")]
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
