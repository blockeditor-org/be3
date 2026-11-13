use be_sdk_beui::be_block::profile::RECENTS;
use be_sdk_beui::be_block::{
    BlockContent, BlockMetadata, Home, PREFERRED_EDITOR, Recents, VIEW_EDITOR, WORKSPACE_TYPE,
};
use be_sdk_beui::be_block::{EditorView, EditorViewContent, ViewState};
use be_type_settings::EditorSettings;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use be_sdk_beui::be_sdk_ui::{BlockCatalog, BlockLabel, BlockTypes};
use be_sdk_beui::beui::dock::{
    DockEntry, DockNode, DockPane, DockTab, DockWindow, DockingLayout, TabId,
};
use be_sdk_beui::beui::dock_host::styled::DockHostLayout;
use be_sdk_beui::beui::icons::{Glyph, ICON_APP_WINDOW, ICON_MONITOR};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{NodeId, Rect};
use be_sdk_beui::{
    AccessLevel, BlockFilter, ChildState, ContactsWatch, DisplaysWatch, Editor, EditorHost,
    Failure, FocusedBlock, HostPanel, HostProgram, HostWindow, HostWindowId, NotifyMode,
    NotifyModesWatch, PickAnchor, PickedBlock, ProgramsWatch, Pushed, ShellDialog, WindowsWatch,
};
use be_sdk_beui::{BlockInfo, BlockList, BlockParent, BlockQuery, Blocks, ChildTarget};
use uuid::Uuid;

use super::dialogs::OpenDialog;
use super::host_panel::{HostPanelView, panel_icon, panel_tab, panel_window, tab_panel};
use super::panel::BlockPanel;
use super::program::{Heard, Program, Programs, listed_windows};
use super::saved::{self, CLOSED_VIEWS, ClosedView, LAYOUT, MAX_CLOSED_VIEWS, TITLES};
use super::share::{Share, ShareAction};
use super::tab::TabItem;

pub const SHELL_BARS: u64 = 1 << 42;
const FIRST_BLOCK_TAB: u64 = 2;
const MAX_OPENED_VIA_HOPS: usize = 64;
const PANEL_PADDING: Space = Space::Lg;
const PANEL_SPACING: Space = Space::Sm;

type Tabs = HashMap<TabId, TabItem>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Focus {
    pub shown: Option<TabId>,
    pub active: Option<TabItem>,
}

struct State {
    layout: DockingLayout<TabId>,
    host: DockHostLayout,
    restored: Cell<bool>,
    handles: RefCell<HashMap<Uuid, BlockList>>,
    block_types: RefCell<HashMap<Uuid, Uuid>>,
    opened_via: RefCell<HashMap<Uuid, Uuid>>,
    next_tab: Cell<u64>,
    active: Cell<Option<Uuid>>,
    roots: RefCell<Option<(BlockList, BlockList)>>,
    saved_titles: RefCell<HashMap<TabId, String>>,
    next_ask: Cell<u64>,
    share_asks: RefCell<Vec<be_sdk_beui::Task>>,
    /// The mode changes the host has not answered yet, a block's last one each.
    notify_sets: RefCell<HashMap<Uuid, be_sdk_beui::Task>>,
}

#[derive(Clone, Copy)]
pub struct Workspace {
    state: Stored<State>,
    editor: Editor,
    tabs: Signal<Tabs>,
    views: Signal<HashMap<TabId, Uuid>>,
    pub(crate) titles: Signal<HashMap<TabId, String>>,
    simulated: Signal<HashMap<Uuid, AccessLevel>>,
    /// The account's notification modes as the host publishes them (`NotifyModesWatch`), none
    /// until the server has said.
    notify_modes: Memo<Option<HashMap<Uuid, NotifyMode>>>,
    /// Modes this workspace set, shown until the host publishes them: each with whether the
    /// host has answered, after which the next published modes take its place.
    setting_modes: Signal<HashMap<Uuid, (NotifyMode, bool)>>,
    error: Signal<Option<String>>,
    routes: Signal<u64>,
    pub(crate) phone: Signal<bool>,
    focus: Signal<Focus>,
    pub(crate) dialog: Signal<Option<OpenDialog>>,
    pub(crate) share: Signal<Option<Share>>,
    pub(crate) windows: Memo<Vec<HostWindow>>,
    pub(crate) programs: Signal<Programs>,
    pub(crate) heard: Signal<HashMap<TabId, Heard>>,
    pub(crate) ready: Signal<bool>,
    pub(crate) ending: Signal<bool>,
    pub(crate) catalog: Memo<Vec<HostProgram>>,
    pub(crate) launchable: Memo<bool>,
    pub(crate) screens: Memo<Vec<Rect>>,
    unreadable: Signal<Option<String>>,
    panels: Signal<Vec<HostPanel>>,
    history_request: Signal<Option<be_sdk_beui::HistoryRequest>>,
    landing_for: Signal<Option<Uuid>>,
    default_editors: Memo<EditorSettings>,
}

impl Workspace {
    pub fn new(editor: Editor) -> Self {
        provide_context(beui::dock::DockKeeper::none());
        let tabs = create_signal(Tabs::new());
        let views = create_signal(HashMap::new());
        let titles = create_signal(HashMap::new());
        let simulated = create_signal(HashMap::new());
        let error = create_signal(None);
        let routes = create_signal(0);
        let phone = create_signal(false);
        let dialog = create_signal(None);
        let share = create_signal(None);
        let panels = create_signal(Vec::new());
        let entries = editor.watch::<WindowsWatch>();
        let windows = create_memo(move || entries.with(|entries| listed_windows(entries)));
        let listing = editor.watch::<ProgramsWatch>();
        let catalog = create_memo(move || listing.with(|listing| listing.programs.clone()));
        let displays = editor.watch::<DisplaysWatch>();
        let screens = create_memo(move || {
            displays.with(|displays| displays.iter().map(|display| display.rect).collect())
        });
        let default_editors = editor.effective_settings::<EditorSettings>();
        let workspace = Self {
            state: store(State {
                layout: DockingLayout::new(),
                host: DockHostLayout::new(),
                restored: Cell::new(false),
                handles: RefCell::new(HashMap::new()),
                block_types: RefCell::new(HashMap::new()),
                opened_via: RefCell::new(HashMap::new()),
                next_tab: Cell::new(FIRST_BLOCK_TAB),
                active: Cell::new(None),
                roots: RefCell::new(None),
                saved_titles: RefCell::new(HashMap::new()),
                next_ask: Cell::new(0),
                share_asks: RefCell::new(Vec::new()),
                notify_sets: RefCell::new(HashMap::new()),
            }),
            editor,
            tabs,
            views,
            titles,
            simulated,
            notify_modes: {
                let watched = editor.watch::<NotifyModesWatch>();
                create_memo(move || {
                    watched.with(|modes| {
                        modes.as_ref().map(|modes| {
                            modes
                                .iter()
                                .map(|row| (Uuid::from_bytes(row.block), row.mode))
                                .collect()
                        })
                    })
                })
            },
            setting_modes: create_signal(HashMap::new()),
            error,
            routes,
            phone,
            focus: create_signal(Focus::default()),
            default_editors,
            dialog,
            share,
            windows,
            programs: create_signal(Programs::new()),
            heard: create_signal(HashMap::new()),
            ready: create_signal(false),
            ending: create_signal(false),
            catalog,
            launchable: editor.offered::<ProgramsWatch>(),
            screens,
            unreadable: create_signal(None),
            panels,
            history_request: create_signal(None),
            landing_for: create_signal(None),
        };
        create_effect(move || {
            workspace.place_windows();
        });
        // Modes the host answered for give way to the next modes it publishes, which hold them.
        create_effect(move || {
            workspace.notify_modes.with(|_| ());
            untrack(|| {
                workspace
                    .setting_modes
                    .update(|modes| modes.retain(|_, (_, answered)| !*answered));
            });
        });
        workspace.list_programs_for_placeholders();
        create_effect(move || {
            workspace.open_landing();
        });
        let shows = workspace.editor.pushed(Pushed::Shows);
        create_effect(move || {
            shows.get();
            untrack(|| workspace.show_requested());
        });
        create_binding(move || {
            workspace.restore();
        });
        create_effect(move || {
            workspace.save();
        });
        create_effect(move || {
            workspace.refresh_titles();
        });
        create_effect(move || {
            workspace.report_focus();
        });
        let contacts = workspace.editor.watch::<ContactsWatch>();
        create_effect(move || {
            contacts.with(|_| ());
            untrack(|| workspace.relist_share());
        });
        create_effect(move || {
            workspace.watch_artifacts();
        });
        workspace
    }

    pub(crate) fn next_tab(&self) -> u64 {
        self.state.value().next_tab.get()
    }

    pub(crate) fn set_next_tab(&self, next: u64) {
        self.state.value().next_tab.set(next);
    }

    pub fn tab_title(&self, tab: TabId) -> Memo<String> {
        let titles = self.titles;
        create_memo(move || {
            titles.with(|titles| {
                titles
                    .get(&tab)
                    .cloned()
                    .unwrap_or_else(|| "Untitled".to_owned())
            })
        })
    }

    pub fn tab_glyph(&self, tab: TabId) -> Memo<Option<Glyph>> {
        let workspace = *self;
        create_memo(move || {
            if let Some(program) = workspace.program(tab) {
                return Some(match program.record.display {
                    true => ICON_MONITOR,
                    false => ICON_APP_WINDOW,
                });
            }
            workspace.tab(tab).and_then(|item| {
                be_sdk_beui::label_glyph(&workspace.label(item.id, item.block_type))
            })
        })
    }

    pub fn editor(&self) -> Editor {
        self.editor
    }

    pub fn layout(&self) -> DockingLayout<TabId> {
        let inner = self.state.value();
        inner.layout
    }

    /// The last version history the host asked to show: the shell shows it where it keeps a
    /// block's history.
    pub fn history_request(&self) -> ReadSignal<Option<be_sdk_beui::HistoryRequest>> {
        self.history_request.read_only()
    }

    pub fn dock_host(&self) -> DockHostLayout {
        let inner = self.state.value();
        inner.host
    }

    pub fn on_dock_host<R>(
        &self,
        tab: TabId,
        act: impl FnOnce(DockHostLayout, TabId) -> R,
    ) -> Option<R> {
        let placed = self.layout().tab_of(&tab)?;
        Some(act(self.dock_host(), placed))
    }

    pub fn phone(&self) -> ReadSignal<bool> {
        self.phone.read_only()
    }

    pub fn error(&self) -> ReadSignal<Option<String>> {
        self.error.read_only()
    }

    pub fn ready(&self) -> ReadSignal<bool> {
        self.ready.read_only()
    }

    pub fn focus(&self) -> ReadSignal<Focus> {
        self.focus.read_only()
    }

    pub fn panels(&self) -> ReadSignal<Vec<HostPanel>> {
        self.panels.read_only()
    }

    pub fn block_tabs(&self) -> Memo<Vec<TabId>> {
        let listed = self.tabs;
        create_memo(move || {
            let mut tabs: Vec<TabId> = listed.with(|tabs| tabs.keys().copied().collect());
            tabs.sort();
            tabs
        })
    }

    pub fn dialogs(&self) -> Memo<Vec<(HostWindowId, HostWindowId)>> {
        let windows = self.windows;
        create_memo(move || {
            windows.with(|windows| {
                windows
                    .iter()
                    .filter_map(|window| Some((window.id, window.parent?)))
                    .collect::<Vec<_>>()
            })
        })
    }

    pub fn holds(&self, id: Uuid) -> bool {
        self.tabs
            .with(|tabs| tabs.values().any(|item| item.id == id))
    }

    pub fn shows(&self, id: Uuid) -> bool {
        self.tabs.with(|tabs| {
            tabs.iter()
                .any(|(tab, item)| item.id == id && !self.is_hidden(*tab))
        })
    }

    pub fn window_rect(&self, tab: TabId) -> Option<Rect> {
        self.on_dock_host(tab, |host, tab| host.window_rect(tab))
            .flatten()
    }

    pub fn is_hidden(&self, tab: TabId) -> bool {
        self.on_dock_host(tab, |host, tab| host.is_hidden(tab))
            .unwrap_or(false)
    }

    pub fn hide(&self, tab: TabId) {
        self.on_dock_host(tab, |host, tab| {
            if untrack(|| host.window_rect(tab)).is_some() {
                host.hide_window(tab);
            }
        });
    }

    pub fn show(&self, tab: TabId) {
        self.layout().show(&tab);
    }

    pub fn open_block(&self, id: Uuid, block_type: Uuid) {
        self.open(TabItem::new(id, block_type), None);
    }

    pub fn host(&self) -> EditorHost {
        self.editor.host()
    }

    pub(crate) fn blocks(&self) -> Blocks {
        self.editor.blocks()
    }

    pub fn info(&self, id: Uuid) -> Option<BlockInfo> {
        let inner = self.state.value();
        let mut handles = inner.handles.borrow_mut();
        let list = handles
            .entry(id)
            .or_insert_with(|| self.blocks().watch(BlockQuery::Block(id)));
        list.read()
            .ok()
            .and_then(|list| list.into_iter().next())
            .or_else(|| self.blocks().info(id))
    }

    pub fn types(&self) -> Rc<BlockCatalog> {
        self.editor.block_types()
    }

    pub fn tab(&self, tab: TabId) -> Option<TabItem> {
        self.tabs.with(|tabs| tabs.get(&tab).copied())
    }

    pub(crate) fn simulated(&self, id: Uuid) -> Option<AccessLevel> {
        self.simulated.with(|simulated| simulated.get(&id).copied())
    }

    fn show_requested(&self) {
        for (block, dialog) in self.host().take_dialog_requests() {
            match dialog {
                // An editor never swaps the block of a share dialog the person has open.
                ShellDialog::Share if self.share.with_untracked(Option::is_some) => {}
                ShellDialog::Share => self.open_dialog(OpenDialog::Share(block)),
                ShellDialog::Close => {
                    let showing: Vec<TabId> = self.tabs.with_untracked(|tabs| {
                        tabs.iter()
                            .filter(|(_, item)| item.id == block)
                            .map(|(tab, _)| *tab)
                            .collect()
                    });
                    for tab in showing {
                        self.close(tab);
                    }
                }
            }
        }
        for panel in self.editor.take_panel_requests() {
            self.show_panel(panel);
        }
        for request in self.host().take_history_requests() {
            let shown = self.tabs.with_untracked(|tabs| {
                tabs.iter()
                    .find(|(_, item)| item.id == request.block_id)
                    .map(|(tab, _)| *tab)
            });
            if let Some(tab) = shown {
                self.layout().show(&tab);
            }
            self.history_request.set(Some(request));
        }
        for request in self.host().take_show_requests() {
            let landing = request.block_type == WORKSPACE_TYPE && request.editor.is_none();
            self.landing_for.set(landing.then_some(request.block_id));
            self.open(
                TabItem::new(request.block_id, request.block_type).edited_with(request.editor),
                request.via,
            );
        }
    }

    fn open_landing(&self) {
        let Some(owner) = self.landing_for.get() else {
            return;
        };
        let home = self.editor.resolved_settings::<Home>(owner);
        let Some(landing) = home.with(|home| home.value().landing.map(|landing| landing.0)) else {
            return;
        };
        let Some(info) = self.info(landing).filter(|info| info.access.can_view()) else {
            return;
        };
        untrack(|| {
            self.landing_for.set(None);
            self.open(TabItem::new(landing, info.block_type), Some(owner));
        });
    }

    pub(crate) fn open_dialog(&self, dialog: OpenDialog) {
        self.forget_share();
        if let OpenDialog::Share(block) = dialog {
            let request = self.list_share(block);
            self.share.set(Some(Share::new(block, request)));
        }
        self.dialog.set(Some(dialog));
    }

    pub(crate) fn open_rename(&self, block: Uuid, anchor: Option<Rect>) {
        self.editor.rename_block(block, anchor);
    }

    pub(crate) fn close_dialog(&self) {
        self.forget_share();
        self.dialog.set(None);
    }

    fn ask_number(&self) -> u64 {
        let state = self.state.value();
        state.next_ask.set(state.next_ask.get() + 1);
        state.next_ask.get()
    }

    fn list_share(&self, block: Uuid) -> u64 {
        let ask = self.ask_number();
        let workspace = *self;
        let task = self
            .editor
            .then(self.host().list_access(block), move |listing| {
                workspace.share_listed(ask, listing);
            });
        self.state.value().share_asks.borrow_mut().push(task);
        ask
    }

    fn forget_share(&self) {
        for task in std::mem::take(&mut *self.state.value().share_asks.borrow_mut()) {
            task.cancel();
        }
        self.share.set(None);
    }

    /// Acts in the share dialog that showed `block`; an action meant for another block than the
    /// one shared now is dropped.
    pub(crate) fn share_action(&self, block: Uuid, action: ShareAction) {
        let Some(mut share) = self.share.get_untracked() else {
            return;
        };
        if share.block != block {
            return;
        }
        if let ShareAction::InviteNew = action {
            if share.inviting.is_none() {
                share.invite_link = None;
                share.error = None;
                let ask = self.ask_number();
                let workspace = *self;
                let asked = self.host().contact_link(share.block, share.pending_access);
                let task = self.editor.then(asked, move |made| {
                    workspace.contact_made(ask, made);
                });
                self.state.value().share_asks.borrow_mut().push(task);
                share.inviting = Some(ask);
            }
            self.share.set(Some(share));
            return;
        }
        let (grants, mut reload) = share.act(action, self.host().account_id());
        for (account, access) in grants {
            let workspace = *self;
            let asked = self.host().set_access(share.block, account, access);
            let task = self
                .editor
                .then(asked, move |set| workspace.access_set(set));
            self.state.value().share_asks.borrow_mut().push(task);
            reload = true;
        }
        if reload {
            share.error = None;
            share.relisting();
            share.request = Some(self.list_share(share.block));
        }
        self.share.set(Some(share));
    }

    fn relist_share(&self) {
        if self.share.with_untracked(Option::is_some) {
            if let Some(block) = self.share.with_untracked(|share| share.as_ref().map(|share| share.block)) {
                self.share_action(block, ShareAction::Refresh);
            }
        }
    }

    /// A change of access the host could not make (offline, it needs the server): the dialog
    /// says why, and the listing it asks for again shows what holds.
    fn access_set(&self, set: Result<(), Failure>) {
        let Err(failure) = set else {
            return;
        };
        let Some(mut share) = self.share.get_untracked() else {
            return;
        };
        share.error = Some(failure.to_string());
        share.relisting();
        share.request = Some(self.list_share(share.block));
        self.share.set(Some(share));
    }

    fn contact_made(&self, ask: u64, made: Result<String, Failure>) {
        let Some(mut share) = self.share.get_untracked() else {
            return;
        };
        if share.inviting != Some(ask) {
            return;
        }
        share.inviting = None;
        match made {
            Ok(link) => share.invite_link = Some(link),
            Err(error) => share.error = Some(error.to_string()),
        }
        self.share.set(Some(share));
    }

    fn share_listed(&self, ask: u64, listing: Result<be_sdk_beui::AccessListing, Failure>) {
        let Some(mut share) = self.share.get_untracked() else {
            return;
        };
        if share.request != Some(ask) {
            return;
        }
        share.listed(listing);
        self.share.set(Some(share));
    }

    pub(crate) fn my_files(&self) -> Option<Uuid> {
        let host = self.host();
        let mine = be_sdk_beui::be_block::my_files(host.organization_id(), host.account_id());
        self.top()
            .0
            .into_iter()
            .any(|block| block.id == mine && be_sdk_beui::sections::is_workspace(&block))
            .then_some(mine)
    }

    fn top(&self) -> (Vec<BlockInfo>, Vec<BlockInfo>) {
        let inner = self.state.value();
        let mut held = inner.roots.borrow_mut();
        let (roots, shared) = held.get_or_insert_with(|| {
            (
                self.blocks().watch(BlockQuery::Roots),
                self.blocks().watch(BlockQuery::Shared),
            )
        });
        (
            roots.read().unwrap_or_default(),
            shared.read().unwrap_or_default(),
        )
    }

    fn show_panel(&self, panel: HostPanel) {
        let inner = self.state.value();
        match self.panels.with_untracked(|panels| panels.contains(&panel)) {
            true => inner.layout.show(&panel_tab(panel)),
            false => self.panels.update(|panels| panels.push(panel)),
        }
    }

    pub(crate) fn view_of(&self, tab: TabId) -> Option<Uuid> {
        self.views.with(|views| views.get(&tab).copied())
    }

    pub fn create_view(&self, editor: Uuid, content: Option<Uuid>) -> Option<Uuid> {
        let profile = self.editor.view_block()?;
        Some(self.blocks().create_with(
            EditorViewContent::CONTENT_TYPE,
            Some(EditorView::document(content).encode()),
            BlockParent::Block(profile),
            BlockMetadata::view(editor, None),
        ))
    }

    /// Keeps a closed tab's `view` among the session's closed views, so opening `block` again in
    /// the editor the view names (`VIEW_EDITOR`) finds it; the oldest past `MAX_CLOSED_VIEWS` is
    /// detached.
    fn keep_closed_view(&self, block: Uuid, view: Uuid) {
        let editor = self
            .info(view)
            .and_then(|info| info.metadata.get(&VIEW_EDITOR))
            .unwrap_or_default();
        let mut closed = saved::closed_views(self.editor.view_state(CLOSED_VIEWS).as_ref());
        let mut dropped = Vec::new();
        closed.retain(|(held, held_view)| {
            let same = held.block == block && held.editor == editor;
            if same && *held_view != view {
                dropped.push(*held_view);
            }
            !same && *held_view != view
        });
        closed.insert(0, (ClosedView { block, editor }, view));
        while closed.len() > MAX_CLOSED_VIEWS {
            if let Some((_, view)) = closed.pop() {
                dropped.push(view);
            }
        }
        self.editor
            .set_view_state(CLOSED_VIEWS, Some(&saved::save_closed_views(closed)));
        for view in dropped {
            self.blocks().set_parent(view, BlockParent::Detached);
        }
    }

    /// The view `block` had when last closed in `editor`, taken back out of the closed views.
    fn reopen_view(&self, block: Uuid, editor: Uuid) -> Option<Uuid> {
        let mut closed = saved::closed_views(self.editor.view_state(CLOSED_VIEWS).as_ref());
        let at = closed
            .iter()
            .position(|(held, _)| held.block == block && held.editor == editor)?;
        let (_, view) = closed.remove(at);
        self.editor
            .set_view_state(CLOSED_VIEWS, Some(&saved::save_closed_views(closed)));
        Some(view)
    }

    /// A view for `item`: the one it was last closed with, else a new one.
    fn view_for(&self, item: &TabItem) -> Option<Uuid> {
        let editor = self.editor_of(item).unwrap_or_default();
        self.reopen_view(item.id, editor)
            .or_else(|| self.create_view(editor, Some(item.id)))
    }

    fn restore(&self) {
        let inner = self.state.value();
        if inner.restored.get() {
            return;
        }
        let Some(view) = self.editor.view_content() else {
            if self.editor.view_block().is_none() {
                self.ready.set(true);
            }
            return;
        };
        let Ok((layout, titles)) = view.read(|held| {
            let root = held.root();
            (root.state(LAYOUT).cloned(), root.state(TITLES).cloned())
        }) else {
            return;
        };
        inner.restored.set(true);
        *inner.saved_titles.borrow_mut() = titles
            .as_ref()
            .map(saved::restore_titles)
            .unwrap_or_default();
        match layout.as_ref().map(saved::restore).transpose() {
            Ok(restored) => untrack(|| self.adopt(restored)),
            Err(error) => {
                let bytes = layout.map_or(0, |layout| layout.bytes.len());
                self.unreadable.set(Some(format!(
                    "The saved layout of session {} could not be read: {error}\n\nIt holds {bytes} bytes, kept unchanged until the layout is reset.",
                    self.editor.block_id()
                )));
            }
        }
        self.ready.set(true);
    }

    pub fn unreadable_layout(&self) -> Memo<Option<String>> {
        let unreadable = self.unreadable;
        create_memo(move || unreadable.get())
    }

    pub fn reset_layout(&self) {
        if self.unreadable.with_untracked(Option::is_none) {
            return;
        }
        untrack(|| self.adopt(None));
        self.unreadable.set(None);
    }

    pub fn exit(&self) {
        self.host().close_editor(self.editor.block_id());
    }

    fn adopt(&self, restored: Option<saved::Restored>) {
        let inner = self.state.value();
        if let Some(restored) = restored {
            let mut tabs = Tabs::new();
            let mut views = HashMap::new();
            for (tab, (item, view)) in restored.tabs {
                self.record_type(item.id, item.block_type);
                tabs.insert(tab, item);
                views.insert(tab, view);
            }
            let programs: Programs = restored
                .programs
                .into_iter()
                .map(|(tab, record)| (tab, Program::restored(record)))
                .collect();
            let mut next = restored
                .next_tab
                .max(FIRST_BLOCK_TAB)
                .max(tabs.keys().map(|tab| tab.value() + 1).max().unwrap_or(0))
                .max(
                    programs
                        .keys()
                        .map(|tab| tab.value() + 1)
                        .max()
                        .unwrap_or(0),
                );
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
            inner.next_tab.set(next);
            let panels: Vec<HostPanel> = restored
                .dock
                .keys()
                .filter_map(|tab| tab_panel(*tab))
                .collect();
            self.tabs.set(tabs);
            self.views.set(views);
            self.panels.set(panels);
            self.programs.set(programs);
            inner.layout.restore(restored.dock);
        }
    }

    fn save(&self) {
        let inner = self.state.value();
        let layout = inner.layout.snapshot();
        let tabs = self.tabs.get();
        let views = self.views.get();
        let titles = self.titles.get();
        let programs: Vec<(TabId, super::program::ProgramRecord)> =
            self.programs.with(|programs| {
                programs
                    .iter()
                    .map(|(tab, program)| (*tab, program.record.clone()))
                    .collect()
            });
        let unreadable = self.unreadable.with(Option::is_some);
        if !inner.restored.get() || unreadable {
            return;
        }
        let state = saved::save(&layout, inner.next_tab.get(), &tabs, &views, programs);
        let titles = saved::save_titles(&titles);
        untrack(|| {
            self.editor.set_view_state(LAYOUT, Some(&state));
            self.editor.set_view_state(TITLES, Some(&titles));
        });
    }

    fn refresh_titles(&self) {
        let types = self.types();
        let inner = self.state.value();
        let saved = inner.saved_titles.borrow();
        let mut titles: HashMap<TabId, String> = self.tabs.with(|tabs| {
            tabs.iter()
                .map(|(tab, item)| {
                    let name = match self.info(item.id) {
                        Some(info) => info.label(types.as_ref()).name,
                        None => saved.get(tab).cloned().unwrap_or_else(|| {
                            BlockLabel::new(types.as_ref(), item.block_type, None, false).name
                        }),
                    };
                    (*tab, name)
                })
                .collect()
        });
        let programs: Vec<TabId> = self
            .programs
            .with(|programs| programs.keys().copied().collect());
        for tab in programs {
            if let Some(title) = self.program_title(tab) {
                titles.insert(tab, title);
            }
        }
        if self.titles.with_untracked(|held| *held != titles) {
            self.titles.set(titles);
        }
    }

    fn report_focus(&self) {
        let inner = self.state.value();
        let shown = match self.phone.get() {
            true => inner.layout.shown(),
            false => inner.layout.focused(),
        };
        let current = shown
            .and_then(|tab| self.tabs.with(|tabs| tabs.get(&tab).copied()))
            .map(|item| item.id);
        self.routes.get();
        let active = current.or_else(|| inner.active.get());
        inner.active.set(active);
        let focus = Focus {
            shown,
            active: self.tabs.with(|tabs| {
                active.and_then(|id| tabs.values().find(|item| item.id == id).copied())
            }),
        };
        if self.focus.with_untracked(|held| *held != focus) {
            untrack(|| self.focus.set(focus));
        }
        let focused = active.and_then(|id| {
            let block_type = inner.block_types.borrow().get(&id).copied()?;
            Some((id, block_type))
        });
        if let Some((id, block_type)) = focused {
            untrack(|| self.remember(id, block_type));
        }
        self.host().report_focus(FocusedBlock {
            block_id: focused.map(|(id, _)| id),
            block_type: focused.map_or_else(Uuid::nil, |(_, block_type)| block_type),
            editor: focus.active.and_then(|item| item.editor),
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
        let inner = self.state.value();
        let opened_via = inner.opened_via.borrow();
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
        let inner = self.state.value();
        let previous = {
            let mut opened_via = inner.opened_via.borrow_mut();
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
        let inner = self.state.value();
        if inner.opened_via.borrow_mut().remove(&id).is_some() {
            self.rerouted();
        }
    }

    fn rerouted(&self) {
        self.routes.update(|routes| *routes += 1);
    }

    pub(crate) fn container_of(&self, id: Uuid) -> Option<Uuid> {
        let inner = self.state.value();
        inner.opened_via.borrow().get(&id).copied()
    }

    pub(crate) fn record_type(&self, id: Uuid, block_type: Uuid) {
        let inner = self.state.value();
        let previous = inner.block_types.borrow_mut().insert(id, block_type);
        if previous != Some(block_type) {
            self.rerouted();
        }
    }

    pub(crate) fn known_type(&self, id: Uuid) -> Option<Uuid> {
        let inner = self.state.value();
        let known = inner.block_types.borrow().get(&id).copied();
        known.or_else(|| self.info(id).map(|info| info.block_type))
    }

    pub(crate) fn record_reference_types(&self, reference: &BlockInfo) {
        self.record_type(reference.id, reference.block_type);
        if let Some(BlockParent::Block(parent)) = reference.parent
            && let Some(parent) = self.blocks().info(parent)
        {
            self.record_type(parent.id, parent.block_type);
        }
    }

    pub fn open(&self, item: TabItem, via: Option<Uuid>) {
        let inner = self.state.value();
        self.record_via(item.id, via);
        self.record_type(item.id, item.block_type);
        if let Some(tab) = self.tab_showing(item.id) {
            if item.editor.is_some() {
                self.switch_editor(tab, item);
            }
            self.show(tab);
            inner.active.set(Some(item.id));
            return;
        }
        let tab = TabId::new(inner.next_tab.get());
        inner.next_tab.set(inner.next_tab.get() + 1);
        let view = match item.block_type == EditorViewContent::CONTENT_TYPE {
            true => Some(item.id),
            false => self.view_for(&item),
        };
        if let Some(view) = view {
            self.views.update(|views| {
                views.insert(tab, view);
            });
        }
        let mut tabs = self.tabs.get_untracked();
        tabs.insert(tab, item);
        self.tabs.set(tabs);
        inner.active.set(Some(item.id));
    }

    fn switch_editor(&self, tab: TabId, item: TabItem) {
        let mut tabs = self.tabs.get_untracked();
        let Some(held) = tabs.get_mut(&tab) else {
            return;
        };
        if held.editor == item.editor {
            return;
        }
        held.editor = item.editor;
        if item.block_type != EditorViewContent::CONTENT_TYPE {
            let mut views = self.views.get_untracked();
            if let Some(view) = views.remove(&tab)
                && view != item.id
            {
                self.keep_closed_view(item.id, view);
            }
            if let Some(view) = self.view_for(&item) {
                views.insert(tab, view);
            }
            self.views.set(views);
        }
        self.tabs.set(tabs);
    }

    pub(crate) fn editor_of(&self, item: &TabItem) -> Option<Uuid> {
        let types = self.types();
        let file_name = self.info(item.id).and_then(|info| info.file_name());
        let file_name = file_name.as_deref();
        let edits = |editor: &Uuid| types.edits(*editor, item.block_type, file_name);
        item.editor
            .filter(edits)
            .or_else(|| {
                (item.block_type == EditorViewContent::CONTENT_TYPE)
                    .then(|| self.info(item.id)?.metadata.get(&VIEW_EDITOR))
                    .flatten()
                    .filter(|editor| !editor.is_nil())
            })
            .or_else(|| {
                self.info(item.id)?
                    .metadata
                    .get(&PREFERRED_EDITOR)
                    .filter(edits)
            })
            .or_else(|| {
                self.default_editors
                    .with(|defaults| defaults.default_editor(item.block_type))
                    .filter(edits)
            })
            .or_else(|| types.own_editor(item.block_type, file_name))
    }

    pub fn tab_showing(&self, id: Uuid) -> Option<TabId> {
        self.tabs.with_untracked(|tabs| {
            tabs.iter()
                .find(|(_, item)| item.id == id)
                .map(|(tab, _)| *tab)
        })
    }

    pub fn close(&self, tab: TabId) {
        if self.close_program(tab) {
            return;
        }
        if let Some(panel) = tab_panel(tab) {
            self.panels
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
            self.views.set(views);
            if view != closed.id {
                self.keep_closed_view(closed.id, view);
            }
        }
        self.tabs.set(tabs);
        if !still_open {
            self.forget(closed.id);
        }
    }

    fn forget(&self, id: Uuid) {
        let inner = self.state.value();
        inner.handles.borrow_mut().remove(&id);
        inner.opened_via.borrow_mut().remove(&id);
        let mut simulated = self.simulated.get_untracked();
        simulated.remove(&id);
        self.simulated.set(simulated);
        if inner.active.get() == Some(id) {
            inner.active.set(None);
            self.rerouted();
        }
        self.host().close_editor(id);
    }

    pub fn set_phone(&self, phone: bool) {
        self.phone.set(phone);
    }

    pub fn can_edit(&self, id: Uuid) -> bool {
        self.info(id).is_none_or(|info| info.access.can_edit())
    }

    pub(crate) fn can_manage(&self, id: Uuid) -> bool {
        self.info(id).is_none_or(|info| info.access.can_manage())
    }

    pub(crate) fn ceiling(&self, id: Uuid) -> AccessLevel {
        let Some(info) = self.info(id) else {
            return AccessLevel::Manage;
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
            Some(AccessLevel::Edit) => AccessLevel::Edit.min(ceiling),
            Some(AccessLevel::Manage) | None => ceiling,
        }
    }

    /// How the account is notified about `block`: a change not yet answered, else what the host
    /// publishes, none until the server has said.
    pub(crate) fn notify_mode(&self, block: Uuid) -> Option<NotifyMode> {
        if let Some((setting, _)) = self.setting_modes.with(|modes| modes.get(&block).copied()) {
            return Some(setting);
        }
        self.notify_modes.with(|modes| {
            modes
                .as_ref()
                .map(|modes| modes.get(&block).copied().unwrap_or(NotifyMode::All))
        })
    }

    /// Sets how the account is notified about `block`, showing it until the host answers; a
    /// change the host refuses goes back to what it publishes, and says why.
    pub(crate) fn set_notify_mode(&self, block: Uuid, mode: NotifyMode) {
        self.setting_modes.update(|modes| {
            modes.insert(block, (mode, false));
        });
        let workspace = *self;
        let asked = self.host().set_notify_mode(block, mode);
        let task = self.editor.then(asked, move |set| match set {
            Ok(()) => workspace.setting_modes.update(|modes| {
                if let Some((shown, answered)) = modes.get_mut(&block)
                    && *shown == mode
                {
                    *answered = true;
                }
            }),
            Err(failure) => {
                workspace.setting_modes.update(|modes| {
                    modes.remove(&block);
                });
                workspace.error.set(Some(format!(
                    "The notifications setting was not changed: {failure}"
                )));
            }
        });
        self.state
            .value()
            .notify_sets
            .borrow_mut()
            .insert(block, task);
    }

    pub(crate) fn simulate(&self, id: Uuid, level: AccessLevel) {
        let mut simulated = self.simulated.get_untracked();
        simulated.insert(id, level);
        self.simulated.set(simulated);
        self.host().simulate_access(id, level);
    }

    pub fn label(&self, id: Uuid, block_type: Uuid) -> BlockLabel {
        let types = self.types();
        self.info(id).map_or_else(
            || BlockLabel::new(types.as_ref(), block_type, None, false),
            |info| info.label(types.as_ref()),
        )
    }

    /// Starts a new file where a file made without a chosen place goes.
    pub fn new_file(&self) {
        self.create_in(BlockParent::Root);
    }

    pub fn new_file_near(&self, id: Uuid) {
        let holds_children = self
            .known_type(id)
            .is_some_and(|block_type| self.types().child_operations(block_type).add)
            && self.can_edit(id);
        let parent = match holds_children {
            true => BlockParent::Block(id),
            false => self
                .info(id)
                .and_then(|info| info.parent)
                .unwrap_or(BlockParent::Detached),
        };
        self.create_in(parent);
    }

    pub(crate) fn create_in(&self, parent: BlockParent) {
        self.error.set(None);
        let container = parent.block();
        // An editor cannot see Unfiled, which the host makes with My files once the
        // organization's blocks have loaded; the host fails a creation committed there
        // before it exists, so this only spares asking first.
        if container.is_none() && self.my_files().is_none() {
            self.error
                .set(Some("Your files are still loading".to_owned()));
            return;
        }
        let parent = container.map_or(BlockParent::Unfiled, BlockParent::Block);
        let excluded = container.map(Uuid::into_bytes).into_iter().collect();
        let workspace = *self;
        self.editor.pick_block(
            BlockFilter {
                name: "Block".to_owned(),
                block_types: Vec::new(),
                excluded,
                templates: false,
                place: Some(parent.encode()),
            },
            PickAnchor::Centre,
            move |picked: Result<PickedBlock, Failure>| {
                if !workspace.state.is_alive() {
                    return;
                }
                let picked = match picked {
                    Ok(picked) => picked,
                    Err(error) => {
                        workspace.error.set(Some(error.to_string()));
                        return;
                    }
                };
                let block_type = Uuid::from_bytes(picked.block_type);
                if !picked.placed
                    && let Some(container) = container
                {
                    workspace
                        .host()
                        .place_block(picked.id(), block_type, container, picked.linked);
                }
                workspace.open(TabItem::new(picked.id(), block_type), container);
                if !picked.named {
                    workspace.open_rename(picked.id(), None);
                }
            },
        );
    }

    pub(crate) fn open_picker(&self, parent: Uuid, anchor: Option<Rect>) {
        self.error.set(None);
        let workspace = *self;
        self.editor.pick_block(
            BlockFilter {
                name: "Block".to_owned(),
                block_types: Vec::new(),
                excluded: vec![parent.into_bytes()],
                templates: false,
                place: None,
            },
            anchor,
            move |picked: Result<PickedBlock, Failure>| {
                if !workspace.state.is_alive() {
                    return;
                }
                match picked {
                    Ok(picked) => workspace.host().place_block(
                        picked.id(),
                        Uuid::from_bytes(picked.block_type),
                        parent,
                        picked.linked,
                    ),
                    Err(error) => workspace.error.set(Some(error.to_string())),
                }
            },
        );
    }
}

#[component]
pub fn BlockTab(
    workspace: Workspace,
    tab: TabId,
    #[prop(default = None)] bar: Option<RenderFn<(), NodeId>>,
    #[prop(default = None)] preview: Prop<Option<ChildTarget>>,
    on_details: Callback<()>,
) -> DockEntry<TabId> {
    let title = workspace.tab_title(tab);
    let icon = workspace.tab_glyph(tab);
    let bar = RenderFn::new(move |()| match bar {
        Some(bar) => bar.call(()),
        None => view! {
            <List spacing=Space::None />
        },
    });
    let group = create_memo(move || {
        workspace
            .tab(tab)
            .map(|item| format!("block:{}", item.block_type))
    });
    view! {
        <DockTab id=tab title icon group on_close={move || workspace.close(tab)}>
            <BlockPanel workspace={workspace} tab bar={move || bar.call(())} preview on_details />
        </DockTab>
    }
}

#[component]
pub fn PanelWindow(workspace: Workspace, panel: HostPanel) -> DockNode<TabId> {
    let key = format!("panel.{panel:?}");
    let editor = workspace.editor();
    view! {
        <DockWindow id={key.clone()} rect={panel_window(panel)}>
            <DockPane id={key}>
                <DockTab
                    id={panel_tab(panel)}
                    title={panel.title()}
                    icon={panel_icon(panel)}
                    on_close={move || workspace.close(panel_tab(panel))}
                >
                    <HostPanelView editor={editor} panel />
                </DockTab>
            </DockPane>
        </DockWindow>
    }
}

#[component]
pub fn Failure(failed: Memo<bool>, reason: Memo<String>) -> DynamicSegment<ListItem> {
    view! {
        <Show condition={failed}>
            <Text style=TextStyle::CAPTION color=TextColor::Danger>{reason}</Text>
        </Show>
    }
}

#[component]
pub fn PanelStatus(state: ReadSignal<ChildState>, loading: String) -> NodeId {
    let shown = create_memo(move || state.with(|state| !state.available || state.error.is_some()));
    let message = create_memo(move || {
        state.with(|state| match &state.error {
            Some(error) => error.clone(),
            None => loading.clone(),
        })
    });
    view! {
        <Layers>
            <Show condition={shown}>
                <Frame padding=PANEL_PADDING>
                    <List spacing=PANEL_SPACING>
                        <Text style=TextStyle::CAPTION>{message}</Text>
                        <Spacer />
                    </List>
                </Frame>
            </Show>
        </Layers>
    }
}
