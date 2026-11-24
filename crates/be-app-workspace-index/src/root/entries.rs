use be_type_workspace::{FolderContent, WorkspaceContent};
use std::cmp::Ordering;
use std::collections::HashMap;

use be_sdk_beui::be_block_content::BlockContent;
use be_sdk_beui::be_sdk_ui::BlockTypes;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::block_menu::{MenuBlock, can_move_out_of};
use be_sdk_beui::sections::workspaces;
use be_sdk_beui::{AccessLevel, BlockParent, BlockQuery};
use be_sdk_beui::{ContentProjection, Editor};
use uuid::Uuid;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum FolderSort {
    #[default]
    Intrinsic,
    Name,
    Type,
}

impl FolderSort {
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
    pub(crate) reference: Uuid,
    pub(crate) id: Option<Uuid>,
    pub(crate) block_type: Option<Uuid>,
    pub(crate) name: String,
    pub(crate) type_name: String,
    pub(crate) glyph: Option<be_sdk_beui::beui::icons::Glyph>,
    pub(crate) automatic: bool,
    pub(crate) loaded: bool,
    pub(crate) access: AccessLevel,
    pub(crate) parent: Option<BlockParent>,
}

#[derive(Clone, Copy)]
pub(crate) enum Listing {
    Folder(ContentProjection<FolderContent>),
    Workspace(ContentProjection<WorkspaceContent>),
}

impl Listing {
    pub(crate) fn of(editor: Editor) -> Self {
        match editor.host().block_type() == Some(WorkspaceContent::CONTENT_TYPE) {
            true => Self::Workspace(editor.block_content()),
            false => Self::Folder(editor.block_content()),
        }
    }

    fn blocks(self) -> Memo<Vec<Uuid>> {
        match self {
            Self::Folder(index) => index.project(|index| index.root().blocks()),
            Self::Workspace(index) => index.project(|index| index.root().blocks()),
        }
    }

    fn add(self, block_id: Uuid) {
        match self {
            Self::Folder(index) => {
                if let Ok(edit) = index.read(|index| index.root().add(block_id)) {
                    index.operate(edit);
                }
            }
            Self::Workspace(index) => {
                if let Ok(edit) = index.read(|index| index.root().add(block_id)) {
                    index.operate(edit);
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Folder {
    editor: Editor,
    entries: Memo<Vec<Entry>>,
    /// The entries by their reference, which a row finds its own in.
    by_reference: Memo<HashMap<Uuid, Entry>>,
    index: Listing,
    destinations: Memo<Vec<(Uuid, String)>>,
}

impl Folder {
    pub(crate) fn watch(
        editor: Editor,
        index: Listing,
        sort: ReadSignal<FolderSort>,
        descending: ReadSignal<bool>,
    ) -> Self {
        let references = editor
            .blocks()
            .watch(BlockQuery::References(editor.block_id()));
        let referenced = index.blocks();
        let entries = create_memo(move || {
            let metadata: HashMap<_, _> = references
                .read()
                .unwrap_or_default()
                .into_iter()
                .map(|reference| (reference.id, reference))
                .collect();
            let types = editor.block_types();
            let mut entries: Vec<_> = referenced
                .get()
                .into_iter()
                .map(|reference| {
                    let id = Some(reference);
                    let found = id.and_then(|id| metadata.get(&id));
                    let label = found.map(|found| found.label(types.as_ref()));
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
                        glyph: label.as_ref().and_then(be_sdk_beui::label_glyph),
                        automatic: label.as_ref().is_some_and(|label| label.automatic),
                        loaded: found.is_some(),
                        access: found.map_or(AccessLevel::None, |found| found.access),
                        parent: found.and_then(|found| found.parent),
                    }
                })
                .collect();
            sort_entries(&mut entries, sort.get(), descending.get());
            entries
        });
        let roots = editor.blocks().watch(BlockQuery::Roots);
        let host = editor.host();
        let destinations = create_memo(move || {
            let types = editor.block_types();
            workspaces(
                roots.read().unwrap_or_default(),
                host.organization_id(),
                host.account_id(),
                types.as_ref(),
            )
            .into_iter()
            .map(|workspace| (workspace.id, workspace.label(types.as_ref()).name))
            .collect()
        });
        let by_reference = create_memo(move || {
            entries.with(|entries| {
                entries
                    .iter()
                    .map(|entry| (entry.reference, entry.clone()))
                    .collect()
            })
        });
        Self {
            editor,
            entries,
            by_reference,
            index,
            destinations,
        }
    }

    /// The workspaces an entry can be moved to.
    pub(crate) fn destinations(&self) -> Memo<Vec<(Uuid, String)>> {
        self.destinations
    }

    /// Entry `key`, as it changes.
    pub(crate) fn entry(&self, key: Uuid) -> Memo<Option<Entry>> {
        let by_reference = self.by_reference;
        create_memo(move || by_reference.with(|entries| entries.get(&key).cloned()))
    }

    /// Entry `key`, read where it tracks.
    pub(crate) fn entry_tracked(&self, key: Uuid) -> Option<Entry> {
        self.by_reference.with(|entries| entries.get(&key).cloned())
    }

    /// Entry `key` as it is now.
    pub(crate) fn entry_now(&self, key: Uuid) -> Option<Entry> {
        self.by_reference
            .with_untracked(|entries| entries.get(&key).cloned())
    }

    /// Entry `key` as its menu acts on it, as it changes.
    pub(crate) fn menu_block(&self, key: Uuid) -> Memo<Option<MenuBlock>> {
        let (editor, by_reference) = (self.editor, self.by_reference);
        create_memo(move || {
            by_reference.with(|entries| {
                entries
                    .get(&key)
                    .and_then(|entry| menu_block(editor, entry))
            })
        })
    }

    /// Entry `key` as its menu acts on it now.
    pub(crate) fn menu_block_now(&self, key: Uuid) -> Option<MenuBlock> {
        let editor = self.editor;
        self.by_reference.with_untracked(|entries| {
            entries
                .get(&key)
                .and_then(|entry| menu_block(editor, entry))
        })
    }

    pub(crate) fn entries(&self) -> Memo<Vec<Entry>> {
        self.entries
    }

    pub(crate) fn accepts(&self, block_id: Uuid, folder: Uuid) -> bool {
        block_id != folder
            && untrack(|| {
                self.entries
                    .with(|entries| !entries.iter().any(|entry| entry.id == Some(block_id)))
            })
    }

    pub(crate) fn add(&self, block_id: Uuid) {
        self.index.add(block_id);
    }
}

/// `entry` as its menu acts on it: listed in this folder, which it lives in unless it is a link.
fn menu_block(editor: Editor, entry: &Entry) -> Option<MenuBlock> {
    let (id, block_type) = (entry.id?, entry.block_type?);
    let folder = editor.block_id();
    let source = BlockParent::Block(folder);
    let is_reference = entry.parent != Some(source);
    let types = editor.block_types();
    let can_delete = can_move_out_of(
        &editor.blocks(),
        types.as_ref(),
        |block| {
            (block == folder)
                .then(|| editor.host().block_type())
                .flatten()
        },
        source,
        id,
        is_reference,
    );
    Some(MenuBlock {
        id,
        block_type,
        source,
        is_reference,
        access: entry.access,
        can_delete,
    })
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
    let order: HashMap<Uuid, usize> = entries
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
