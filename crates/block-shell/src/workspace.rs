use block_editor_beui::be_block::profile::{RECENTS, VIEW_EDITORS};
use block_editor_beui::be_block::{
    BlockContent, EditorView, EditorViewContent, FILES_EDITOR, Recents, ViewState,
};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use block_editor_beui::beui::icons::ICON_WEB_ASSET;
use block_editor_beui::beui::reactive::{
    Frame, ItemSize, List, Memo, ReadSignal, Spacer, WriteSignal, batch, clone, component,
    create_effect, create_memo, create_signal, untrack, view,
};
use block_editor_beui::beui::styled::{Caption, use_theme};
use block_editor_beui::beui::unstyled::{
    DockEntry, DockNode, DockPane, DockTab, DockWindow, DockingLayout, TabId,
};
use block_editor_beui::beui::{NodeId, Rect, pos2, vec2};
use block_editor_beui::block_ui::{BlockCatalog, BlockLabel, BlockTypes};
use block_editor_beui::{
    AccessLevel, BlockFilter, BlockPick, ChildState, Editor, EditorHost, FocusedBlock, HostPanel,
    HostWindow, HostWindowId, HostWindows, PickedBlock, Pushed, ShellDialog, WindowAction,
};
use block_editor_beui::{BlockInfo, BlockList, BlockParent, BlockQuery, Blocks};
use uuid::Uuid;

use super::dialogs::OpenDialog;
use super::host_panel::{HostPanelView, panel_icon, panel_tab, panel_window, tab_panel};
use super::panel::BlockPanel;
use super::picker::{Pick, PickAction, PickOutcome};
use super::saved::{self, LAYOUT};
use super::share::{Share, ShareAction};
use super::tab::TabItem;
use super::window::{WindowPanel, dialog_window, tab_window, window_tab, window_title};

pub const FILES: TabId = TabId::new(1);
const FIRST_BLOCK_TAB: u64 = 2;
const MAX_OPENED_VIA_HOPS: usize = 64;
const PANEL_PADDING: f32 = 14.0;
const PANEL_SPACING: f32 = 6.0;

type Tabs = HashMap<TabId, TabItem>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PhoneSheet {
    Closed,
    Details(TabId),
}

pub struct Workspace {
    editor: Editor,
    with_files: bool,
    layout: DockingLayout<TabId>,
    tabs: ReadSignal<Tabs>,
    set_tabs: WriteSignal<Tabs>,
    views: ReadSignal<HashMap<TabId, Uuid>>,
    set_views: WriteSignal<HashMap<TabId, Uuid>>,
    restored: Cell<bool>,
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
    handles: RefCell<HashMap<Uuid, BlockList>>,
    block_types: RefCell<HashMap<Uuid, Uuid>>,
    opened_via: RefCell<HashMap<Uuid, Uuid>>,
    routes: ReadSignal<u64>,
    set_routes: WriteSignal<u64>,
    next_tab: Cell<u64>,
    active: Cell<Option<Uuid>>,
    pub(crate) phone: ReadSignal<bool>,
    set_phone: WriteSignal<bool>,
    pub(crate) sheet: ReadSignal<PhoneSheet>,
    set_sheet: WriteSignal<PhoneSheet>,
    pub(crate) picks: ReadSignal<Vec<Pick>>,
    set_picks: WriteSignal<Vec<Pick>>,
    pub(crate) dialog: ReadSignal<Option<OpenDialog>>,
    set_dialog: WriteSignal<Option<OpenDialog>>,
    pub(crate) share: ReadSignal<Option<Share>>,
    set_share: WriteSignal<Option<Share>>,
    every_block: RefCell<Option<BlockList>>,
    windows: Memo<Vec<HostWindow>>,
    panels: ReadSignal<Vec<HostPanel>>,
    set_panels: WriteSignal<Vec<HostPanel>>,
}

impl Workspace {
    pub fn new(editor: Editor, with_files: bool) -> Rc<Self> {
        let (tabs, set_tabs) = create_signal(Tabs::new());
        let (views, set_views) = create_signal(HashMap::new());
        let (titles, set_titles) = create_signal(HashMap::new());
        let (simulated, set_simulated) = create_signal(HashMap::new());
        let (debugged, set_debugged) = create_signal(HashSet::new());
        let (error, set_error) = create_signal(None);
        let (files, set_files) = create_signal(None);
        let (routes, set_routes) = create_signal(0);
        let (phone, set_phone) = create_signal(false);
        let (sheet, set_sheet) = create_signal(PhoneSheet::Closed);
        let (picks, set_picks) = create_signal(Vec::new());
        let (dialog, set_dialog) = create_signal(None);
        let (share, set_share) = create_signal(None);
        let (panels, set_panels) = create_signal(Vec::new());
        let windows = editor.host_value::<HostWindows>();
        let workspace = Rc::new(Self {
            editor,
            with_files,
            layout: DockingLayout::new(),
            tabs,
            set_tabs,
            views,
            set_views,
            restored: Cell::new(false),
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
            handles: RefCell::new(HashMap::new()),
            block_types: RefCell::new(HashMap::new()),
            opened_via: RefCell::new(HashMap::new()),
            routes,
            set_routes,
            next_tab: Cell::new(FIRST_BLOCK_TAB),
            active: Cell::new(None),
            phone,
            set_phone,
            sheet,
            set_sheet,
            picks,
            set_picks,
            dialog,
            set_dialog,
            share,
            set_share,
            every_block: RefCell::new(None),
            windows,
            panels,
            set_panels,
        });
        let shows = workspace.editor.pushed(Pushed::Shows);
        let showing = Rc::downgrade(&workspace);
        create_effect(move || {
            shows.get();
            if let Some(workspace) = showing.upgrade() {
                untrack(|| workspace.show_requested());
            }
        });
        let restoring = Rc::downgrade(&workspace);
        create_effect(move || {
            if let Some(workspace) = restoring.upgrade() {
                workspace.restore();
            }
        });
        let saving = Rc::downgrade(&workspace);
        create_effect(move || {
            if let Some(workspace) = saving.upgrade() {
                workspace.save();
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
        let listing = Rc::downgrade(&workspace);
        workspace.editor.on_reply(move || {
            if let Some(workspace) = listing.upgrade() {
                workspace.share_listed();
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

    pub fn editor(&self) -> &Editor {
        &self.editor
    }

    pub fn layout(&self) -> DockingLayout<TabId> {
        self.layout.clone()
    }

    pub fn phone(&self) -> ReadSignal<bool> {
        self.phone.clone()
    }

    pub fn error(&self) -> ReadSignal<Option<String>> {
        self.error.clone()
    }

    pub fn files(&self) -> ReadSignal<Option<Uuid>> {
        self.files.clone()
    }

    pub fn panels(&self) -> ReadSignal<Vec<HostPanel>> {
        self.panels.clone()
    }

    pub fn block_tabs(&self) -> Memo<Vec<TabId>> {
        let listed = self.tabs.clone();
        create_memo(move || {
            let mut tabs: Vec<TabId> = listed.with(|tabs| tabs.keys().copied().collect());
            tabs.sort();
            tabs
        })
    }

    pub fn windows(&self, dialogs: bool) -> Memo<Vec<HostWindowId>> {
        let windows = self.windows.clone();
        create_memo(move || {
            windows.with(|windows| {
                windows
                    .iter()
                    .filter(|window| window.parent.is_some() == dialogs)
                    .map(|window| window.id)
                    .collect::<Vec<_>>()
            })
        })
    }

    pub fn open_block(&self, id: Uuid, block_type: Uuid) {
        self.open(TabItem { id, block_type }, None);
    }

    pub(crate) fn host(&self) -> &EditorHost {
        self.editor.host()
    }

    pub(crate) fn blocks(&self) -> Blocks {
        self.editor.blocks()
    }

    pub fn info(&self, id: Uuid) -> Option<BlockInfo> {
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
        let requested: Vec<Pick> = self
            .host()
            .take_pick_requests()
            .into_iter()
            .map(Pick::new)
            .collect();
        if !requested.is_empty() {
            self.set_picks.update(|picks| picks.extend(requested));
        }
        for (block, dialog) in self.host().take_dialog_requests() {
            match dialog {
                ShellDialog::Rename => self.open_dialog(OpenDialog::Rename(block)),
                ShellDialog::Share => self.open_dialog(OpenDialog::Share(block)),
            }
        }
        for panel in self.editor.take_panel_requests() {
            self.show_panel(panel);
        }
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

    pub(crate) fn open_dialog(&self, dialog: OpenDialog) {
        self.forget_share();
        if let OpenDialog::Share(block) = dialog {
            let request = self.host().list_access(block);
            self.set_share.set(Some(Share::new(block, request)));
        }
        self.set_dialog.set(Some(dialog));
    }

    pub(crate) fn close_dialog(&self) {
        self.forget_share();
        self.set_dialog.set(None);
    }

    fn forget_share(&self) {
        if let Some(Share {
            request: Some(request),
            ..
        }) = self.share.get_untracked()
        {
            self.host().forget_request(request);
        }
        self.set_share.set(None);
    }

    pub(crate) fn rename(&self, block: Uuid, name: &str) {
        let name = name.trim().to_owned();
        self.blocks()
            .set_name(block, (!name.is_empty()).then_some(name));
        self.close_dialog();
    }

    pub(crate) fn share_action(&self, action: ShareAction) {
        let Some(mut share) = self.share.get_untracked() else {
            return;
        };
        let (grants, mut reload) = share.act(action, self.host().account_id());
        for (account, access) in grants {
            self.host().set_access(share.block, account, access);
            reload = true;
        }
        if reload {
            if let Some(request) = share.request.take() {
                self.host().forget_request(request);
            }
            share.error = None;
            share.request = Some(self.host().list_access(share.block));
        }
        self.set_share.set(Some(share));
    }

    fn share_listed(&self) {
        let Some(mut share) = self.share.get_untracked() else {
            return;
        };
        let Some(listing) = share
            .request
            .and_then(|request| self.host().take_access_listing(request))
        else {
            return;
        };
        share.listed(listing);
        self.set_share.set(Some(share));
    }

    pub(crate) fn every_block(&self) -> Vec<BlockInfo> {
        let mut held = self.every_block.borrow_mut();
        held.get_or_insert_with(|| self.blocks().watch(BlockQuery::All))
            .read()
    }

    pub(crate) fn pick(&self, pick: u64) -> Option<Pick> {
        self.picks
            .with(|picks| picks.iter().find(|held| held.pick == pick).cloned())
    }

    pub(crate) fn pick_action(&self, pick: u64, action: PickAction) {
        let Some(held) = self.pick(pick) else {
            return;
        };
        match held.act(action) {
            PickOutcome::Open(mut held) => {
                self.send_commit(&mut held);
                self.resolve(PickOutcome::Open(held));
            }
            answered => self.resolve(answered),
        }
    }

    fn send_commit(&self, held: &mut Pick) {
        let name = Some(held.name.trim().to_owned()).filter(|name| !name.is_empty());
        let parent = held.created_parent();
        if let Some(creating) = &mut held.creating
            && let Some(child) = creating.child
            && creating.committed
            && !creating.sent
        {
            creating.sent = true;
            self.host().commit_child(child, parent, name);
        }
    }

    pub(crate) fn creation_state(&self, pick: u64, state: &ChildState) {
        let Some(mut held) = self.pick(pick) else {
            return;
        };
        if let Some(creating) = &mut held.creating {
            creating.child = state.child;
        }
        self.send_commit(&mut held);
        match &state.creation {
            Some(progress) => self.resolve(held.progressed(progress)),
            None => self.resolve(PickOutcome::Open(Box::new(held))),
        }
    }

    fn resolve(&self, outcome: PickOutcome) {
        match outcome {
            PickOutcome::Open(held) => {
                if self.pick(held.pick).as_ref() != Some(&*held) {
                    self.set_picks.update(|picks| {
                        if let Some(slot) = picks.iter_mut().find(|slot| slot.pick == held.pick) {
                            *slot = *held;
                        }
                    });
                }
            }
            PickOutcome::Answered(pick, answer, into) => {
                self.set_picks
                    .update(|picks| picks.retain(|held| held.pick != pick));
                if let (BlockPick::Chosen { .. }, Some((block, block_type, container))) =
                    (&answer, into)
                {
                    self.host().place_block(block, block_type, container, false);
                }
                self.host().answer_pick(pick, answer);
            }
        }
    }

    fn show_panel(&self, panel: HostPanel) {
        match self.panels.with_untracked(|panels| panels.contains(&panel)) {
            true => self.layout.show(&panel_tab(panel)),
            false => self.set_panels.update(|panels| panels.push(panel)),
        }
    }

    pub(crate) fn view_of(&self, tab: TabId) -> Option<Uuid> {
        self.views.with(|views| views.get(&tab).copied())
    }

    fn create_view(&self, editor: Uuid, content: Option<Uuid>) -> Option<Uuid> {
        let profile = self.editor.view_block()?;
        Some(self.blocks().create_with(
            EditorViewContent::CONTENT_TYPE,
            Some(EditorView::document(editor, content).encode()),
            BlockParent::Block(profile),
            None,
            None,
        ))
    }

    fn restore(&self) {
        if self.restored.get() {
            return;
        }
        let Some(view) = self.editor.view_content() else {
            return;
        };
        let Some(layout) = view.read(|held| held.root().state(LAYOUT).cloned()) else {
            return;
        };
        self.restored.set(true);
        untrack(|| self.adopt(layout.as_ref().and_then(saved::restore)));
    }

    fn adopt(&self, restored: Option<saved::Restored>) {
        let mut files = None;
        if let Some(restored) = restored {
            files = restored.files;
            let mut tabs = Tabs::new();
            let mut views = HashMap::new();
            for (tab, (item, view)) in restored.tabs {
                self.record_type(item.id, item.block_type);
                tabs.insert(tab, item);
                views.insert(tab, view);
            }
            let mut next = restored
                .next_tab
                .max(FIRST_BLOCK_TAB)
                .max(tabs.keys().map(|tab| tab.value() + 1).max().unwrap_or(0));
            let opened = self.tabs.get_untracked();
            let mut opened_views = self.views.get_untracked();
            for (tab, item) in opened {
                let moved = TabId::new(next);
                next += 1;
                tabs.insert(moved, item);
                if let Some(view) = opened_views.remove(&tab) {
                    views.insert(moved, view);
                }
            }
            self.next_tab.set(next);
            let panels: Vec<HostPanel> = restored
                .dock
                .keys()
                .filter_map(|tab| tab_panel(*tab))
                .collect();
            batch(|| {
                self.set_tabs.set(tabs);
                self.set_views.set(views);
                self.set_panels.set(panels);
                self.layout.restore(restored.dock);
            });
        }
        if self.with_files {
            let files = files.or_else(|| self.create_view(FILES_EDITOR, None));
            self.set_files.set(files);
        }
    }

    fn save(&self) {
        let layout = self.layout.snapshot();
        let tabs = self.tabs.get();
        let views = self.views.get();
        let files = self.files.get();
        if !self.restored.get() {
            return;
        }
        let state = saved::save(&layout, self.next_tab.get(), files, &tabs, &views);
        untrack(|| self.editor.set_view_state(LAYOUT, Some(&state)));
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
        let shown = match self.phone.get() {
            true => self.layout.shown(),
            false => self.layout.focused(),
        };
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
        if let Some((id, block_type)) = focused {
            untrack(|| self.remember(id, block_type));
        }
        self.host().report_focus(FocusedBlock {
            block_id: focused.map(|(id, _)| id),
            block_type: focused.map_or_else(Uuid::nil, |(_, block_type)| block_type),
            via: focused.map_or_else(Vec::new, |(id, _)| self.via_chain(id)),
        });
    }

    fn remember(&self, id: Uuid, block_type: Uuid) {
        let recents: Recents = self
            .editor
            .view_state(RECENTS)
            .and_then(|state| state.value())
            .unwrap_or_default();
        if let Some(visited) = recents.visit(id, block_type) {
            self.editor
                .set_view_state(RECENTS, Some(&ViewState::new(&visited, Vec::new())));
        }
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
            self.layout.show(&tab);
            self.active.set(Some(item.id));
            return;
        }
        let tab = TabId::new(self.next_tab.get());
        self.next_tab.set(self.next_tab.get() + 1);
        let view = match VIEW_EDITORS.contains(&item.block_type) {
            true => Some(item.id),
            false => self.create_view(item.block_type, Some(item.id)),
        };
        if let Some(view) = view {
            self.set_views.update(|views| {
                views.insert(tab, view);
            });
        }
        let mut tabs = self.tabs.get_untracked();
        tabs.insert(tab, item);
        self.set_tabs.set(tabs);
        self.active.set(Some(item.id));
    }

    pub(crate) fn set_sheet(&self, sheet: PhoneSheet) {
        self.set_sheet.set(sheet);
    }

    pub(crate) fn dismiss_sheet(&self, sheet: PhoneSheet) {
        if self.sheet.get_untracked() == sheet {
            self.set_sheet.set(PhoneSheet::Closed);
        }
    }

    fn tab_showing(&self, id: Uuid) -> Option<TabId> {
        self.tabs.with_untracked(|tabs| {
            tabs.iter()
                .find(|(_, item)| item.id == id)
                .map(|(tab, _)| *tab)
        })
    }

    fn close(&self, tab: TabId) {
        if let Some(window) = tab_window(tab) {
            self.host().act(WindowAction::Close(window));
            return;
        }
        if let Some(panel) = tab_panel(tab) {
            self.set_panels
                .update(|panels| panels.retain(|open| *open != panel));
            return;
        }
        let mut tabs = self.tabs.get_untracked();
        let Some(closed) = tabs.remove(&tab) else {
            return;
        };
        let still_open = tabs.values().any(|item| item.id == closed.id);
        let mut views = self.views.get_untracked();
        if let Some(view) = views.remove(&tab) {
            self.set_views.set(views);
            if view != closed.id {
                self.blocks().set_parent(view, BlockParent::Detached);
            }
        }
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

    pub fn set_phone(&self, phone: bool) {
        self.set_sheet.set(PhoneSheet::Closed);
        self.set_phone.set(phone);
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

    pub(crate) fn new_file_near(self: &Rc<Self>, id: Uuid) {
        let holds_children = self
            .known_type(id)
            .is_some_and(|block_type| self.types().child_edits(block_type).add)
            && self.can_edit(id);
        let parent = match holds_children {
            true => BlockParent::Block(id),
            false => self
                .info(id)
                .map_or(BlockParent::Root, |info| match info.parent {
                    BlockParent::Detached => BlockParent::Root,
                    parent => parent,
                }),
        };
        self.create_in(parent);
    }

    pub(crate) fn create_in(self: &Rc<Self>, parent: BlockParent) {
        self.set_sheet.set(PhoneSheet::Closed);
        self.set_error.set(None);
        let parent = match parent {
            BlockParent::Detached => BlockParent::Root,
            parent => parent,
        };
        let excluded = match parent {
            BlockParent::Block(id) => vec![id.into_bytes()],
            BlockParent::Root | BlockParent::Detached => Vec::new(),
        };
        let picking = Rc::downgrade(self);
        self.editor.pick_block(
            BlockFilter {
                name: "Block".to_owned(),
                block_types: Vec::new(),
                excluded,
                templates: false,
                place: Some(parent.encode()),
            },
            move |picked: Result<PickedBlock, String>| {
                let Some(workspace) = picking.upgrade() else {
                    return;
                };
                let picked = match picked {
                    Ok(picked) => picked,
                    Err(error) => {
                        workspace.set_error.set(Some(error));
                        return;
                    }
                };
                let container = match parent {
                    BlockParent::Block(id) => Some(id),
                    BlockParent::Root | BlockParent::Detached => None,
                };
                if !picked.placed {
                    match container {
                        Some(id) => workspace.host().place_block(
                            picked.id,
                            picked.block_type,
                            id,
                            picked.linked,
                        ),
                        None if !picked.linked => {
                            workspace.blocks().set_parent(picked.id, BlockParent::Root);
                        }
                        None => {}
                    }
                }
                workspace.open(
                    TabItem {
                        id: picked.id,
                        block_type: picked.block_type,
                    },
                    container,
                );
            },
        );
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
                place: None,
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

#[component]
pub fn BlockTab(workspace: Rc<Workspace>, tab: TabId) -> DockEntry<TabId> {
    let titles = workspace.titles.clone();
    let title = create_memo(move || {
        titles.with(|titles| {
            titles
                .get(&tab)
                .cloned()
                .unwrap_or_else(|| "Untitled".to_owned())
        })
    });
    let naming = Rc::clone(&workspace);
    let icon = create_memo(move || {
        naming
            .tab(tab)
            .and_then(|item| naming.label(item.id, item.block_type).icon)
            .unwrap_or_default()
            .to_owned()
    });
    let closing = Rc::clone(&workspace);
    view! {
        <DockTab id=tab title icon on_close={move || closing.close(tab)}>
            <BlockPanel workspace={Rc::clone(&workspace)} tab />
        </DockTab>
    }
}

#[component]
pub fn WindowTab(workspace: Rc<Workspace>, window: HostWindowId) -> DockEntry<TabId> {
    let windows = workspace.windows.clone();
    let name = create_memo(move || {
        windows.with(|windows| {
            windows
                .iter()
                .find(|listed| listed.id == window)
                .map_or_else(|| "Window".to_owned(), window_title)
        })
    });
    let listed = workspace.windows.clone();
    let responding = create_memo(move || {
        listed.with(|windows| {
            windows
                .iter()
                .find(|listed| listed.id == window)
                .is_none_or(|listed| listed.responding)
        })
    });
    let title = create_memo(clone!(name responding -> move || match responding.get() {
        true => name.get(),
        false => format!("{} (not responding)", name.get()),
    }));
    let listed = workspace.windows.clone();
    let fullscreen = create_memo(move || {
        listed.with(|windows| {
            let area = windows
                .iter()
                .find(|listed| listed.id == window)?
                .fullscreen?;
            Some(Rect::from_min_size(
                pos2(area.x, area.y),
                vec2(area.width, area.height),
            ))
        })
    });
    let listed = workspace.windows.clone();
    let focused = create_memo(move || {
        listed.with(|windows| {
            windows
                .iter()
                .any(|listed| listed.id == window && listed.focused)
        })
    });
    let closing = Rc::clone(&workspace);
    let editor = workspace.editor().clone();
    view! {
        <DockTab
            id={window_tab(window)}
            title
            icon=ICON_WEB_ASSET
            on_close={move || closing.close(window_tab(window))}
        >
            <WindowPanel
                editor={editor.clone()}
                window
                fullscreen={fullscreen.clone()}
                name={name.clone()}
                responding={responding.clone()}
                focused={focused.clone()}
            />
        </DockTab>
    }
}

#[component]
pub fn DialogWindow(workspace: Rc<Workspace>, window: HostWindowId) -> DockNode<TabId> {
    let rect = workspace
        .windows
        .get_untracked()
        .iter()
        .find(|listed| listed.id == window)
        .map(dialog_window)
        .unwrap_or_else(|| panel_window(HostPanel::BlockStack));
    let key = format!("window.{}", window.0);
    view! {
        <DockWindow id={key.clone()} rect>
            <DockPane id={key}>
                <WindowTab workspace window />
            </DockPane>
        </DockWindow>
    }
}

#[component]
pub fn PanelWindow(workspace: Rc<Workspace>, panel: HostPanel) -> DockNode<TabId> {
    let key = format!("panel.{panel:?}");
    let closing = Rc::clone(&workspace);
    let editor = workspace.editor().clone();
    view! {
        <DockWindow id={key.clone()} rect={panel_window(panel)}>
            <DockPane id={key}>
                <DockTab
                    id={panel_tab(panel)}
                    title={panel.title()}
                    icon={panel_icon(panel)}
                    on_close={move || closing.close(panel_tab(panel))}
                >
                    <HostPanelView editor={editor.clone()} panel />
                </DockTab>
            </DockPane>
        </DockWindow>
    }
}

#[component]
pub fn Failure(failed: Memo<bool>, reason: Memo<String>) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame visible={failed}>
            <Caption content={reason} color={theme.danger.clone()} />
        </Frame>
    }
}

#[component]
pub fn PanelStatus(state: ReadSignal<ChildState>, loading: String) -> NodeId {
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
