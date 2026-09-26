use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use block_editor_plugin::beui::reactive::{
    Memo, WriteSignal, create_effect, create_memo, create_signal, untrack,
};
use block_editor_plugin::block_ui::BlockTypes;
use block_editor_plugin::{AccessLevel, BlockInfo, BlockList, BlockParent, BlockQuery, Blocks};
use block_editor_plugin::{BlockSource, Editor};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) enum RowKey {
    Block(Vec<Uuid>),
    Orphans,
    Note(Vec<Uuid>),
}

#[derive(Clone, PartialEq)]
pub(crate) struct Row {
    pub(crate) key: RowKey,
    pub(crate) id: Option<Uuid>,
    pub(crate) block_type: Uuid,
    pub(crate) label: String,
    pub(crate) glyph: String,
    pub(crate) automatic: bool,
    pub(crate) depth: usize,
    pub(crate) expandable: bool,
    pub(crate) expanded: bool,
    pub(crate) container: Option<Uuid>,
    pub(crate) source: BlockSource,
    pub(crate) is_reference: bool,
    pub(crate) access: AccessLevel,
    pub(crate) dynamic_artifact: bool,
    pub(crate) parent: BlockParent,
    pub(crate) can_add: bool,
    pub(crate) can_edit: bool,
    pub(crate) can_delete: bool,
    pub(crate) unlink: Result<(), &'static str>,
    pub(crate) inspection: Option<Inspection>,
}

#[derive(Clone, PartialEq, Default)]
pub(crate) struct Inspection {
    pub(crate) name: String,
    pub(crate) id: String,
    pub(crate) block_type: String,
    pub(crate) author: String,
    pub(crate) parent: String,
    pub(crate) references: String,
    pub(crate) access: String,
    pub(crate) generated: String,
    pub(crate) shown_as: String,
}

impl Row {
    pub(crate) fn note(path: Vec<Uuid>, depth: usize, label: &str) -> Self {
        Self {
            key: RowKey::Note(path),
            id: None,
            block_type: Uuid::nil(),
            label: label.to_owned(),
            glyph: String::new(),
            automatic: true,
            depth,
            expandable: false,
            expanded: false,
            container: None,
            source: BlockSource::Root,
            is_reference: false,
            access: AccessLevel::None,
            dynamic_artifact: false,
            parent: BlockParent::Root,
            can_add: false,
            can_edit: false,
            can_delete: false,
            unlink: Err("Loading…"),
            inspection: None,
        }
    }
}

#[derive(Default)]
struct Watched {
    expanded: HashMap<Uuid, BlockList>,
    orphans: Option<BlockList>,
    block_types: HashMap<Uuid, Uuid>,
}

pub(crate) struct Tree {
    client: Blocks,
    watched: Rc<RefCell<Watched>>,
    rows: Memo<Vec<Row>>,
    orphans_open: Memo<bool>,
    set_expanded: block_editor_plugin::beui::reactive::WriteSignal<HashSet<Uuid>>,
    set_orphans_open: block_editor_plugin::beui::reactive::WriteSignal<bool>,
    set_remembered: WriteSignal<u64>,
}

impl Tree {
    pub(crate) fn watch(editor: &Editor) -> Rc<Self> {
        let client = editor.blocks();
        let roots = client.watch(BlockQuery::Roots);
        let watched = Rc::new(RefCell::new(Watched::default()));
        let (expanded, set_expanded) = create_signal(HashSet::<Uuid>::new());
        let (orphans_open, set_orphans_open) = create_signal(false);
        let (rows, set_rows) = create_signal(Vec::<Row>::new());
        let (remembered, set_remembered) = create_signal(0_u64);

        let build = Rc::clone(&watched);
        let host = editor.host().clone();
        let building = client.clone();
        let open = expanded.clone();
        let orphans = orphans_open.clone();
        create_effect(move || {
            remembered.get();
            let mut watched = build.borrow_mut();
            let open = open.get();
            watched.expanded.retain(|id, _| open.contains(id));
            for id in &open {
                watched
                    .expanded
                    .entry(*id)
                    .or_insert_with(|| building.watch(BlockQuery::References(*id)));
            }
            let showing = orphans.get();
            if showing && watched.orphans.is_none() {
                watched.orphans = Some(building.watch(BlockQuery::Detached));
            }
            if !showing {
                watched.orphans = None;
            }

            let types = host.block_types();
            let mut builder = Builder {
                client: &building,
                types: types.as_ref(),
                watched: &mut watched,
                rows: Vec::new(),
            };
            let listed = roots.read();
            if listed.is_empty() && roots.is_loaded() {
                builder
                    .rows
                    .push(Row::note(Vec::new(), 0, "No root blocks"));
            }
            for root in listed {
                builder.push(root, 0, None, &mut Vec::new());
            }
            builder.push_orphans(showing);
            let rows = builder.rows;
            drop(watched);
            set_rows.set(rows);
        });

        Rc::new(Self {
            client,
            watched,
            rows: create_memo(move || rows.get()),
            orphans_open: create_memo(move || orphans_open.get()),
            set_expanded,
            set_orphans_open,
            set_remembered,
        })
    }

    pub(crate) fn rows(&self) -> Memo<Vec<Row>> {
        self.rows.clone()
    }

    pub(crate) fn keys(&self) -> Memo<Vec<RowKey>> {
        let rows = self.rows.clone();
        create_memo(move || rows.with(|rows| rows.iter().map(|row| row.key.clone()).collect()))
    }

    pub(crate) fn row(&self, key: RowKey) -> Memo<Option<Row>> {
        let rows = self.rows.clone();
        create_memo(move || rows.with(|rows| rows.iter().find(|row| row.key == key).cloned()))
    }

    pub(crate) fn remember(&self, id: Uuid, block_type: Uuid) {
        self.watched.borrow_mut().block_types.insert(id, block_type);
        self.set_remembered.update(|count| *count += 1);
    }

    pub(crate) fn blocks(&self) -> &Blocks {
        &self.client
    }

    pub(crate) fn toggle(&self, key: &RowKey, id: Option<Uuid>, expanded: bool) {
        match key {
            RowKey::Orphans => {
                let open = untrack(|| self.orphans_open.get());
                self.set_orphans_open.set(!open);
            }
            RowKey::Block(_) => {
                let Some(id) = id else {
                    return;
                };
                self.set_expanded.update(|open| match expanded {
                    true => {
                        open.remove(&id);
                    }
                    false => {
                        open.insert(id);
                    }
                });
            }
            RowKey::Note(_) => {}
        }
    }

    pub(crate) fn expand(&self, ancestors: impl IntoIterator<Item = Uuid>) {
        let ancestors: Vec<_> = ancestors.into_iter().collect();
        self.set_expanded.update(|open| open.extend(ancestors));
    }

    pub(crate) fn show_orphans(&self) {
        self.set_orphans_open.set(true);
    }
}

struct Builder<'a> {
    client: &'a Blocks,
    types: &'a dyn BlockTypes,
    watched: &'a mut Watched,
    rows: Vec<Row>,
}

impl Builder<'_> {
    fn push_orphans(&mut self, open: bool) {
        self.rows.push(Row {
            key: RowKey::Orphans,
            label: "Recently Deleted".to_owned(),
            expandable: true,
            expanded: open,
            ..Row::note(Vec::new(), 0, "Recently Deleted")
        });
        if !open {
            return;
        }
        let Some(orphans) = self.watched.orphans.as_ref() else {
            return;
        };
        let listed = orphans.read();
        let loaded = orphans.is_loaded();
        if listed.is_empty() && loaded {
            self.rows
                .push(Row::note(Vec::new(), 1, "No recently deleted blocks"));
            return;
        }
        for block in listed {
            self.push(block, 1, None, &mut Vec::new());
        }
    }

    fn push(
        &mut self,
        reference: BlockInfo,
        depth: usize,
        container: Option<Uuid>,
        path: &mut Vec<Uuid>,
    ) {
        self.watched
            .block_types
            .insert(reference.id, reference.block_type);
        let is_reference = container.is_some_and(|id| reference.parent != BlockParent::Block(id));
        let source = container.map_or_else(
            || match reference.parent {
                BlockParent::Detached => BlockSource::Orphaned,
                BlockParent::Root | BlockParent::Block(_) => BlockSource::Root,
            },
            BlockSource::Block,
        );
        let access = self.client.access(reference.id);
        let can_edit = access.can_edit();
        let can_add = self.types.child_edits(reference.block_type).add && can_edit;
        let can_delete = source != BlockSource::Orphaned
            && self.can_move_out_of(source, reference.id, is_reference);
        let unlink = self.unlink_permission(container);
        let expandable = !is_reference && !reference.references.is_empty();
        let mut key_path = path.clone();
        key_path.push(reference.id);
        let expanded = expandable && self.watched.expanded.contains_key(&reference.id);
        let label = reference.label(self.types);
        let inspection = self.inspect(&reference, &label.name, access, is_reference);
        self.rows.push(Row {
            key: RowKey::Block(key_path.clone()),
            id: Some(reference.id),
            block_type: reference.block_type,
            label: label.name,
            glyph: label.icon.map(str::to_owned).unwrap_or_default(),
            automatic: label.automatic,
            depth,
            expandable,
            expanded,
            container,
            source,
            is_reference,
            access,
            dynamic_artifact: reference.is_artifact(),
            parent: reference.parent,
            can_add,
            can_edit,
            can_delete,
            unlink,
            inspection: Some(inspection),
        });
        if !expanded || path.contains(&reference.id) {
            return;
        }
        let watched = &self.watched.expanded[&reference.id];
        let children = watched.read();
        let loaded = watched.is_loaded();
        if children.is_empty() && loaded {
            self.rows
                .push(Row::note(key_path.clone(), depth + 1, "No references"));
            return;
        }
        path.push(reference.id);
        for child in children {
            self.push(child, depth + 1, Some(reference.id), path);
        }
        path.pop();
    }

    fn inspect(
        &self,
        block: &BlockInfo,
        name: &str,
        access: AccessLevel,
        is_reference: bool,
    ) -> Inspection {
        let named = match block.named_by_hand {
            true => name.to_owned(),
            false => format!("{name} (automatic)"),
        };
        let parent = match block.parent {
            BlockParent::Root => "Root".to_owned(),
            BlockParent::Detached => "Recently Deleted".to_owned(),
            BlockParent::Block(id) => self.described(id),
        };
        let author = match block.author.is_nil() {
            true => "Unknown".to_owned(),
            false => block.author.to_string(),
        };
        let generated = block.artifact.as_ref().map_or_else(
            || "No".to_owned(),
            |artifact| format!("From {}", self.type_name(artifact.source_type)),
        );
        Inspection {
            name: named,
            id: block.id.to_string(),
            block_type: self.type_name(block.block_type),
            author,
            parent,
            references: block.references.len().to_string(),
            access: access_name(access).to_owned(),
            generated,
            shown_as: match is_reference {
                true => "A link to a block that lives elsewhere".to_owned(),
                false => "The block itself".to_owned(),
            },
        }
    }

    fn type_name(&self, block_type: Uuid) -> String {
        match self.types.display_name(block_type) {
            Some(name) => format!("{name} ({block_type})"),
            None => block_type.to_string(),
        }
    }

    fn described(&self, id: Uuid) -> String {
        self.rows
            .iter()
            .find(|row| row.id == Some(id))
            .map_or_else(|| id.to_string(), |row| format!("{} ({id})", row.label))
    }

    fn can_move_out_of(&self, source: BlockSource, child: Uuid, is_reference: bool) -> bool {
        can_move_out_of(
            self.client,
            self.types,
            &self.watched.block_types,
            source,
            child,
            is_reference,
        )
    }

    fn unlink_permission(&self, container: Option<Uuid>) -> Result<(), &'static str> {
        unlink_permission(
            self.client,
            self.types,
            &self.watched.block_types,
            container,
        )
    }
}

pub(crate) fn can_move_out_of(
    client: &Blocks,
    types: &dyn BlockTypes,
    block_types: &HashMap<Uuid, Uuid>,
    source: BlockSource,
    child: Uuid,
    is_reference: bool,
) -> bool {
    let can_delete = match source {
        BlockSource::Root | BlockSource::Orphaned => true,
        BlockSource::Block(id) => {
            block_types
                .get(&id)
                .is_some_and(|block_type| types.child_edits(*block_type).delete)
                && client.access(id).can_edit()
        }
    };
    can_delete && (is_reference || client.access(child).can_edit())
}

pub(crate) fn unlink_permission(
    client: &Blocks,
    types: &dyn BlockTypes,
    block_types: &HashMap<Uuid, Uuid>,
    container: Option<Uuid>,
) -> Result<(), &'static str> {
    let container_type = container.and_then(|id| block_types.get(&id).copied());
    match (container, container_type) {
        (Some(_), Some(block_type)) if !types.child_edits(block_type).replace => {
            Err("This container doesn't support replacing a reference")
        }
        (Some(container), Some(_)) if !client.access(container).can_edit() => {
            Err("You don't have permission to edit this container")
        }
        (Some(_), Some(_)) => Ok(()),
        _ => Err("Loading…"),
    }
}

pub(crate) fn access_name(access: AccessLevel) -> &'static str {
    match access {
        AccessLevel::Edit => "Can edit",
        AccessLevel::View => "Can view",
        AccessLevel::KnowExists => "Can see that it exists",
        AccessLevel::None => "None",
    }
}

pub(crate) fn access_hint(access: AccessLevel) -> &'static str {
    match access {
        AccessLevel::Edit => "",
        AccessLevel::View => "You can view this block, but not change it.",
        AccessLevel::KnowExists | AccessLevel::None => {
            "You can see that this block exists, but not open it."
        }
    }
}

pub(crate) fn access_marker(access: AccessLevel) -> Option<&'static str> {
    match access {
        AccessLevel::Edit => None,
        AccessLevel::View => Some(block_editor_plugin::beui::icons::ICON_VISIBILITY),
        AccessLevel::KnowExists | AccessLevel::None => {
            Some(block_editor_plugin::beui::icons::ICON_LOCK)
        }
    }
}

#[cfg(test)]
mod tests;
