use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use block::BlockReferenceList;
use block_client::BlockClient;
use block_client::block_ref::BlockRef;
use block_client::blocks::workspace_index::{WorkspaceIndex, WorkspaceIndexOperation};
use block_client::references::{ReferenceClassificationQueue, ReferenceResolutionCache};
use block_editor_plugin::beui::reactive::{Memo, ReadSignal, create_memo, create_signal, untrack};
use block_editor_plugin::block_ui::{BlockLabel, BlockTypes};
use block_editor_plugin::{BlockProjection, Editor};
use uuid::Uuid;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum FolderSort {
    #[default]
    Intrinsic,
    Name,
    Type,
}

impl FolderSort {
    pub(crate) const ALL: [Self; 3] = [Self::Intrinsic, Self::Name, Self::Type];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Intrinsic => "Intrinsic",
            Self::Name => "Name",
            Self::Type => "Type",
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct Entry {
    pub(crate) reference: BlockRef,
    pub(crate) id: Option<Uuid>,
    pub(crate) block_type: Option<Uuid>,
    pub(crate) name: String,
    pub(crate) type_name: String,
    pub(crate) glyph: String,
    pub(crate) automatic: bool,
    pub(crate) loaded: bool,
}

pub(crate) struct Folder {
    entries: Memo<Vec<Entry>>,
    adds: RefCell<ReferenceClassificationQueue<()>>,
    client: Arc<BlockClient>,
}

impl Folder {
    pub(crate) fn watch(
        editor: &Editor,
        index: &BlockProjection<WorkspaceIndex>,
        sort: ReadSignal<FolderSort>,
        descending: ReadSignal<bool>,
    ) -> Self {
        let references = editor
            .client()
            .watch_references(BlockReferenceList::References(editor.block_id()));
        let referenced = index.project(|index| index.entries().to_vec());
        let (rows, set_rows) = create_signal(Vec::<Entry>::new());
        let cache = RefCell::new(ReferenceResolutionCache::default());
        let client = Arc::clone(editor.client());
        let block_id = editor.block_id();
        let host = editor.host().clone();
        editor.each_frame(move || {
            let mut cache = cache.borrow_mut();
            cache.poll();
            let metadata: HashMap<_, _> = references
                .read()
                .into_iter()
                .map(|reference| (reference.id, reference))
                .collect();
            let types = host.block_types();
            let mut entries: Vec<_> = referenced
                .get_untracked()
                .into_iter()
                .map(|reference| {
                    let id = cache.resolve(&client, block_id, reference);
                    let found = id.and_then(|id| metadata.get(&id));
                    let label = found.map(|found| BlockLabel::for_reference(types.as_ref(), found));
                    Entry {
                        reference,
                        id,
                        block_type: found.map(|found| found.block_type),
                        name: label
                            .as_ref()
                            .map_or_else(|| "Loading…".to_owned(), |label| label.name.clone()),
                        type_name: found.map_or_else(
                            || "Loading…".to_owned(),
                            |found| type_name(types.as_ref(), found.block_type),
                        ),
                        glyph: label
                            .as_ref()
                            .and_then(|label| label.icon)
                            .map_or_else(String::new, |icon| icon.codepoint.to_owned()),
                        automatic: label.as_ref().is_some_and(|label| label.automatic),
                        loaded: found.is_some(),
                    }
                })
                .collect();
            sort_entries(
                &mut entries,
                sort.get_untracked(),
                descending.get_untracked(),
            );
            set_rows.set(entries);
        });
        Self {
            entries: create_memo(move || rows.get()),
            adds: RefCell::default(),
            client: Arc::clone(editor.client()),
        }
    }

    pub(crate) fn entries(&self) -> Memo<Vec<Entry>> {
        self.entries.clone()
    }

    pub(crate) fn accepts(&self, block_id: Uuid, folder: Uuid) -> bool {
        block_id != folder
            && untrack(|| {
                self.entries
                    .with(|entries| !entries.iter().any(|entry| entry.id == Some(block_id)))
            })
    }

    pub(crate) fn add(&self, folder: Uuid, block_id: Uuid) {
        self.adds
            .borrow_mut()
            .push(&self.client, folder, block_id, ());
    }

    pub(crate) fn poll_adds(&self, index: &BlockProjection<WorkspaceIndex>) {
        for (reference, ()) in self.adds.borrow_mut().poll() {
            index.operate(WorkspaceIndexOperation::Add(reference));
        }
    }
}

fn type_name(types: &dyn BlockTypes, block_type: Uuid) -> String {
    types
        .display_name(block_type)
        .map_or_else(|| block_type.to_string(), str::to_owned)
}

fn sort_entries(entries: &mut [Entry], sort: FolderSort, descending: bool) {
    if sort == FolderSort::Intrinsic {
        return;
    }
    let order: HashMap<BlockRef, usize> = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.reference, index))
        .collect();
    entries.sort_by(|left, right| {
        let loaded = right.loaded.cmp(&left.loaded);
        if loaded != Ordering::Equal {
            return loaded;
        }
        let ordering = match sort {
            FolderSort::Intrinsic => Ordering::Equal,
            FolderSort::Name => compare_name(left, right),
            FolderSort::Type => compare_type(left, right).then_with(|| compare_name(left, right)),
        };
        let ordering = match descending {
            true => ordering.reverse(),
            false => ordering,
        };
        ordering.then_with(|| order[&left.reference].cmp(&order[&right.reference]))
    });
}

fn compare_name(left: &Entry, right: &Entry) -> Ordering {
    left.name.to_lowercase().cmp(&right.name.to_lowercase())
}

fn compare_type(left: &Entry, right: &Entry) -> Ordering {
    left.type_name
        .to_lowercase()
        .cmp(&right.type_name.to_lowercase())
}
