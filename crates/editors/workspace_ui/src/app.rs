mod block_data;
mod chrome;
mod menu;
mod status;

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use block::{BlockAccess, BlockParent, BlockReference, BlockReferenceList};
use block_client::{
    BlockClient, BlockHandleAccess, ReferenceList, blocks::file_tree::FileTree,
    root_settings::RootSetting,
};
use block_editor_plugin::{
    AccessLevel, BlockFilter, BlockPicker, EditorHost, FocusedBlock,
    block_ui::{BlockCatalog, BlockLabel},
    egui,
};
use egui_dock::{DockArea, DockState, TabViewer, widgets::tab_viewer::OnCloseResponse};
use uuid::Uuid;

const COMPACT_FILES_WIDTH: f32 = 700.0;
const MAX_OPENED_VIA_HOPS: usize = 64;
pub(crate) const NO_EDIT_ACCESS: &str = "You do not have permission to change this block";

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TabItem {
    pub(crate) id: Uuid,
    pub(crate) block_type: Uuid,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct BlockTab {
    id: Uuid,
    history: Vec<TabItem>,
    index: usize,
}

impl BlockTab {
    fn new(item: TabItem) -> Self {
        Self {
            id: Uuid::new_v4(),
            history: vec![item],
            index: 0,
        }
    }

    fn current(&self) -> TabItem {
        self.history[self.index]
    }

    fn can_go_back(&self) -> bool {
        self.index > 0
    }

    fn can_go_forward(&self) -> bool {
        self.index + 1 < self.history.len()
    }

    fn navigate(&mut self, item: TabItem) {
        if self.current().id == item.id {
            return;
        }
        self.history.truncate(self.index + 1);
        self.history.push(item);
        self.index += 1;
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum DockTab {
    Files,
    Empty,
    Block(BlockTab),
}

#[derive(Clone, Copy)]
pub(crate) enum Navigation {
    Back,
    Forward,
    Open(TabItem),
}

pub(crate) struct Frame<'a> {
    pub(crate) host: &'a EditorHost,
    pub(crate) client: &'a BlockClient,
    pub(crate) types: &'a BlockCatalog,
}

impl Frame<'_> {
    pub(crate) fn can_edit(&self, id: Uuid) -> bool {
        self.client.block_access(id).can_edit()
    }

    pub(crate) fn ceiling(&self, id: Uuid) -> BlockAccess {
        let access = self.client.block_access(id);
        match self.client.is_dynamic_artifact(id) {
            true => access.min(BlockAccess::View),
            false => access,
        }
    }

    pub(crate) fn label(&self, id: Uuid, block_type: Uuid) -> BlockLabel {
        self.client.cached_block(id).map_or_else(
            || BlockLabel::new(self.types, block_type, None),
            |cached| BlockLabel::for_cached(self.types, &cached),
        )
    }
}

struct State {
    host: EditorHost,
    client: Arc<BlockClient>,
    file_tree: RootSetting<FileTree>,
    parents: HashMap<Uuid, ReferenceList>,
    references: HashMap<Uuid, ReferenceList>,
    backrefs: HashMap<Uuid, ReferenceList>,
    parent_candidates: HashMap<Uuid, ReferenceList>,
    block_types: HashMap<Uuid, Uuid>,
    handles: HashMap<Uuid, Box<dyn BlockHandleAccess>>,
    opened_via: HashMap<Uuid, Uuid>,
    picker: BlockPicker,
    picker_target: Option<Uuid>,
    picker_error: Option<String>,
    simulated: HashMap<Uuid, AccessLevel>,
    debug_blocks: HashSet<Uuid>,
    active: Option<Uuid>,
    files_compact: bool,
}

impl State {
    fn watch(&mut self, item: TabItem) {
        self.block_types.insert(item.id, item.block_type);
        self.parents
            .entry(item.id)
            .or_insert_with(|| self.client.watch_parents(item.id));
        self.references.entry(item.id).or_insert_with(|| {
            self.client
                .watch_references(BlockReferenceList::References(item.id))
        });
        self.backrefs.entry(item.id).or_insert_with(|| {
            self.client
                .watch_references(BlockReferenceList::Backrefs(item.id))
        });
    }

    pub(crate) fn handle(&mut self, item: TabItem) -> Option<&dyn BlockHandleAccess> {
        if !self.handles.contains_key(&item.id) {
            let handle = block_client::blocks::open(&self.client, item.id, item.block_type)?;
            self.handles.insert(item.id, handle);
        }
        self.handles.get(&item.id).map(Box::as_ref)
    }

    pub(crate) fn open_picker(&mut self, parent: Uuid) {
        self.picker_error = None;
        self.picker_target = Some(parent);
        self.picker.open(
            &self.host,
            BlockFilter {
                name: "Block".to_owned(),
                block_types: Vec::new(),
                excluded: vec![parent.into_bytes()],
                templates: false,
            },
        );
    }

    fn poll_picker(&mut self) {
        let Some(result) = self.picker.poll(&self.host) else {
            return;
        };
        let Some(parent) = self.picker_target.take() else {
            return;
        };
        match result {
            Ok(picked) => {
                self.host
                    .place_block(picked.id, picked.block_type, parent, picked.linked)
            }
            Err(error) => self.picker_error = Some(error),
        }
    }

    fn forget(&mut self, id: Uuid) {
        self.handles.remove(&id);
        self.parents.remove(&id);
        self.references.remove(&id);
        self.backrefs.remove(&id);
        self.parent_candidates.remove(&id);
        self.simulated.remove(&id);
        self.debug_blocks.remove(&id);
        if self.active == Some(id) {
            self.active = None;
        }
        self.host.close_editor(id);
    }

    pub(crate) fn record_reference_types(&mut self, reference: &BlockReference) {
        self.block_types.insert(reference.id, reference.block_type);
        if let BlockParent::Uuid(parent) = reference.parent
            && let Some(parent) = self.client.cached_block(parent)
        {
            self.block_types.insert(parent.id, parent.block_type);
        }
    }

    fn access(&self, frame: &Frame<'_>, id: Uuid) -> BlockAccess {
        let ceiling = frame.ceiling(id);
        match self.simulated.get(&id) {
            Some(AccessLevel::None) => BlockAccess::None.min(ceiling),
            Some(AccessLevel::KnowExists) => BlockAccess::KnowExists.min(ceiling),
            Some(AccessLevel::View) => BlockAccess::View.min(ceiling),
            Some(AccessLevel::Edit) | None => ceiling,
        }
    }

    fn via_chain(&self, id: Uuid) -> Vec<Uuid> {
        let mut via = Vec::new();
        let mut visited = HashSet::new();
        visited.insert(id);
        let mut current = id;
        while let Some(&container) = self.opened_via.get(&current) {
            if via.len() >= MAX_OPENED_VIA_HOPS || !visited.insert(container) {
                break;
            }
            via.push(container);
            current = container;
        }
        via
    }
}

#[derive(Default)]
pub struct WorkspaceUiApp {
    state: Option<State>,
    dock: Option<DockState<DockTab>>,
}

fn default_dock() -> DockState<DockTab> {
    let mut dock = DockState::new(vec![DockTab::Files]);
    let files = dock
        .find_tab(&DockTab::Files)
        .expect("a new dock state contains Files");
    dock[files.surface].split_right(files.node, 0.22, vec![DockTab::Empty]);
    dock
}

fn ensure_empty_workspace(dock: &mut DockState<DockTab>) {
    let has_editor = dock
        .iter_all_tabs()
        .any(|(_, tab)| matches!(tab, DockTab::Block(_)));
    if has_editor || dock.find_tab(&DockTab::Empty).is_some() {
        return;
    }
    if let Some(files) = dock.find_tab(&DockTab::Files) {
        dock[files.surface].split_right(files.node, 0.22, vec![DockTab::Empty]);
    }
}

fn set_files_compact(dock: &mut DockState<DockTab>, compact: bool) {
    let active = dock
        .find_active_focused()
        .map(|(_, tab)| tab.clone())
        .unwrap_or(DockTab::Files);
    let Some(files) = dock.find_tab(&DockTab::Files) else {
        return;
    };
    let target = dock
        .iter_all_tabs()
        .find_map(|(path, tab)| (*tab != DockTab::Files).then_some(path.node_path()));
    let Some(target) = target else {
        return;
    };
    dock.set_focused_node_and_surface(target);
    dock.remove_tab(files);
    if compact {
        dock.push_to_focused_leaf(DockTab::Files);
    } else if let Some(target) = dock.find_tab(&active).or_else(|| {
        dock.iter_all_tabs()
            .find_map(|(path, tab)| (*tab != DockTab::Files).then_some(path))
    }) {
        dock[target.surface].split_left(target.node, 0.78, vec![DockTab::Files]);
    }
    if let Some(path) = dock.find_tab(&active) {
        let _ = dock.set_active_tab(path);
        dock.set_focused_node_and_surface(path.node_path());
    }
}

impl WorkspaceUiApp {
    fn open(&mut self, item: TabItem, via: Option<Uuid>) {
        let (Some(state), Some(dock)) = (self.state.as_mut(), self.dock.as_mut()) else {
            return;
        };
        match via {
            Some(container) => state.opened_via.insert(item.id, container),
            None => state.opened_via.remove(&item.id),
        };
        state.watch(item);
        let existing = dock.iter_all_tabs().find_map(|(path, tab)| {
            matches!(tab, DockTab::Block(tab) if tab.current().id == item.id).then_some(path)
        });
        if let Some(path) = existing {
            let _ = dock.set_active_tab(path);
            dock.set_focused_node_and_surface(path.node_path());
            state.active = Some(item.id);
            return;
        }
        let tab = DockTab::Block(BlockTab::new(item));
        let existing_block = dock
            .iter_all_tabs()
            .find_map(|(path, tab)| matches!(tab, DockTab::Block(_)).then_some(path));
        if let Some(empty) = dock.find_tab(&DockTab::Empty) {
            let leaf = dock
                .leaf_mut(empty.node_path())
                .expect("the blank workspace is a dock leaf");
            leaf.tabs_mut()[empty.tab.0] = tab;
            let _ = dock.set_active_tab(empty);
            dock.set_focused_node_and_surface(empty.node_path());
        } else if let Some(path) = existing_block {
            dock.set_focused_node_and_surface(path.node_path());
            dock.push_to_focused_leaf(tab);
        } else if let Some(files) = dock.find_tab(&DockTab::Files) {
            dock[files.surface].split_right(files.node, 0.22, vec![tab]);
        } else {
            dock.push_to_focused_leaf(tab);
        }
        state.active = Some(item.id);
    }

    fn navigate(&mut self, tab_id: Uuid, navigation: Navigation) {
        let (Some(state), Some(dock)) = (self.state.as_mut(), self.dock.as_mut()) else {
            return;
        };
        if let Navigation::Open(item) = navigation {
            state.watch(item);
        }
        let Some(tab) = dock.iter_all_tabs_mut().find_map(|(_, tab)| match tab {
            DockTab::Block(tab) if tab.id == tab_id => Some(tab),
            DockTab::Files | DockTab::Empty | DockTab::Block(_) => None,
        }) else {
            return;
        };
        match navigation {
            Navigation::Back if tab.can_go_back() => tab.index -= 1,
            Navigation::Forward if tab.can_go_forward() => tab.index += 1,
            Navigation::Open(item) => tab.navigate(item),
            Navigation::Back | Navigation::Forward => {}
        }
        let current = tab.current();
        state.watch(current);
        state.active = Some(current.id);
    }

    fn close(&mut self, tab_id: Uuid) {
        let (Some(state), Some(dock)) = (self.state.as_mut(), self.dock.as_mut()) else {
            return;
        };
        let Some((path, blocks)) = dock.iter_all_tabs().find_map(|(path, tab)| match tab {
            DockTab::Block(tab) if tab.id == tab_id => Some((
                path,
                tab.history.iter().map(|item| item.id).collect::<Vec<_>>(),
            )),
            DockTab::Files | DockTab::Empty | DockTab::Block(_) => None,
        }) else {
            return;
        };
        let editors = dock
            .iter_all_tabs()
            .filter(|(_, tab)| matches!(tab, DockTab::Block(_)))
            .count();
        if editors == 1 {
            let leaf = dock
                .leaf_mut(path.node_path())
                .expect("an editor tab is in a dock leaf");
            leaf.tabs_mut()[path.tab.0] = DockTab::Empty;
        } else {
            dock.remove_tab(path);
        }
        let still_open: HashSet<Uuid> = dock
            .iter_all_tabs()
            .filter_map(|(_, tab)| match tab {
                DockTab::Block(tab) => Some(tab.history.iter().map(|item| item.id)),
                DockTab::Files | DockTab::Empty => None,
            })
            .flatten()
            .collect();
        for id in blocks {
            if !still_open.contains(&id) {
                state.forget(id);
            }
        }
    }
}

impl WorkspaceUiApp {
    pub fn open_blocks(&self) -> Vec<Uuid> {
        self.dock
            .iter()
            .flat_map(DockState::iter_all_tabs)
            .filter_map(|(_, tab)| match tab {
                DockTab::Block(tab) => Some(tab.current().id),
                DockTab::Files | DockTab::Empty => None,
            })
            .collect()
    }

    fn active_tab(&mut self) -> Option<Uuid> {
        let dock = self.dock.as_mut()?;
        let focused = dock.find_active_focused().and_then(|(_, tab)| match tab {
            DockTab::Block(tab) => Some(tab.id),
            DockTab::Files | DockTab::Empty => None,
        });
        focused.or_else(|| {
            dock.iter_all_tabs().find_map(|(_, tab)| match tab {
                DockTab::Block(tab) => Some(tab.id),
                DockTab::Files | DockTab::Empty => None,
            })
        })
    }

    pub fn close_active(&mut self) {
        if let Some(tab) = self.active_tab() {
            self.close(tab);
        }
    }

    pub fn navigate_active(&mut self, id: Uuid, block_type: Uuid) {
        if let Some(tab) = self.active_tab() {
            self.navigate(tab, Navigation::Open(TabItem { id, block_type }));
        }
    }
}

impl block_editor_plugin::App for WorkspaceUiApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, _block_id: Uuid) {
        let file_tree = RootSetting::new(&client);
        self.state = Some(State {
            host,
            client,
            file_tree,
            parents: HashMap::new(),
            references: HashMap::new(),
            backrefs: HashMap::new(),
            parent_candidates: HashMap::new(),
            block_types: HashMap::new(),
            handles: HashMap::new(),
            opened_via: HashMap::new(),
            picker: BlockPicker::default(),
            picker_target: None,
            picker_error: None,
            simulated: HashMap::new(),
            debug_blocks: HashSet::new(),
            active: None,
            files_compact: false,
        });
        self.dock = Some(default_dock());
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let Some((host, client)) = self
            .state
            .as_ref()
            .map(|state| (state.host.clone(), Arc::clone(&state.client)))
        else {
            return;
        };
        for request in host.take_show_requests() {
            self.open(
                TabItem {
                    id: request.block_id,
                    block_type: request.block_type,
                },
                request.via,
            );
        }
        if let Some(state) = self.state.as_mut() {
            state.poll_picker();
        }
        let compact = ui.available_width() < COMPACT_FILES_WIDTH;
        if let (Some(state), Some(dock)) = (self.state.as_mut(), self.dock.as_mut())
            && compact != state.files_compact
        {
            set_files_compact(dock, compact);
            state.files_compact = compact;
        }
        let types = host.block_types();
        let frame = Frame {
            host: &host,
            client: &client,
            types: types.as_ref(),
        };
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if let Some(error) = state.picker_error.clone() {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        let Some(mut dock) = self.dock.take() else {
            return;
        };
        let mut viewer = Viewer {
            state,
            frame: &frame,
            actions: Vec::new(),
        };
        DockArea::new(&mut dock).show_inside(ui, &mut viewer);
        let actions = std::mem::take(&mut viewer.actions);
        let active = dock
            .find_active_focused()
            .and_then(|(_, tab)| match tab {
                DockTab::Files | DockTab::Empty => None,
                DockTab::Block(tab) => Some(tab.current().id),
            })
            .or(state.active);
        state.active = active;
        let watched: Vec<Uuid> = dock
            .iter_all_tabs()
            .filter_map(|(_, tab)| match tab {
                DockTab::Block(tab) => Some(tab.current().id),
                DockTab::Files | DockTab::Empty => None,
            })
            .filter(|id| client.is_dynamic_artifact(*id))
            .collect();
        self.dock = Some(dock);
        host.watch_artifacts(watched);
        for action in actions {
            match action {
                Action::Navigate(tab_id, navigation) => self.navigate(tab_id, navigation),
                Action::Close(tab_id) => self.close(tab_id),
            }
        }
        if let Some(state) = self.state.as_mut() {
            if let Some(dock) = self.dock.as_mut() {
                ensure_empty_workspace(dock);
            }
            let focused = state.active.and_then(|id| {
                let block_type = state.block_types.get(&id).copied()?;
                Some((id, block_type))
            });
            state.host.report_focus(FocusedBlock {
                block_id: focused.map(|(id, _)| id),
                block_type: focused.map_or_else(Uuid::nil, |(_, block_type)| block_type),
                via: focused.map_or_else(Vec::new, |(id, _)| state.via_chain(id)),
            });
        }
    }
}

enum Action {
    Navigate(Uuid, Navigation),
    Close(Uuid),
}

struct Viewer<'a, 'b> {
    state: &'a mut State,
    frame: &'a Frame<'b>,
    actions: Vec<Action>,
}

impl TabViewer for Viewer<'_, '_> {
    type Tab = DockTab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab {
            DockTab::Files => "Files".into(),
            DockTab::Empty => "Workspace".into(),
            DockTab::Block(tab) => {
                let item = tab.current();
                self.frame
                    .label(item.id, item.block_type)
                    .rich_text()
                    .into()
            }
        }
    }

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        match tab {
            DockTab::Files => egui::Id::new("files-tab"),
            DockTab::Empty => egui::Id::new("empty-tab"),
            DockTab::Block(tab) => egui::Id::new(tab.id),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            DockTab::Files => {
                let file_tree = self
                    .state
                    .file_tree
                    .find(&self.state.client, self.state.host.client_id())
                    .map(block_client::BlockHandle::id);
                let Some(id) = file_tree else {
                    ui.centered_and_justified(|ui| {
                        ui.spinner();
                    });
                    return;
                };
                let child = self
                    .state
                    .host
                    .child(ui, id, <FileTree as block::Block>::TYPE_ID);
                child.keep_active();
                child.own_frame();
                if !child.available() {
                    ui.put(
                        child.rect(),
                        egui::Label::new(child.error().unwrap_or("Files are loading\u{2026}")),
                    );
                }
            }
            DockTab::Empty => {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("No file open");
                        ui.weak("Open or create a file from Files to get started.");
                    });
                });
            }
            DockTab::Block(tab) => {
                let current = tab.current();
                self.state.watch(current);
                let tab_id = tab.id;
                let mut navigation = None;
                egui::Panel::bottom(egui::Id::new(("block-statusbar", tab_id)))
                    .resizable(false)
                    .show_inside(ui, |ui| {
                        navigation = status::show(ui, self.state, self.frame, current.id);
                    });
                let outcome = chrome::show(
                    ui,
                    self.state,
                    self.frame,
                    current,
                    tab.can_go_back(),
                    tab.can_go_forward(),
                );
                if let Some(item) = navigation {
                    self.actions
                        .push(Action::Navigate(tab_id, Navigation::Open(item)));
                }
                if let Some(navigation) = outcome.navigation {
                    self.actions.push(Action::Navigate(tab_id, navigation));
                }
            }
        }
    }

    fn on_close(&mut self, tab: &mut Self::Tab) -> OnCloseResponse {
        match tab {
            DockTab::Files | DockTab::Empty => OnCloseResponse::Ignore,
            DockTab::Block(tab) => {
                self.actions.push(Action::Close(tab.id));
                OnCloseResponse::Ignore
            }
        }
    }

    fn is_closeable(&self, tab: &Self::Tab) -> bool {
        matches!(tab, DockTab::Block(_))
    }

    fn scroll_bars(&self, _tab: &Self::Tab) -> [bool; 2] {
        [false, false]
    }
}
