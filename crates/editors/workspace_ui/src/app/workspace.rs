use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use block::{Block, BlockAccess, BlockParent, BlockReference};
use block_client::blocks::file_tree::FileTree;
use block_client::root_settings::RootSetting;
use block_client::{BlockClient, BlockHandle, BlockHandleAccess};
use block_editor_plugin::beui::reactive::{
    Align, Frame, Func, ItemSize, List, NodeRef, ReadSignal, Show, Spacer, WriteSignal, clone,
    component, create_memo, create_signal, on_shortcut, view,
};
use block_editor_plugin::beui::styled::{Caption, DockArea, Heading, use_theme};
use block_editor_plugin::beui::unstyled::{
    Container, DockState, LeafId, Side, TabId, narrower_than,
};
use block_editor_plugin::beui::{Key, KeyPress, NodeId};
use block_editor_plugin::block_ui::{BlockCatalog, BlockLabel};
use block_editor_plugin::{
    AccessLevel, BlockFilter, ChildBlock, ChildBlockHandle, ChildMode, ChildState, ChildTarget,
    Editor, EditorHost, FocusedBlock, PickedBlock,
};
use uuid::Uuid;

use super::panel::BlockPanel;
use super::tab::{BlockTab, Navigation, TabItem};

pub(crate) const FILES: TabId = TabId::new(1);
pub(crate) const EMPTY: TabId = TabId::new(2);

const FIRST_BLOCK_TAB: u64 = 3;
const FILES_SHARE: f32 = 0.22;
const COMPACT_FILES_WIDTH: f32 = 700.0;
const MAX_OPENED_VIA_HOPS: usize = 64;
const PANEL_PADDING: f32 = 14.0;
const PANEL_SPACING: f32 = 6.0;

type Tabs = HashMap<TabId, BlockTab>;

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
    file_tree: RefCell<RootSetting<FileTree>>,
    handles: RefCell<HashMap<Uuid, Box<dyn BlockHandleAccess>>>,
    block_types: RefCell<HashMap<Uuid, Uuid>>,
    opened_via: RefCell<HashMap<Uuid, Uuid>>,
    next_tab: Cell<u64>,
    active: Cell<Option<Uuid>>,
}

impl Workspace {
    fn new(editor: Editor) -> Rc<Self> {
        let file_tree = RootSetting::new(editor.client());
        let (layout, set_layout) = create_signal(starting_layout());
        let (tabs, set_tabs) = create_signal(Tabs::new());
        let (titles, set_titles) = create_signal(HashMap::new());
        let (simulated, set_simulated) = create_signal(HashMap::new());
        let (debugged, set_debugged) = create_signal(HashSet::new());
        let (error, set_error) = create_signal(None);
        let (files, set_files) = create_signal(None);
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
            file_tree: RefCell::new(file_tree),
            handles: RefCell::new(HashMap::new()),
            block_types: RefCell::new(HashMap::new()),
            opened_via: RefCell::new(HashMap::new()),
            next_tab: Cell::new(FIRST_BLOCK_TAB),
            active: Cell::new(None),
        });
        let each_frame = Rc::downgrade(&workspace);
        workspace.editor.each_frame(move || {
            if let Some(workspace) = each_frame.upgrade() {
                workspace.frame();
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

    pub(crate) fn client(&self) -> &Arc<BlockClient> {
        self.editor.client()
    }

    pub(crate) fn types(&self) -> Rc<BlockCatalog> {
        self.editor.block_types()
    }

    pub(crate) fn tab(&self, tab: TabId) -> Option<BlockTab> {
        self.tabs.with(|tabs| tabs.get(&tab).cloned())
    }

    pub(crate) fn simulated(&self, id: Uuid) -> Option<AccessLevel> {
        self.simulated.with(|simulated| simulated.get(&id).copied())
    }

    pub(crate) fn is_debugged(&self, id: Uuid) -> bool {
        self.debugged.with(|debugged| debugged.contains(&id))
    }

    fn frame(&self) {
        for request in self.host().take_show_requests() {
            self.open(
                TabItem {
                    id: request.block_id,
                    block_type: request.block_type,
                },
                request.via,
                request.from,
            );
        }
        let files = self
            .file_tree
            .borrow_mut()
            .find(self.client(), self.host().client_id())
            .map(BlockHandle::id);
        self.set_files.set(files);
        self.refresh_titles();
        self.report_focus();
        self.watch_artifacts();
    }

    fn refresh_titles(&self) {
        let types = self.types();
        let titles = self.tabs.with_untracked(|tabs| {
            tabs.iter()
                .map(|(tab, block)| {
                    let item = block.current();
                    let label = self.client().cached_block(item.id).map_or_else(
                        || BlockLabel::new(types.as_ref(), item.block_type, None),
                        |cached| BlockLabel::for_cached(types.as_ref(), &cached),
                    );
                    (*tab, label.name)
                })
                .collect()
        });
        self.set_titles.set(titles);
    }

    fn report_focus(&self) {
        let shown = self.layout.with_untracked(DockState::focused_tab);
        let current = shown
            .and_then(|tab| self.tabs.with_untracked(|tabs| tabs.get(&tab).cloned()))
            .map(|tab| tab.current().id);
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
        let watched = self.tabs.with_untracked(|tabs| {
            tabs.values()
                .map(|tab| tab.current().id)
                .filter(|id| self.client().is_dynamic_artifact(*id))
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
        let mut opened_via = self.opened_via.borrow_mut();
        match via {
            Some(container) => opened_via.insert(id, container),
            None => opened_via.remove(&id),
        };
    }

    pub(crate) fn forget_container(&self, id: Uuid) {
        self.opened_via.borrow_mut().remove(&id);
    }

    pub(crate) fn container_of(&self, id: Uuid) -> Option<Uuid> {
        self.opened_via.borrow().get(&id).copied()
    }

    pub(crate) fn record_type(&self, id: Uuid, block_type: Uuid) {
        self.block_types.borrow_mut().insert(id, block_type);
    }

    pub(crate) fn known_type(&self, id: Uuid) -> Option<Uuid> {
        self.block_types.borrow().get(&id).copied().or_else(|| {
            self.client()
                .cached_block(id)
                .map(|cached| cached.block_type)
        })
    }

    pub(crate) fn record_reference_types(&self, reference: &BlockReference) {
        self.record_type(reference.id, reference.block_type);
        if let BlockParent::Uuid(parent) = reference.parent
            && let Some(parent) = self.client().cached_block(parent)
        {
            self.record_type(parent.id, parent.block_type);
        }
    }

    fn open(&self, item: TabItem, via: Option<Uuid>, from: Option<Uuid>) {
        if let Some(from) = from.filter(|from| *from != item.id)
            && let Some(tab) = self.tab_showing(from)
        {
            self.record_via(item.id, via);
            self.navigate(tab, Navigation::Open(item));
            return;
        }
        self.record_via(item.id, via);
        self.record_type(item.id, item.block_type);
        if let Some(tab) = self.tab_showing(item.id) {
            let mut layout = self.layout.get_untracked();
            layout.show(tab);
            self.set_layout.set(layout);
            self.active.set(Some(item.id));
            return;
        }
        let tab = TabId::new(self.next_tab.get());
        self.next_tab.set(self.next_tab.get() + 1);
        let mut tabs = self.tabs.get_untracked();
        tabs.insert(tab, BlockTab::new(item));
        self.set_tabs.set(tabs);
        let mut layout = self.layout.get_untracked();
        place_tab(&mut layout, tab);
        self.set_layout.set(settled(layout));
        self.active.set(Some(item.id));
    }

    fn tab_showing(&self, id: Uuid) -> Option<TabId> {
        self.tabs.with_untracked(|tabs| {
            tabs.iter()
                .find(|(_, tab)| tab.current().id == id)
                .map(|(tab, _)| *tab)
        })
    }

    pub(crate) fn navigate(&self, tab: TabId, navigation: Navigation) {
        let mut tabs = self.tabs.get_untracked();
        let Some(block) = tabs.get_mut(&tab) else {
            return;
        };
        match navigation {
            Navigation::Back if block.can_go_back() => block.index -= 1,
            Navigation::Forward if block.can_go_forward() => block.index += 1,
            Navigation::Open(item) => block.navigate(item),
            Navigation::Back | Navigation::Forward => {}
        }
        let current = block.current();
        self.record_type(current.id, current.block_type);
        self.set_tabs.set(tabs);
        self.active.set(Some(current.id));
    }

    fn close(&self, tab: TabId) {
        let mut tabs = self.tabs.get_untracked();
        let Some(closed) = tabs.remove(&tab) else {
            return;
        };
        let still_open: HashSet<Uuid> = tabs.values().flat_map(BlockTab::blocks).collect();
        self.set_tabs.set(tabs);
        for id in closed.blocks() {
            if !still_open.contains(&id) {
                self.forget(id);
            }
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

    pub(crate) fn read_handle<R>(
        &self,
        item: TabItem,
        read: impl FnOnce(&dyn BlockHandleAccess) -> R,
    ) -> Option<R> {
        let mut handles = self.handles.borrow_mut();
        if let std::collections::hash_map::Entry::Vacant(e) = handles.entry(item.id) {
            let handle = block_client::blocks::open(self.client(), item.id, item.block_type)?;
            e.insert(handle);
        }
        handles.get(&item.id).map(|handle| read(handle.as_ref()))
    }

    pub(crate) fn can_edit(&self, id: Uuid) -> bool {
        self.client().block_access(id).can_edit()
    }

    pub(crate) fn ceiling(&self, id: Uuid) -> BlockAccess {
        let access = self.client().block_access(id);
        match self.client().is_dynamic_artifact(id) {
            true => access.min(BlockAccess::View),
            false => access,
        }
    }

    pub(crate) fn access(&self, id: Uuid) -> BlockAccess {
        let ceiling = self.ceiling(id);
        match self.simulated(id) {
            Some(AccessLevel::None) => BlockAccess::None.min(ceiling),
            Some(AccessLevel::KnowExists) => BlockAccess::KnowExists.min(ceiling),
            Some(AccessLevel::View) => BlockAccess::View.min(ceiling),
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
        self.client().cached_block(id).map_or_else(
            || BlockLabel::new(types.as_ref(), block_type, None),
            |cached| BlockLabel::for_cached(types.as_ref(), &cached),
        )
    }

    pub(crate) fn history_command(&self, redo: bool) -> bool {
        let Some(tab) = self.layout.with_untracked(DockState::focused_tab) else {
            return false;
        };
        let Some(block) = self.tabs.with_untracked(|tabs| tabs.get(&tab).cloned()) else {
            return false;
        };
        let item = block.current();
        if !self.access(item.id).can_edit() {
            return false;
        }
        self.read_handle(item, |handle| {
            let Some(history) = handle.history() else {
                return false;
            };
            match redo {
                true if history.can_redo() => {
                    history.redo();
                    true
                }
                false if history.can_undo() => {
                    history.undo();
                    true
                }
                _ => false,
            }
        })
        .unwrap_or(false)
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
        .filter(|leaf| state.tabs(*leaf).len() == 1)
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
    let alone = state.tabs(position.leaf).len() == 1;
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
    workspace.editor().each_frame(move || {
        let Some(workspace) = sizing.upgrade() else {
            return;
        };
        let now = compact.get_untracked();
        if was_compact.replace(now) != now {
            workspace.set_compact(now);
        }
    });
    let shortcuts = Rc::downgrade(&workspace);
    on_shortcut(move |press: KeyPress| {
        if !press.pressed || !press.modifiers.ctrl {
            return false;
        }
        let Some(workspace) = shortcuts.upgrade() else {
            return false;
        };
        match press.key {
            Key::Z => workspace.history_command(press.modifiers.shift),
            Key::Y => workspace.history_command(true),
            _ => false,
        }
    });
    let surface = NodeRef::new();
    workspace.editor().content(&surface);
    let layout = workspace.layout.clone();
    let titles = workspace.titles.clone();
    let failure = workspace.error.clone();
    let failed = create_memo(clone!(failure -> move || failure.get().is_some()));
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
    let theme = use_theme();
    view! {
        <Frame @node_ref={&surface} color={theme.background.clone()}>
            <List spacing=0.0>
                <Show condition={failed}>
                    <Caption content={reason} color={theme.danger.clone()} />
                </Show>
                <DockArea
                    @sizing=ItemSize::Percent(100.0)
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
                </DockArea>
            </List>
        </Frame>
    }
}

#[component]
fn FilesPanel(workspace: Rc<Workspace>) -> NodeId {
    let files = workspace.files.clone();
    let target = create_memo(move || {
        files
            .get()
            .map(|id| ChildTarget::new(id, <FileTree as Block>::TYPE_ID))
    });
    let editor = workspace.editor().clone();
    view! {
        <ChildBlock
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
