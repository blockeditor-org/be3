use block_editor_beui::beui::icons::Glyph;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::{Duration, Instant};
use workspace_ui_types::ViewState;

use block_editor_beui::be_block::SETTINGS;
use block_editor_beui::be_block::metadata::ICON_COLOR;
use block_editor_beui::be_block::{Root, settings::Settings};
use block_editor_beui::beui::icons::{
    ICON_BOXES, ICON_DATABASE, ICON_ROTATE_CCW_CLOCK, ICON_USERS, named,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::block_ui::{BlockTypes, Press};
pub(crate) use block_editor_beui::sections::{own_system, shared_with_me, workspaces};
use block_editor_beui::{AccessLevel, BlockInfo, BlockList, BlockParent, BlockQuery, Blocks};
use block_editor_beui::{Editor, label_glyph};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub(crate) enum RowKey {
    Block(Vec<Uuid>),
    Shared,
    Orphans,
    System,
    Note(Vec<Uuid>),
    SharedNote,
    OrphansNote,
    SystemNote,
}

#[derive(Clone, PartialEq)]
pub(crate) struct Row {
    pub(crate) key: RowKey,
    pub(crate) id: Option<Uuid>,
    pub(crate) block_type: Uuid,
    pub(crate) label: String,
    pub(crate) glyph: Option<Glyph>,
    pub(crate) icon_color: Option<[u8; 3]>,
    pub(crate) automatic: bool,
    pub(crate) depth: usize,
    pub(crate) section: bool,
    pub(crate) expandable: bool,
    pub(crate) expanded: bool,
    pub(crate) press: Press,
    pub(crate) container: Option<Uuid>,
    pub(crate) source: BlockParent,
    pub(crate) is_reference: bool,
    pub(crate) access: AccessLevel,
    pub(crate) dynamic_artifact: bool,
    pub(crate) parent: Option<BlockParent>,
    pub(crate) can_add: bool,
    pub(crate) can_edit: bool,
    pub(crate) can_delete: bool,
    pub(crate) can_leave: bool,
    pub(crate) unlink: Result<(), &'static str>,
    pub(crate) inspection: Option<Inspection>,
    pub(crate) deleted: bool,
    pub(crate) marked_deleted: bool,
    pub(crate) sets_access: bool,
    pub(crate) settings: Option<Uuid>,
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
        Self::note_at(RowKey::Note(path), depth, label)
    }

    pub(crate) fn note_at(key: RowKey, depth: usize, label: &str) -> Self {
        Self {
            key,
            id: None,
            block_type: Uuid::nil(),
            label: label.to_owned(),
            glyph: None,
            icon_color: None,
            automatic: true,
            depth,
            section: false,
            expandable: false,
            expanded: false,
            press: Press::Open,
            container: None,
            source: BlockParent::Root,
            is_reference: false,
            access: AccessLevel::None,
            dynamic_artifact: false,
            parent: None,
            can_add: false,
            can_edit: false,
            can_delete: false,
            can_leave: false,
            unlink: Err("Loading…"),
            inspection: None,
            deleted: false,
            marked_deleted: false,
            sets_access: false,
            settings: None,
        }
    }

    pub(crate) fn is_workspace(&self) -> bool {
        self.section && self.id.is_some()
    }
}

const OPENED: &str = "opened";

#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
struct Opened {
    expanded: Vec<Uuid>,
    orphans: bool,
    collapsed: Vec<Uuid>,
    shared_closed: bool,
    system: bool,
}

#[derive(Clone, Copy, Default, PartialEq)]
struct Folded {
    orphans_open: bool,
    shared_closed: bool,
    system_open: bool,
}

const GRACE: Duration = Duration::from_millis(300);

struct Listing {
    list: Rc<BlockList>,
    since: Instant,
}

impl Listing {
    fn watch(client: &Blocks, query: BlockQuery) -> Self {
        Self {
            list: Rc::new(client.watch(query)),
            since: now(),
        }
    }
}

#[derive(Clone, Copy)]
struct Clock {
    now: Instant,
    due: Option<Instant>,
}

impl Clock {
    fn listed(&mut self, listing: &Listing) -> Result<Vec<BlockInfo>, Option<String>> {
        match listing.list.read() {
            Ok(blocks) => Ok(blocks),
            Err(Pending::Failed(error)) => Err(Some(error)),
            Err(Pending::Loading) if self.now >= listing.since + GRACE => {
                Err(Some("Loading…".to_owned()))
            }
            Err(Pending::Loading) => {
                let due = listing.since + GRACE;
                self.due = Some(self.due.map_or(due, |held| held.min(due)));
                Err(None)
            }
        }
    }
}

#[derive(Default)]
struct Watched {
    expanded: HashMap<Uuid, Listing>,
    sections: HashMap<Uuid, Listing>,
    orphans: Option<Listing>,
    block_types: HashMap<Uuid, Uuid>,
    workspaces: HashSet<Uuid>,
    system: HashSet<Uuid>,
}

#[derive(Clone, Copy)]
pub(crate) struct Tree {
    inner: Stored<Inner>,
    rows: Memo<Vec<Row>>,
    folded: Signal<Folded>,
    expanded: Signal<HashSet<Uuid>>,
    collapsed: Signal<HashSet<Uuid>>,
    remembered: Signal<u64>,
}

struct Inner {
    watched: Rc<RefCell<Watched>>,
}

impl Tree {
    pub(crate) fn watch(editor: Editor) -> Self {
        let client = editor.blocks();
        let roots = Listing::watch(&client, BlockQuery::Roots);
        let shared = Listing::watch(&client, BlockQuery::Shared);
        let watched = Rc::new(RefCell::new(Watched::default()));
        let expanded = create_signal(HashSet::<Uuid>::new());
        let collapsed = create_signal(HashSet::<Uuid>::new());
        let folded = create_signal(Folded::default());
        let rows = create_signal(Vec::<Row>::new());
        let remembered = create_signal(0_u64);
        let tick = create_signal(0_u64);
        let timer = create_timer(move || {
            tick.update(|count| *count += 1);
            None
        });

        let restored = Rc::new(Cell::new(false));
        let adopted = Rc::clone(&restored);
        let view = editor.view_content();
        create_binding(move || {
            if adopted.get() {
                return;
            }
            let Some(view) = view else {
                return;
            };
            let Ok(opened) = view.read(|held| {
                held.root()
                    .state(OPENED)
                    .and_then(ViewState::value::<Opened>)
            }) else {
                return;
            };
            untrack(|| {
                if let Some(opened) = opened {
                    expanded.set(opened.expanded.into_iter().collect());
                    collapsed.set(opened.collapsed.into_iter().collect());
                    folded.set(Folded {
                        orphans_open: opened.orphans,
                        shared_closed: opened.shared_closed,
                        system_open: opened.system,
                    });
                }
            });
            adopted.set(true);
        });

        let build = Rc::clone(&watched);
        let host = editor.host();
        let building = client.clone();
        create_binding(move || {
            remembered.get();
            tick.get();
            let mut watched = build.borrow_mut();
            let open = expanded.get();
            let collapsed = collapsed.get();
            let Folded {
                orphans_open: showing,
                shared_closed,
                system_open,
            } = folded.get();
            watched.expanded.retain(|id, _| open.contains(id));
            for id in &open {
                watched
                    .expanded
                    .entry(*id)
                    .or_insert_with(|| Listing::watch(&building, BlockQuery::References(*id)));
            }
            if showing && watched.orphans.is_none() {
                watched.orphans = Some(Listing::watch(&building, BlockQuery::Detached));
            }
            if !showing {
                watched.orphans = None;
            }

            let types = host.block_types();
            let mut clock = Clock {
                now: now(),
                due: None,
            };
            let listed = clock.listed(&roots);
            let held = listed.clone().unwrap_or_default();
            let sections = workspaces(held.clone(), host.account_id(), types.as_ref());
            let system = listed
                .as_ref()
                .map(|_| own_system(held.clone(), host.account_id()))
                .map_err(Clone::clone);
            let loose = clock.listed(&shared).map(shared_with_me);
            watched.system = system.iter().flatten().map(|block| block.id).collect();
            watched.workspaces = sections.iter().map(|block| block.id).collect();
            let shown: HashSet<Uuid> = sections
                .iter()
                .map(|block| block.id)
                .filter(|id| !collapsed.contains(id))
                .collect();
            watched.sections.retain(|id, _| shown.contains(id));
            for id in &shown {
                watched
                    .sections
                    .entry(*id)
                    .or_insert_with(|| Listing::watch(&building, BlockQuery::References(*id)));
            }
            let mut builder = Builder {
                account: host.account_id(),
                client: &building,
                types: types.as_ref(),
                watched: &mut watched,
                clock,
                rows: Vec::new(),
            };
            match listed {
                Ok(_) if sections.is_empty() => {
                    builder
                        .rows
                        .push(Row::note(Vec::new(), 0, "No workspaces yet"))
                }
                Err(Some(error)) => builder.rows.push(Row::note(Vec::new(), 0, &error)),
                Ok(_) | Err(None) => {}
            }
            for section in sections {
                let open = shown.contains(&section.id);
                builder.push_section(section, open);
            }
            builder.push_shared(loose, !shared_closed);
            builder.push_orphans(showing);
            builder.push_system(system, system_open);
            let due = builder.clock.due;
            let built = builder.rows;
            drop(watched);
            if let Some(due) = due {
                timer.start(due.saturating_duration_since(clock.now));
            }
            rows.set(built);
        });
        create_effect(move || {
            let mut expanded: Vec<Uuid> = expanded.get().into_iter().collect();
            let mut collapsed: Vec<Uuid> = collapsed.get().into_iter().collect();
            let Folded {
                orphans_open,
                shared_closed,
                system_open,
            } = folded.get();
            if !restored.get() {
                return;
            }
            expanded.sort_unstable();
            collapsed.sort_unstable();
            let opened = ViewState::new(
                &Opened {
                    expanded,
                    orphans: orphans_open,
                    collapsed,
                    shared_closed,
                    system: system_open,
                },
                Vec::new(),
            );
            untrack(|| editor.set_view_state(OPENED, Some(&opened)));
        });

        Self {
            inner: store(Inner { watched }),
            rows: create_memo(move || rows.get()),
            folded,
            expanded,
            collapsed,
            remembered,
        }
    }

    pub(crate) fn rows(&self) -> Memo<Vec<Row>> {
        self.rows
    }

    pub(crate) fn row(&self, key: RowKey) -> Memo<Option<Row>> {
        let rows = self.rows;
        create_memo(move || rows.with(|rows| rows.iter().find(|row| row.key == key).cloned()))
    }

    pub(crate) fn remember(&self, id: Uuid, block_type: Uuid) {
        self.inner.with(|inner| {
            inner
                .watched
                .borrow_mut()
                .block_types
                .insert(id, block_type)
        });
        self.remembered.update(|count| *count += 1);
    }

    pub(crate) fn workspaces(&self) -> HashSet<Uuid> {
        self.inner
            .with(|inner| inner.watched.borrow().workspaces.clone())
    }

    pub(crate) fn toggle(&self, key: &RowKey, id: Option<Uuid>, expanded: bool) {
        match key {
            RowKey::Orphans => {
                let folded = self.folded.get_untracked();
                self.folded.set(Folded {
                    orphans_open: !folded.orphans_open,
                    ..folded
                });
            }
            RowKey::Shared => {
                let folded = self.folded.get_untracked();
                self.folded.set(Folded {
                    shared_closed: !folded.shared_closed,
                    ..folded
                });
            }
            RowKey::System => {
                let folded = self.folded.get_untracked();
                self.folded.set(Folded {
                    system_open: !folded.system_open,
                    ..folded
                });
            }
            RowKey::Block(path)
                if path.len() == 1 && id.is_some_and(|id| self.workspaces().contains(&id)) =>
            {
                let Some(id) = id else {
                    return;
                };
                self.collapsed.update(|collapsed| match expanded {
                    true => {
                        collapsed.insert(id);
                    }
                    false => {
                        collapsed.remove(&id);
                    }
                });
            }
            RowKey::Block(_) => {
                let Some(id) = id else {
                    return;
                };
                self.expanded.update(|open| match expanded {
                    true => {
                        open.remove(&id);
                    }
                    false => {
                        open.insert(id);
                    }
                });
            }
            RowKey::Note(_) | RowKey::SharedNote | RowKey::OrphansNote | RowKey::SystemNote => {}
        }
    }

    pub(crate) fn expand(&self, ancestors: impl IntoIterator<Item = Uuid>) {
        let ancestors: Vec<_> = ancestors.into_iter().collect();
        self.expanded.update(|open| open.extend(ancestors));
    }

    pub(crate) fn show_orphans(&self) {
        let folded = self.folded.get_untracked();
        self.folded.set(Folded {
            orphans_open: true,
            ..folded
        });
    }

    pub(crate) fn reveal_system(&self, root: Option<Uuid>) {
        let held = self
            .inner
            .with(|inner| root.is_some_and(|id| inner.watched.borrow().system.contains(&id)));
        let folded = self.folded.get_untracked();
        if held && !folded.system_open {
            self.folded.set(Folded {
                system_open: true,
                ..folded
            });
        }
    }
}

struct Builder<'a> {
    account: Uuid,
    client: &'a Blocks,
    types: &'a dyn BlockTypes,
    watched: &'a mut Watched,
    clock: Clock,
    rows: Vec<Row>,
}

fn note_for(
    rows: &mut Vec<Row>,
    listed: Result<Vec<BlockInfo>, Option<String>>,
    empty: &str,
    note: impl Fn(&str) -> Row,
) -> Option<Vec<BlockInfo>> {
    match listed {
        Ok(blocks) if blocks.is_empty() => rows.push(note(empty)),
        Ok(blocks) => return Some(blocks),
        Err(Some(error)) => rows.push(note(&error)),
        Err(None) => {}
    }
    None
}

impl Builder<'_> {
    fn push_orphans(&mut self, open: bool) {
        self.rows.push(Row {
            key: RowKey::Orphans,
            label: "Recently Deleted".to_owned(),
            glyph: Some(ICON_ROTATE_CCW_CLOCK),
            section: true,
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
        let listed = self.clock.listed(orphans);
        let Some(listed) = note_for(
            &mut self.rows,
            listed,
            "No recently deleted blocks",
            |label| Row::note_at(RowKey::OrphansNote, 1, label),
        ) else {
            return;
        };
        for block in listed {
            self.push(block, 1, None, &mut Vec::new(), true);
        }
    }

    fn push_system(&mut self, blocks: Result<Vec<BlockInfo>, Option<String>>, open: bool) {
        self.rows.push(Row {
            section: true,
            expandable: true,
            expanded: open,
            glyph: Some(ICON_DATABASE),
            ..Row::note_at(RowKey::System, 0, "System")
        });
        if !open {
            return;
        }
        let Some(blocks) = note_for(&mut self.rows, blocks, "Nothing here yet", |label| {
            Row::note_at(RowKey::SystemNote, 1, label)
        }) else {
            return;
        };
        for block in blocks {
            self.push(block, 1, None, &mut Vec::new(), false);
        }
    }

    fn push_section(&mut self, section: BlockInfo, open: bool) {
        self.watched
            .block_types
            .insert(section.id, section.block_type);
        let path = vec![section.id];
        let mut row = describe(
            self.client,
            self.types,
            &self.watched.block_types,
            &section,
            Placement {
                key: RowKey::Block(path.clone()),
                depth: 0,
                container: None,
                expanded: false,
                trashed: false,
            },
            |id| id.to_string(),
        );
        row.section = true;
        if row.glyph.is_none() {
            row.glyph = Some(ICON_BOXES);
        }
        row.expandable = true;
        row.expanded = open;
        row.can_leave = section.author != Some(self.account);
        self.rows.push(row);
        if !open {
            return;
        }
        let Some(watched) = self.watched.sections.get(&section.id) else {
            return;
        };
        let listed = self.clock.listed(watched);
        let Some(children) = note_for(&mut self.rows, listed, "Nothing here yet", |label| {
            Row::note(path.clone(), 1, label)
        }) else {
            return;
        };
        let mut path = path;
        for child in children {
            self.push(child, 1, Some(section.id), &mut path, false);
        }
    }

    fn push_shared(&mut self, shared: Result<Vec<BlockInfo>, Option<String>>, open: bool) {
        self.rows.push(Row {
            key: RowKey::Shared,
            section: true,
            expandable: true,
            expanded: open,
            glyph: Some(ICON_USERS),
            ..Row::note_at(RowKey::Shared, 0, "Shared with me")
        });
        if !open {
            return;
        }
        let Some(shared) = note_for(
            &mut self.rows,
            shared,
            "Nothing has been shared with you",
            |label| Row::note_at(RowKey::SharedNote, 1, label),
        ) else {
            return;
        };
        for block in shared {
            let at = self.rows.len();
            let leavable = block.author != Some(self.account);
            self.push(block, 1, None, &mut Vec::new(), false);
            self.rows[at].can_leave = leavable;
        }
    }

    fn push(
        &mut self,
        reference: BlockInfo,
        depth: usize,
        container: Option<Uuid>,
        path: &mut Vec<Uuid>,
        trashed: bool,
    ) {
        if reference.block_type == <Settings as Root>::CONTENT_TYPE
            && container
                .is_some_and(|container| reference.parent == Some(BlockParent::Block(container)))
        {
            return;
        }
        self.watched
            .block_types
            .insert(reference.id, reference.block_type);
        let mut key_path = path.clone();
        key_path.push(reference.id);
        let expanded = self.watched.expanded.contains_key(&reference.id);
        let rows = &self.rows;
        let row = describe(
            self.client,
            self.types,
            &self.watched.block_types,
            &reference,
            Placement {
                key: RowKey::Block(key_path.clone()),
                depth,
                container,
                expanded,
                trashed,
            },
            |id| {
                rows.iter()
                    .find(|row| row.id == Some(id))
                    .map_or_else(|| id.to_string(), |row| format!("{} ({id})", row.label))
            },
        );
        let expanded = row.expanded;
        let trashed = trashed || row.deleted;
        self.rows.push(row);
        if !expanded || path.contains(&reference.id) {
            return;
        }
        let listed = self.clock.listed(&self.watched.expanded[&reference.id]);
        let Some(children) = note_for(&mut self.rows, listed, "No references", |label| {
            Row::note(key_path.clone(), depth + 1, label)
        }) else {
            return;
        };
        path.push(reference.id);
        for child in children {
            self.push(child, depth + 1, Some(reference.id), path, trashed);
        }
        path.pop();
    }
}

pub(crate) struct Placement {
    pub(crate) key: RowKey,
    pub(crate) depth: usize,
    pub(crate) container: Option<Uuid>,
    pub(crate) expanded: bool,
    pub(crate) trashed: bool,
}

pub(crate) fn describe(
    client: &Blocks,
    types: &dyn BlockTypes,
    block_types: &HashMap<Uuid, Uuid>,
    reference: &BlockInfo,
    placement: Placement,
    described: impl Fn(Uuid) -> String,
) -> Row {
    let Placement {
        key,
        depth,
        container,
        expanded,
        trashed,
    } = placement;
    let is_reference = container.is_some_and(|id| reference.parent != Some(BlockParent::Block(id)));
    let source = container.map_or_else(
        || match reference.parent {
            Some(BlockParent::Detached) => BlockParent::Detached,
            Some(BlockParent::Root | BlockParent::Block(_)) | None => BlockParent::Root,
        },
        BlockParent::Block,
    );
    let access = client.access(reference.id);
    let can_edit = access.can_edit();
    let can_add = types.child_edits(reference.block_type).add && can_edit;
    let can_delete = source != BlockParent::Detached
        && can_move_out_of(
            client,
            types,
            block_types,
            source,
            reference.id,
            is_reference,
        );
    let unlink = unlink_permission(client, types, block_types, container);
    let expandable = !is_reference && !reference.references.is_empty();
    let label = reference
        .label(types)
        .deleted(reference.deleted && !trashed);
    let ordinary = label.reach == block_editor_beui::block_ui::Reach::Open && !label.deleted;
    let default_glyph = label_glyph(&label);
    let (glyph, icon_color) = if ordinary {
        (
            reference
                .metadata
                .icon()
                .and_then(|icon| named(&icon))
                .or(default_glyph),
            reference
                .metadata
                .get(&ICON_COLOR)
                .or_else(|| types.color(reference.block_type)),
        )
    } else {
        (default_glyph, None)
    };
    let inspection = inspect(
        types,
        reference,
        &label.name,
        access,
        is_reference,
        described,
    );
    Row {
        key,
        id: Some(reference.id),
        block_type: reference.block_type,
        label: label.name.clone(),
        glyph,
        icon_color,
        automatic: label.automatic,
        depth,
        section: false,
        expandable,
        expanded: expandable && expanded,
        press: types.press(reference.block_type),
        container,
        source,
        is_reference,
        access,
        dynamic_artifact: reference.is_artifact(),
        parent: reference.parent,
        can_add,
        can_edit,
        can_delete,
        can_leave: false,
        unlink,
        inspection: Some(inspection),
        deleted: reference.deleted,
        marked_deleted: label.deleted,
        sets_access: ordinary && reference.sets_access,
        settings: reference
            .metadata
            .get(&SETTINGS)
            .map(|settings| settings.0)
            .filter(|_| ordinary && access.can_view()),
    }
}

fn inspect(
    types: &dyn BlockTypes,
    block: &BlockInfo,
    name: &str,
    access: AccessLevel,
    is_reference: bool,
    described: impl Fn(Uuid) -> String,
) -> Inspection {
    let named = match block.named_by_hand() {
        true => name.to_owned(),
        false => format!("{name} (automatic)"),
    };
    let parent = match block.parent {
        Some(BlockParent::Root) => "Root".to_owned(),
        Some(BlockParent::Detached) => "Recently Deleted".to_owned(),
        Some(BlockParent::Block(id)) if block.deleted => {
            format!("{}, in Recently Deleted", described(id))
        }
        Some(BlockParent::Block(id)) => described(id),
        None => "Unknown".to_owned(),
    };
    let author = match block.author.filter(|author| !author.is_nil()) {
        Some(author) => author.to_string(),
        None => "Unknown".to_owned(),
    };
    let generated = block.artifact().map_or_else(
        || "No".to_owned(),
        |artifact| format!("From {}", type_name(types, artifact.source_type)),
    );
    Inspection {
        name: named,
        id: block.id.to_string(),
        block_type: type_name(types, block.block_type),
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

fn type_name(types: &dyn BlockTypes, block_type: Uuid) -> String {
    match types.display_name(block_type) {
        Some(name) => format!("{name} ({block_type})"),
        None => block_type.to_string(),
    }
}

pub(crate) fn can_move_out_of(
    client: &Blocks,
    types: &dyn BlockTypes,
    block_types: &HashMap<Uuid, Uuid>,
    source: BlockParent,
    child: Uuid,
    is_reference: bool,
) -> bool {
    let can_delete = match source {
        BlockParent::Root | BlockParent::Detached => true,
        BlockParent::Block(id) => {
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
    let held: Ready<(Uuid, Uuid)> = container
        .and_then(|id| Some((id, *block_types.get(&id)?)))
        .ok_or(Pending::Loading);
    match held {
        Ok((_, block_type)) if !types.child_edits(block_type).replace => {
            Err("This container doesn't support replacing a reference")
        }
        Ok((container, _)) if !client.access(container).can_edit() => {
            Err("You don't have permission to edit this container")
        }
        Ok(_) => Ok(()),
        Err(_) => Err("Loading…"),
    }
}

pub(crate) fn access_name(access: AccessLevel) -> &'static str {
    match access {
        AccessLevel::Manage => "Can manage",
        AccessLevel::Edit => "Can edit",
        AccessLevel::View => "Can view",
        AccessLevel::KnowExists => "Can see that it exists",
        AccessLevel::None => "None",
    }
}

pub(crate) fn access_hint(access: AccessLevel) -> &'static str {
    match access {
        AccessLevel::Manage | AccessLevel::Edit => "",
        AccessLevel::View => "You can view this block, but not change it.",
        AccessLevel::KnowExists | AccessLevel::None => {
            "You can see that this block exists, but not open it."
        }
    }
}

pub(crate) fn access_marker(access: AccessLevel) -> Option<Glyph> {
    match access {
        AccessLevel::Manage | AccessLevel::Edit => None,
        AccessLevel::View => Some(block_editor_beui::beui::icons::ICON_EYE),
        AccessLevel::KnowExists | AccessLevel::None => {
            Some(block_editor_beui::beui::icons::ICON_LOCK)
        }
    }
}

#[cfg(test)]
mod tests;
