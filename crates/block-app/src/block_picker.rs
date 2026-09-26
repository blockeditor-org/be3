use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use be_block::{BlockContent, BlockMetadata, CanvasContent};
use be_graph::BlockParent;
use beui::Rect;
use uuid::Uuid;

use crate::{
    editors::{BlockLabel, CreationStep, EditorAccess, PendingCreation, PluginEditor},
    host::{SurfaceOutput, Ui},
    slide_templates::SlideTemplate,
    surfaces::{self, SurfaceId},
};

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub(crate) enum PickerTab {
    Add,
    Templates,
    LinkExisting,
}

#[derive(Clone, Debug)]
pub(crate) enum PickerAction {
    Tab(PickerTab),
    Search(String),
    Add(Uuid),
    Template(usize),
    Link(Uuid),
    Close,
    Create,
    CancelCreation,
    DismissError,
}

#[derive(Clone, Debug)]
pub(crate) struct PickerCommand {
    pub(crate) picker: Uuid,
    pub(crate) action: PickerAction,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Tile {
    pub(crate) key: String,
    pub(crate) label: String,
    pub(crate) icon: String,
    pub(crate) important: bool,
    pub(crate) action: TileAction,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TileAction {
    Add(Uuid),
    Template(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LinkRow {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) automatic: bool,
    pub(crate) icon: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ChooseView {
    pub(crate) tab: PickerTab,
    pub(crate) search: String,
    pub(crate) tiles: Vec<Tile>,
    pub(crate) links: Vec<LinkRow>,
    pub(crate) empty: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CreateView {
    pub(crate) title: String,
    pub(crate) working: bool,
    pub(crate) ready: bool,
    pub(crate) dialog: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PickerView {
    pub(crate) id: Uuid,
    pub(crate) depth: usize,
    pub(crate) choose: Option<ChooseView>,
    pub(crate) create: Option<CreateView>,
    pub(crate) error: Option<String>,
}

#[derive(Default)]
struct Board {
    inbox: Vec<PickerCommand>,
    views: Vec<PickerView>,
}

thread_local! {
    static BOARD: RefCell<Board> = RefCell::new(Board::default());
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}

pub(crate) fn deliver(command: PickerCommand) {
    BOARD.with(|board| board.borrow_mut().inbox.push(command));
}

pub(crate) fn views() -> Vec<PickerView> {
    let mut views = BOARD.with(|board| std::mem::take(&mut board.borrow_mut().views));
    views.sort_by_key(|view| view.depth);
    views
}

fn take_actions(picker: Uuid) -> Vec<PickerAction> {
    BOARD.with(|board| {
        let mut board = board.borrow_mut();
        let (mine, rest) = std::mem::take(&mut board.inbox)
            .into_iter()
            .partition::<Vec<_>, _>(|command| command.picker == picker);
        board.inbox = rest;
        mine.into_iter().map(|command| command.action).collect()
    })
}

fn publish(view: PickerView) {
    if view.choose.is_none() && view.create.is_none() && view.error.is_none() {
        return;
    }
    BOARD.with(|board| {
        let mut board = board.borrow_mut();
        if board.views.iter().all(|shown| shown.id != view.id) {
            board.views.push(view);
        }
    });
}

pub(crate) fn creation_surface(depth: usize) -> SurfaceId {
    match depth {
        0 => SurfaceId::Creation,
        _ => SurfaceId::NestedCreation,
    }
}

struct PendingBlock {
    block_type: Uuid,
    creation: Box<dyn PendingCreation>,
    creating: bool,
    step: Option<CreationStep>,
}

pub struct BlockPickerResult {
    pub id: Uuid,
    pub block_type: Uuid,
    pub linked: bool,
}

pub struct BlockPicker {
    id: Uuid,
    depth: usize,
    open: bool,
    tab: PickerTab,
    search: String,
    excluded: HashSet<Uuid>,
    allowed: HashSet<Uuid>,
    pending_block: Option<PendingBlock>,
    error: Option<String>,
}

impl Default for BlockPicker {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            depth: 0,
            open: false,
            tab: PickerTab::Add,
            search: String::new(),
            excluded: HashSet::new(),
            allowed: HashSet::new(),
            pending_block: None,
            error: None,
        }
    }
}

impl BlockPicker {
    pub fn open_for_types(
        &mut self,
        excluded: impl IntoIterator<Item = Uuid>,
        allowed: impl IntoIterator<Item = Uuid>,
    ) {
        self.allowed = allowed.into_iter().collect();
        self.open_on_tab(excluded, PickerTab::Add);
    }

    pub fn open_templates_for_types(
        &mut self,
        excluded: impl IntoIterator<Item = Uuid>,
        allowed: impl IntoIterator<Item = Uuid>,
    ) {
        self.allowed = allowed.into_iter().collect();
        self.open_on_tab(excluded, PickerTab::Templates);
    }

    fn open_on_tab(&mut self, excluded: impl IntoIterator<Item = Uuid>, tab: PickerTab) {
        self.open = true;
        self.tab = tab;
        self.search.clear();
        self.excluded = excluded.into_iter().collect();
    }

    pub fn is_open(&self) -> bool {
        self.open || self.pending_block.is_some() || self.error.is_some()
    }

    pub fn handle(
        &mut self,
        editors: &mut EditorAccess<'_>,
        created_parent: BlockParent,
    ) -> Option<BlockPickerResult> {
        self.depth = DEPTH.get();
        let mut result = None;
        for action in take_actions(self.id) {
            match action {
                PickerAction::Tab(tab) => self.tab = tab,
                PickerAction::Search(search) => self.search = search,
                PickerAction::Close => self.open = false,
                PickerAction::Add(block_type) => {
                    self.open = false;
                    result = result.or_else(|| self.create_registered_block(editors, block_type));
                }
                PickerAction::Template(index) => {
                    self.open = false;
                    if let Some(template) = SlideTemplate::ALL.get(index).copied() {
                        result = Some(Self::finish_template(editors, template, created_parent));
                    }
                }
                PickerAction::Link(id) => {
                    self.open = false;
                    if let Some(block) = crate::be::node(id) {
                        editors.ensure(block.id, block.content_type);
                        result = Some(BlockPickerResult {
                            id: block.id,
                            block_type: block.content_type,
                            linked: true,
                        });
                    }
                }
                PickerAction::Create => {
                    if let Some(pending) = &mut self.pending_block {
                        pending.creating = true;
                    }
                }
                PickerAction::CancelCreation => self.pending_block = None,
                PickerAction::DismissError => self.error = None,
            }
        }
        if result.is_none() {
            result = self.run_creation(editors, created_parent);
        }
        publish(self.view(editors));
        result
    }

    fn view(&self, editors: &EditorAccess<'_>) -> PickerView {
        let registry = editors.registry();
        PickerView {
            id: self.id,
            depth: self.depth,
            choose: self.open.then(|| {
                let mut tiles: Vec<Tile> = match self.tab {
                    PickerTab::Add => registry
                        .new_block_actions()
                        .iter()
                        .filter(|(_, block_type, _)| {
                            self.allowed.is_empty() || self.allowed.contains(block_type)
                        })
                        .map(|&(label, block_type, important)| Tile {
                            key: block_type.to_string(),
                            label: label.to_owned(),
                            icon: registry.icon(block_type).unwrap_or_default().to_owned(),
                            important,
                            action: TileAction::Add(block_type),
                        })
                        .collect(),
                    PickerTab::Templates => SlideTemplate::ALL
                        .iter()
                        .enumerate()
                        .map(|(index, template)| Tile {
                            key: format!("template-{index}"),
                            label: template.label().to_owned(),
                            icon: template.icon().to_owned(),
                            important: true,
                            action: TileAction::Template(index),
                        })
                        .collect(),
                    PickerTab::LinkExisting => Vec::new(),
                };
                tiles.sort_by_key(|tile| !tile.important);
                let query = self.search.trim().to_lowercase();
                let links: Vec<LinkRow> = match self.tab {
                    PickerTab::LinkExisting => crate::be::nodes()
                        .into_iter()
                        .filter(|block| block.access.can_view())
                        .filter(|block| block.parent != BlockParent::Detached)
                        .filter(|block| !self.excluded.contains(&block.id))
                        .filter(|block| {
                            self.allowed.is_empty() || self.allowed.contains(&block.content_type)
                        })
                        .map(|block| (BlockLabel::for_node(registry, &block), block.id))
                        .filter(|(label, id)| {
                            query.is_empty()
                                || label.name.to_lowercase().contains(&query)
                                || id.to_string().contains(&query)
                        })
                        .map(|(label, id)| LinkRow {
                            id,
                            name: label.name,
                            automatic: label.automatic,
                            icon: label.icon.unwrap_or_default().to_owned(),
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                ChooseView {
                    tab: self.tab,
                    search: self.search.clone(),
                    tiles,
                    links,
                    empty: match query.is_empty() {
                        true => "No blocks are available to link.".to_owned(),
                        false => "No matching blocks.".to_owned(),
                    },
                }
            }),
            create: self.pending_block.as_ref().map(|pending| {
                let title = registry.display_name(pending.block_type).unwrap_or("block");
                let (working, ready) = match pending.step {
                    Some(CreationStep::Working) | None => (true, false),
                    Some(CreationStep::Options(ready)) => (false, ready),
                };
                CreateView {
                    title: title.to_owned(),
                    working,
                    ready: ready && !pending.creating,
                    dialog: pending.creation.height().is_some(),
                }
            }),
            error: self.error.clone(),
        }
    }

    fn create_registered_block(
        &mut self,
        editors: &mut EditorAccess<'_>,
        block_type: Uuid,
    ) -> Option<BlockPickerResult> {
        match editors.registry().create(block_type) {
            Some(creation) => {
                self.pending_block = Some(PendingBlock {
                    block_type,
                    creation,
                    creating: false,
                    step: None,
                });
                None
            }
            None => {
                self.error = Some(format!("Could not create block type {block_type}"));
                None
            }
        }
    }

    fn run_creation(
        &mut self,
        editors: &mut EditorAccess<'_>,
        parent: BlockParent,
    ) -> Option<BlockPickerResult> {
        let mut pending = self.pending_block.take()?;
        let surface = creation_surface(self.depth);
        surfaces::set_height(surface, pending.creation.height());
        DEPTH.set(self.depth + 1);
        let step =
            surfaces::with(surface, |ui| pending.creation.ui(ui, editors)).unwrap_or_else(|| {
                let mut scratch = SurfaceOutput::default();
                let mut ui = Ui::new(&mut scratch, Rect::ZERO, Rect::ZERO, 1);
                pending.creation.ui(&mut ui, editors)
            });
        DEPTH.set(self.depth);
        if matches!(step, CreationStep::Working) {
            pending.creating = true;
        }
        pending.step = Some(step);
        if !pending.creating {
            self.pending_block = Some(pending);
            return None;
        }
        match pending.creation.create() {
            Ok(Some(editor)) => {
                surfaces::set_height(surface, None);
                Some(Self::finish_creation(
                    editors,
                    editor,
                    pending.block_type,
                    parent,
                ))
            }
            Ok(None) => {
                self.pending_block = Some(pending);
                None
            }
            Err(error) => {
                surfaces::set_height(surface, None);
                self.error = Some(error);
                None
            }
        }
    }

    fn finish_template(
        editors: &mut EditorAccess<'_>,
        template: SlideTemplate,
        parent: BlockParent,
    ) -> BlockPickerResult {
        let canvas = crate::slide_templates::build_template_canvas(template);
        let id = Uuid::new_v4();
        crate::be::create(
            id,
            CanvasContent::CONTENT_TYPE,
            parent,
            BlockMetadata::default(),
            Some(canvas.encode()),
        );
        editors.ensure(id, CanvasContent::CONTENT_TYPE);
        BlockPickerResult {
            id,
            block_type: CanvasContent::CONTENT_TYPE,
            linked: false,
        }
    }

    fn finish_creation(
        editors: &mut EditorAccess<'_>,
        editor: PluginEditor,
        block_type: Uuid,
        parent: BlockParent,
    ) -> BlockPickerResult {
        let id = editor.id();
        crate::be::set_parent(id, parent);
        editors.insert(editor);
        BlockPickerResult {
            id,
            block_type,
            linked: false,
        }
    }
}

#[cfg(test)]
mod tests;
