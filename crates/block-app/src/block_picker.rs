use std::cell::RefCell;
use std::collections::HashSet;

use be_graph::BlockParent;
use beui::Rect;
use block_plugin_api::TemplateCategory;
use uuid::Uuid;

use crate::{
    editors::{BlockLabel, CreationStep, EditorAccess, PendingCreation, plugin::CreationTarget},
    host::{SurfaceOutput, Ui},
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
    Make(TileAction),
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
    pub(crate) action: TileAction,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TileAction {
    pub(crate) editor: Uuid,
    pub(crate) template: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TileSection {
    pub(crate) key: String,
    pub(crate) title: String,
    pub(crate) icon: Option<String>,
    pub(crate) tiles: Vec<Tile>,
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
    pub(crate) sections: Vec<TileSection>,
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
    pub(crate) choose: Option<ChooseView>,
    pub(crate) create: Option<CreateView>,
    pub(crate) error: Option<String>,
}

#[derive(Default)]
struct Board {
    inbox: Vec<PickerCommand>,
    view: Option<PickerView>,
}

thread_local! {
    static BOARD: RefCell<Board> = RefCell::new(Board::default());
}

pub(crate) fn deliver(command: PickerCommand) {
    BOARD.with(|board| board.borrow_mut().inbox.push(command));
}

pub(crate) fn view() -> Option<PickerView> {
    BOARD.with(|board| board.borrow_mut().view.take())
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
    BOARD.with(|board| {
        let mut board = board.borrow_mut();
        if board.view.is_none() {
            board.view = Some(view);
        }
    });
}

struct PendingBlock {
    target: CreationTarget,
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
        let mut result = None;
        for action in take_actions(self.id) {
            match action {
                PickerAction::Tab(tab) => self.tab = tab,
                PickerAction::Search(search) => self.search = search,
                PickerAction::Close => self.open = false,
                PickerAction::Make(tile) => {
                    self.open = false;
                    self.create_from_template(editors, tile);
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
            choose: self.open.then(|| {
                let sections = match self.tab {
                    PickerTab::Add => self.add_sections(editors),
                    PickerTab::Templates => self.template_sections(editors),
                    PickerTab::LinkExisting => Vec::new(),
                };
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
                    sections,
                    links,
                    empty: match query.is_empty() {
                        true => "No blocks are available to link.".to_owned(),
                        false => "No matching blocks.".to_owned(),
                    },
                }
            }),
            create: self.pending_block.as_ref().map(|pending| {
                let title = pending.target.name;
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

    fn offered(&self, editors: &EditorAccess<'_>) -> Vec<(Tile, TemplateCategory, Uuid)> {
        editors
            .registry()
            .templates()
            .iter()
            .filter(|entry| {
                self.allowed.is_empty() || self.allowed.contains(&entry.target.block_type)
            })
            .map(|entry| {
                let action = TileAction {
                    editor: entry.target.editor,
                    template: entry.target.template,
                };
                let tile = Tile {
                    key: format!("{}/{}", action.editor, action.template),
                    label: entry.target.name.to_owned(),
                    icon: entry.icon.to_owned(),
                    action,
                };
                (tile, entry.category, entry.target.editor)
            })
            .collect()
    }

    fn add_sections(&self, editors: &EditorAccess<'_>) -> Vec<TileSection> {
        let offered = self.offered(editors);
        [
            (TemplateCategory::Important, "Common"),
            (TemplateCategory::Regular, "More blocks"),
            (TemplateCategory::Debug, "Debug"),
        ]
        .into_iter()
        .map(|(category, title)| {
            let mut tiles: Vec<Tile> = offered
                .iter()
                .filter(|(_, offered, _)| *offered == category)
                .map(|(tile, _, _)| tile.clone())
                .collect();
            tiles.sort_by(|a, b| a.label.cmp(&b.label));
            TileSection {
                key: format!("{category:?}"),
                title: title.to_owned(),
                icon: None,
                tiles,
            }
        })
        .filter(|section| !section.tiles.is_empty())
        .collect()
    }

    fn template_sections(&self, editors: &EditorAccess<'_>) -> Vec<TileSection> {
        let registry = editors.registry();
        let mut sections: Vec<TileSection> = Vec::new();
        for (tile, category, editor) in self.offered(editors) {
            if category != TemplateCategory::Template {
                continue;
            }
            let key = editor.to_string();
            match sections.iter_mut().find(|section| section.key == key) {
                Some(section) => section.tiles.push(tile),
                None => sections.push(TileSection {
                    key,
                    title: registry.display_name(editor).unwrap_or_default().to_owned(),
                    icon: registry.icon(editor).map(str::to_owned),
                    tiles: vec![tile],
                }),
            }
        }
        sections.sort_by(|a, b| a.title.cmp(&b.title));
        sections
    }

    fn create_from_template(&mut self, editors: &mut EditorAccess<'_>, tile: TileAction) {
        match editors.registry().create(tile.editor, tile.template) {
            Some((target, creation)) => {
                self.pending_block = Some(PendingBlock {
                    target,
                    creation,
                    creating: false,
                    step: None,
                });
            }
            None => {
                self.error = Some(format!(
                    "Could not create template {} of {}",
                    tile.template, tile.editor
                ));
            }
        }
    }

    fn run_creation(
        &mut self,
        editors: &mut EditorAccess<'_>,
        parent: BlockParent,
    ) -> Option<BlockPickerResult> {
        let mut pending = self.pending_block.take()?;
        surfaces::set_height(SurfaceId::Creation, pending.creation.height());
        let step = surfaces::with(SurfaceId::Creation, |ui| pending.creation.ui(ui, editors))
            .unwrap_or_else(|| {
                let mut scratch = SurfaceOutput::default();
                let mut ui = Ui::new(&mut scratch, Rect::ZERO, Rect::ZERO, 1);
                pending.creation.ui(&mut ui, editors)
            });
        if matches!(step, CreationStep::Working) {
            pending.creating = true;
        }
        pending.step = Some(step);
        if !pending.creating {
            self.pending_block = Some(pending);
            return None;
        }
        match pending.creation.create() {
            Ok(Some(id)) => {
                surfaces::set_height(SurfaceId::Creation, None);
                Some(Self::finish_creation(
                    editors,
                    id,
                    pending.target.block_type,
                    parent,
                ))
            }
            Ok(None) => {
                self.pending_block = Some(pending);
                None
            }
            Err(error) => {
                surfaces::set_height(SurfaceId::Creation, None);
                self.error = Some(error);
                None
            }
        }
    }

    fn finish_creation(
        editors: &mut EditorAccess<'_>,
        id: Uuid,
        declared: Uuid,
        parent: BlockParent,
    ) -> BlockPickerResult {
        let block_type = crate::be::node(id).map_or(declared, |node| node.content_type);
        crate::be::set_parent(id, parent);
        editors.ensure(id, block_type);
        BlockPickerResult {
            id,
            block_type,
            linked: false,
        }
    }
}
