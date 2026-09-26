use block_editor_plugin::be_block::{BlockContent, FileTreeContent};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::{
    Align, Frame, Func, ItemSize, List, Memo, NodeRef, ReadSignal, Spacer, WriteSignal, clone,
    component, create_effect, create_memo, create_signal, untrack, view,
};
use block_editor_plugin::beui::styled::{Caption, Heading, use_theme};
use block_editor_plugin::beui::unstyled::{
    Container, DockState, LeafId, Side, TabId, narrower_than,
};
use block_editor_plugin::block_ui::{BlockCatalog, BlockLabel};
use block_editor_plugin::root_settings::RootSetting;
use block_editor_plugin::{
    AccessLevel, BlockFilter, ChildBlock, ChildBlockHandle, ChildMode, ChildState, ChildTarget,
    Editor, EditorDock, EditorHost, FocusedBlock, PickedBlock, Pushed,
};
use block_editor_plugin::{BlockInfo, BlockList, BlockParent, BlockQuery, Blocks};
use uuid::Uuid;

use super::panel::BlockPanel;
use super::tab::TabItem;

pub(crate) const FILES: TabId = TabId::new(1);
pub(crate) const EMPTY: TabId = TabId::new(2);

const FIRST_BLOCK_TAB: u64 = 3;
const FILES_SHARE: f32 = 0.22;
const COMPACT_FILES_WIDTH: f32 = 700.0;
const MAX_OPENED_VIA_HOPS: usize = 64;
const PANEL_PADDING: f32 = 14.0;
const PANEL_SPACING: f32 = 6.0;

type Tabs = HashMap<TabId, TabItem>;

pub(crate) struct Workspace {
    editor: Editor,
    layout: ReadSignal<DockState>,
    set_layout: WriteSignal<DockState>,
    tabs: ReadSignal<Tabs>,
    set_tabs: WriteSignal<Tabs>,
    titles: ReadSignal<HashMap<TabId, String>>,
    set_titles: WriteSignal<HashMap<TabId, String>>,
    simulated: ReadSignal<HashMap<Uuid, AccessLevel>>,
    set_simulated: WriteSignal<HashMap<Uuid, AccessLevel>>,
    debugged: ReadSignal<HashSet<Uuid>>,
    set_debugged: WriteSignal<HashSet<Uuid>>,
    error: ReadSignal<Option<String>>,
    set_error: WriteSignal<Option<String>>,
    files: ReadSignal<Option<Uuid>>,
    set_files: WriteSignal<Option<Uuid>>,
    file_tree: RefCell<RootSetting<FileTreeContent>>,
    handles: RefCell<HashMap<Uuid, BlockList>>,
    block_types: RefCell<HashMap<Uuid, Uuid>>,
    opened_via: RefCell<HashMap<Uuid, Uuid>>,
    routes: ReadSignal<u64>,
    set_routes: WriteSignal<u64>,
    next_tab: Cell<u64>,
    active: Cell<Option<Uuid>>,
}

impl Workspace {
    fn new(editor: Editor) -> Rc<Self> {
        let (layout, set_layout) = create_signal(starting_layout());
        let (tabs, set_tabs) = create_signal(Tabs::new());
        let (titles, set_titles) = create_signal(HashMap::new());
        let (simulated, set_simulated) = create_signal(HashMap::new());
        let (debugged, set_debugged) = create_signal(HashSet::new());
        let (error, set_error) = create_signal(None);
        let (files, set_files) = create_signal(None);
        let (routes, set_routes) = create_signal(0);
        let workspace = Rc::new(Self {
            editor,
            layout,
            set_layout,
            tabs,
            set_tabs,
            titles,
            set_titles,
            simulated,
            set_simulated,
            debugged,
            set_debugged,
            error,
            set_error,
            files,
            set_files,
            file_tree: RefCell::new(RootSetting::default()),
            handles: RefCell::new(HashMap::new()),
            block_types: RefCell::new(HashMap::new()),
            opened_via: RefCell::new(HashMap::new()),
            routes,
            set_routes,
            next_tab: Cell::new(FIRST_BLOCK_TAB),
            active: Cell::new(None),
        });
        let shows = workspace.editor.pushed(Pushed::Shows);
        let showing = Rc::downgrade(&workspace);
        create_effect(move || {
            shows.get();
            if let Some(workspace) = showing.upgrade() {
                untrack(|| workspace.show_requested());
            }
        });
        let finding = Rc::downgrade(&workspace);
        create_effect(move || {
            if let Some(workspace) = finding.upgrade() {
                workspace.find_files();
            }
        });
        let titling = Rc::downgrade(&workspace);
        create_effect(move || {
            if let Some(workspace) = titling.upgrade() {
                workspace.refresh_titles();
            }
        });
        let focusing = Rc::downgrade(&workspace);
        create_effect(move || {
            if let Some(workspace) = focusing.upgrade() {
                workspace.report_focus();
            }
        });
        let watching = Rc::downgrade(&workspace);
        create_effect(move || {
            if let Some(workspace) = watching.upgrade() {
                workspace.watch_artifacts();
            }
        });
        workspace
    }

    pub(crate) fn editor(&self) -> &Editor {
        &self.editor
    }

    pub(crate) fn host(&self) -> &EditorHost {
        self.editor.host()
    }

    pub(crate) fn blocks(&self) -> Blocks {
        self.editor.blocks()
    }

    pub(crate) fn info(&self, id: Uuid) -> Option<BlockInfo> {
        let mut handles = self.handles.borrow_mut();
        let list = handles
            .entry(id)
            .or_insert_with(|| self.blocks().watch(BlockQuery::Block(id)));
        list.read()
            .into_iter()
            .next()
            .or_else(|| self.blocks().info(id))
    }

    pub(crate) fn debug_data(&self, id: Uuid) -> Option<String> {
        let info = self.info(id)?;
        let parent = match info.parent {
            BlockParent::Root => "root".to_owned(),
            BlockParent::Detached => "detached".to_owned(),
            BlockParent::Block(parent) => parent.to_string(),
        };
        let data = serde_json::json!({
            "id": info.id.to_string(),
            "type": info.block_type.to_string(),
            "author": info.author.to_string(),
            "parent": parent,
            "name": info.name,
            "named_by_hand": info.named_by_hand,
            "references": info.references.iter().map(Uuid::to_string).collect::<Vec<_>>(),
            "access": info.access.label(),
            "artifact_source": info.artifact.as_ref().map(|artifact| artifact.source_type.to_string()),
        });
        serde_json::to_string(&data).ok()
    }

    pub(crate) fn types(&self) -> Rc<BlockCatalog> {
        self.editor.block_types()
    }

    pub(crate) fn tab(&self, tab: TabId) -> Option<TabItem> {
        self.tabs.with(|tabs| tabs.get(&tab).copied())
    }

    pub(crate) fn simulated(&self, id: Uuid) -> Option<AccessLevel> {
        self.simulated.with(|simulated| simulated.get(&id).copied())
    }

    pub(crate) fn is_debugged(&self, id: Uuid) -> bool {
        self.debugged.with(|debugged| debugged.contains(&id))
    }

    fn show_requested(&self) {
        for request in self.host().take_show_requests() {
            self.open(
                TabItem {
                    id: request.block_id,
                    block_type: request.block_type,
                },
                request.via,
            );
        }
    }

    fn find_files(&self) {
        let files = self
            .file_tree
            .borrow_mut()
            .find(&self.editor, self.host().client_id());
        self.set_files.set(files);
    }

    fn refresh_titles(&self) {
        let types = self.types();
        let titles = self.tabs.with(|tabs| {
            tabs.iter()
                .map(|(tab, item)| {
                    let label = self.info(item.id).map_or_else(
                        || BlockLabel::new(types.as_ref(), item.block_type, None, false),
                        |info| info.label(types.as_ref()),
                    );
                    (*tab, label.name)
                })
                .collect()
        });
        self.set_titles.set(titles);
    }

    fn report_focus(&self) {
        let shown = self.layout.with(DockState::focused_tab);
        let current = shown
            .and_then(|tab| self.tabs.with(|tabs| tabs.get(&tab).copied()))
            .map(|item| item.id);
        self.routes.get();
        let active = current.or_else(|| self.active.get());
        self.active.set(active);
        let focused = active.and_then(|id| {
            let block_type = self.block_types.borrow().get(&id).copied()?;
            Some((id, block_type))
        });
        self.host().report_focus(FocusedBlock {
            block_id: focused.map(|(id, _)| id),
            block_type: focused.map_or_else(Uuid::nil, |(_, block_type)| block_type),
            via: focused.map_or_else(Vec::new, |(id, _)| self.via_chain(id)),
        });
    }

    fn watch_artifacts(&self) {
        let watched = self.tabs.with(|tabs| {
            tabs.values()
                .map(|item| item.id)
                .filter(|id| self.info(*id).is_some_and(|info| info.is_artifact()))
                .collect::<Vec<_>>()
        });
        self.host().watch_artifacts(watched);
    }

    fn via_chain(&self, id: Uuid) -> Vec<Uuid> {
        let opened_via = self.opened_via.borrow();
        let mut via = Vec::new();
        let mut visited = HashSet::new();
        visited.insert(id);
        let mut current = id;
        while let Some(&container) = opened_via.get(&current) {
            if via.len() >= MAX_OPENED_VIA_HOPS || !visited.insert(container) {
                break;
            }
            via.push(container);
            current = container;
        }
        via
    }

    fn record_via(&self, id: Uuid, via: Option<Uuid>) {
        let previous = {
            let mut opened_via = self.opened_via.borrow_mut();
            match via {
                Some(container) => opened_via.insert(id, container),
                None => opened_via.remove(&id),
            }
        };
        if previous != via {
            self.rerouted();
        }
    }

    pub(crate) fn forget_container(&self, id: Uuid) {
        if self.opened_via.borrow_mut().remove(&id).is_some() {
            self.rerouted();
        }
    }

    fn rerouted(&self) {
        self.set_routes.update(|routes| *routes += 1);
    }

    pub(crate) fn container_of(&self, id: Uuid) -> Option<Uuid> {
        self.opened_via.borrow().get(&id).copied()
    }

    pub(crate) fn record_type(&self, id: Uuid, block_type: Uuid) {
        let previous = self.block_types.borrow_mut().insert(id, block_type);
        if previous != Some(block_type) {
            self.rerouted();
        }
    }

    pub(crate) fn known_type(&self, id: Uuid) -> Option<Uuid> {
        let known = self.block_types.borrow().get(&id).copied();
        known.or_else(|| self.info(id).map(|info| info.block_type))
    }

    pub(crate) fn record_reference_types(&self, reference: &BlockInfo) {
        self.record_type(reference.id, reference.block_type);
        if let BlockParent::Block(parent) = reference.parent
            && let Some(parent) = self.blocks().info(parent)
        {
            self.record_type(parent.id, parent.block_type);
        }
    }

    pub(crate) fn open(&self, item: TabItem, via: Option<Uuid>) {
        self.record_via(item.id, via);
        self.record_type(item.id, item.block_type);
        if let Some(tab) = self.tab_showing(item.id) {
            let mut layout = self.layout.get_untracked();
            layout.show(tab);
            self.set_layout.set(layout);
            self.editor.show_pane(tab);
            self.active.set(Some(item.id));
            return;
        }
        let tab = TabId::new(self.next_tab.get());
        self.next_tab.set(self.next_tab.get() + 1);
        let mut tabs = self.tabs.get_untracked();
        tabs.insert(tab, item);
        self.set_tabs.set(tabs);
        let mut layout = self.layout.get_untracked();
        place_tab(&mut layout, tab);
        self.set_layout.set(settled(layout));
        self.editor.show_pane(tab);
        self.active.set(Some(item.id));
    }

    fn tab_showing(&self, id: Uuid) -> Option<TabId> {
        self.tabs.with_untracked(|tabs| {
            tabs.iter()
                .find(|(_, item)| item.id == id)
                .map(|(tab, _)| *tab)
        })
    }

    fn close(&self, tab: TabId) {
        let mut tabs = self.tabs.get_untracked();
        let Some(closed) = tabs.remove(&tab) else {
            return;
        };
        let still_open = tabs.values().any(|item| item.id == closed.id);
        self.set_tabs.set(tabs);
        if !still_open {
            self.forget(closed.id);
        }
    }

    fn forget(&self, id: Uuid) {
        self.handles.borrow_mut().remove(&id);
        self.opened_via.borrow_mut().remove(&id);
        let mut simulated = self.simulated.get_untracked();
        simulated.remove(&id);
        self.set_simulated.set(simulated);
        let mut debugged = self.debugged.get_untracked();
        debugged.remove(&id);
        self.set_debugged.set(debugged);
        if self.active.get() == Some(id) {
            self.active.set(None);
            self.rerouted();
        }
        self.host().close_editor(id);
    }

    fn changed(&self, next: DockState) {
        self.set_layout.set(settled(next));
    }

    fn set_compact(&self, compact: bool) {
        let mut layout = self.layout.get_untracked();
        set_files_compact(&mut layout, compact);
        self.set_layout.set(layout);
    }

    pub(crate) fn can_edit(&self, id: Uuid) -> bool {
        self.info(id).is_none_or(|info| info.access.can_edit())
    }

    pub(crate) fn ceiling(&self, id: Uuid) -> AccessLevel {
        let Some(info) = self.info(id) else {
            return AccessLevel::Edit;
        };
        match info.is_artifact() {
            true => info.access.min(AccessLevel::View),
            false => info.access,
        }
    }

    pub(crate) fn access(&self, id: Uuid) -> AccessLevel {
        let ceiling = self.ceiling(id);
        match self.simulated(id) {
            Some(AccessLevel::None) => AccessLevel::None.min(ceiling),
            Some(AccessLevel::KnowExists) => AccessLevel::KnowExists.min(ceiling),
            Some(AccessLevel::View) => AccessLevel::View.min(ceiling),
            Some(AccessLevel::Edit) | None => ceiling,
        }
    }

    pub(crate) fn simulate(&self, id: Uuid, level: AccessLevel) {
        let mut debugged = self.debugged.get_untracked();
        debugged.remove(&id);
        self.set_debugged.set(debugged);
        let mut simulated = self.simulated.get_untracked();
        simulated.insert(id, level);
        self.set_simulated.set(simulated);
        self.host().simulate_access(id, level);
    }

    pub(crate) fn debug(&self, id: Uuid, debugging: bool) {
        let mut debugged = self.debugged.get_untracked();
        match debugging {
            true => debugged.insert(id),
            false => debugged.remove(&id),
        };
        self.set_debugged.set(debugged);
    }

    pub(crate) fn label(&self, id: Uuid, block_type: Uuid) -> BlockLabel {
        let types = self.types();
        self.info(id).map_or_else(
            || BlockLabel::new(types.as_ref(), block_type, None, false),
            |info| info.label(types.as_ref()),
        )
    }

    pub(crate) fn open_picker(self: &Rc<Self>, parent: Uuid) {
        self.set_error.set(None);
        let picking = Rc::downgrade(self);
        self.editor.pick_block(
            BlockFilter {
                name: "Block".to_owned(),
                block_types: Vec::new(),
                excluded: vec![parent.into_bytes()],
                templates: false,
            },
            move |picked: Result<PickedBlock, String>| {
                let Some(workspace) = picking.upgrade() else {
                    return;
                };
                match picked {
                    Ok(picked) => workspace.host().place_block(
                        picked.id,
                        picked.block_type,
                        parent,
                        picked.linked,
                    ),
                    Err(error) => workspace.set_error.set(Some(error)),
                }
            },
        );
    }
}

pub(crate) fn starting_layout() -> DockState {
    let mut state = DockState::new([FILES]);
    let files = state.leaves(state.main())[0];
    state.split(files, Side::Right, 1.0 - FILES_SHARE, vec![EMPTY]);
    state
}

pub(crate) fn settled(mut state: DockState) -> DockState {
    let open = state
        .all_tabs()
        .into_iter()
        .any(|tab| tab != FILES && tab != EMPTY);
    if open {
        state.remove(EMPTY);
        return state;
    }
    if state.contains(EMPTY) {
        return state;
    }
    place_tab(&mut state, EMPTY);
    state
}

fn files_only_leaf(state: &DockState) -> Option<LeafId> {
    state
        .find(FILES)
        .map(|position| position.leaf)
        .filter(|leaf| state.entries(*leaf).len() == 1)
}

fn editor_leaf(state: &DockState) -> Option<LeafId> {
    let exclusive = files_only_leaf(state);
    state
        .focused_leaf()
        .filter(|leaf| Some(*leaf) != exclusive)
        .or_else(|| {
            state
                .surfaces()
                .into_iter()
                .flat_map(|surface| state.leaves(surface))
                .find(|leaf| Some(*leaf) != exclusive)
        })
}

pub(crate) fn place_tab(state: &mut DockState, tab: TabId) {
    if state.replace(EMPTY, tab) {
        state.show(tab);
        return;
    }
    let files = state.find(FILES).map(|position| position.leaf);
    match (editor_leaf(state), files) {
        (Some(leaf), _) => state.push(leaf, tab),
        (None, Some(files)) => {
            state.split(files, Side::Right, 1.0 - FILES_SHARE, vec![tab]);
        }
        (None, None) => state.push_to_focused(tab),
    }
    state.show(tab);
}

pub(crate) fn set_files_compact(state: &mut DockState, compact: bool) {
    let Some(position) = state.find(FILES) else {
        return;
    };
    let alone = state.entries(position.leaf).len() == 1;
    if compact != alone {
        return;
    }
    let focused = state.focused_tab().filter(|tab| *tab != FILES);
    let target = match compact {
        true => state
            .surfaces()
            .into_iter()
            .flat_map(|surface| state.leaves(surface))
            .find(|leaf| *leaf != position.leaf),
        false => Some(position.leaf),
    };
    let Some(target) = target else {
        return;
    };
    state.remove(FILES);
    match compact {
        true => state.push(target, FILES),
        false => {
            state.split(target, Side::Left, FILES_SHARE, vec![FILES]);
        }
    }
    if let Some(tab) = focused {
        state.show(tab);
    }
}

#[component]
pub(crate) fn WorkspaceShell(editor: Editor) -> NodeId {
    let workspace = Workspace::new(editor);
    view! {
        <Container>
            {move |_| {
                let workspace = Rc::clone(&workspace);
                view! {
                    <WorkspaceBody workspace={workspace} />
                }
            }}
        </Container>
    }
}

#[component]
fn WorkspaceBody(workspace: Rc<Workspace>) -> NodeId {
    let compact = narrower_than(COMPACT_FILES_WIDTH);
    let sizing = Rc::downgrade(&workspace);
    let was_compact = Cell::new(false);
    create_effect(move || {
        let now = compact.get();
        let Some(workspace) = sizing.upgrade() else {
            return;
        };
        if was_compact.replace(now) != now {
            untrack(|| workspace.set_compact(now));
        }
    });
    let surface = NodeRef::new();
    workspace.editor().content(&surface);
    let layout = workspace.layout.clone();
    let titles = workspace.titles.clone();
    let failure = workspace.error.clone();
    let docked = workspace.host().panes_offered();
    let failed = create_memo(clone!(failure -> move || !docked && failure.get().is_some()));
    let reason = create_memo(clone!(failure -> move || failure.get().unwrap_or_default()));
    let title = Func::new(move |tab: TabId| match tab {
        FILES => "Files".to_owned(),
        EMPTY => "Workspace".to_owned(),
        tab => titles.with(|titles| {
            titles
                .get(&tab)
                .cloned()
                .unwrap_or_else(|| "Untitled".to_owned())
        }),
    });
    let changing = Rc::clone(&workspace);
    let closing = Rc::clone(&workspace);
    let content = Rc::clone(&workspace);
    let editor = workspace.editor().clone();
    let theme = use_theme();
    view! {
        <Frame @node_ref={&surface} color={theme.background.clone()}>
            <List spacing=0.0>
                <Failure failed={failed} reason={reason} />
                <EditorDock
                    @sizing=ItemSize::Percent(100.0)
                    editor={editor}
                    state={layout}
                    title={title}
                    closable={Func::new(|tab: TabId| tab != FILES && tab != EMPTY)}
                    on_change={move |next: DockState| changing.changed(next)}
                    on_close={move |tab: TabId| closing.close(tab)}
                >
                    {move |tab: TabId| {
                        let workspace = Rc::clone(&content);
                        match tab {
                            FILES => view! {
                                <FilesPanel workspace={workspace} />
                            },
                            EMPTY => view! {
                                <EmptyPanel />
                            },
                            tab => view! {
                                <BlockPanel workspace={workspace} tab={tab} />
                            },
                        }
                    }}
                </EditorDock>
            </List>
        </Frame>
    }
}

#[component]
fn Failure(failed: Memo<bool>, reason: Memo<String>) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame visible={failed}>
            <Caption content={reason} color={theme.danger.clone()} />
        </Frame>
    }
}

#[component]
fn FilesPanel(workspace: Rc<Workspace>) -> NodeId {
    let failure = workspace.error.clone();
    let docked = workspace.host().panes_offered();
    let failed = create_memo(clone!(failure -> move || docked && failure.get().is_some()));
    let reason = create_memo(clone!(failure -> move || failure.get().unwrap_or_default()));
    let files = workspace.files.clone();
    let target = create_memo(move || {
        files
            .get()
            .map(|id| ChildTarget::new(id, FileTreeContent::CONTENT_TYPE))
    });
    let editor = workspace.editor().clone();
    view! {
        <List spacing=0.0>
            <Failure failed={failed} reason={reason} />
            <ChildBlock
                @sizing=ItemSize::Percent(100.0)
                editor={editor}
                block={target}
                mode=ChildMode::Live
                own_frame=true
                @test_id={"workspace.files"}
            >
                {move |handle: ChildBlockHandle| view! {
                    <PanelStatus state={handle.state} loading="Files are loading…" />
                }}
            </ChildBlock>
        </List>
    }
}

#[component]
pub(crate) fn PanelStatus(state: ReadSignal<ChildState>, loading: String) -> NodeId {
    let theme = use_theme();
    let shown = create_memo(clone!(state -> move || {
        state.with(|state| !state.available || state.error.is_some())
    }));
    let message = create_memo(clone!(state -> move || {
        state.with(|state| match &state.error {
            Some(error) => error.clone(),
            None => loading.clone(),
        })
    }));
    view! {
        <Frame visible={shown} padding_horizontal=PANEL_PADDING padding_vertical=PANEL_PADDING>
            <List spacing=PANEL_SPACING>
                <Caption content={message} color={theme.text_muted.clone()} />
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
        </Frame>
    }
}

#[component]
fn EmptyPanel() -> NodeId {
    view! {
        <Frame padding_horizontal=PANEL_PADDING padding_vertical=PANEL_PADDING>
            <List spacing=PANEL_SPACING align=Align::Center>
                <Heading content="No file open" />
                <Caption content="Open or create a file from Files to get started." />
            </List>
        </Frame>
    }
}
