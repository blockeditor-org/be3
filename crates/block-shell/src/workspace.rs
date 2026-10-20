use block_editor_beui::be_block::profile::{RECENTS, VIEW_EDITORS};
use block_editor_beui::be_block::{
    BlockContent, EditorSettings, EditorView, EditorViewContent, FILES_EDITOR, MY_FILES, Recents,
    ViewState, WorkspaceContent,
};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use block_editor_beui::beui::icons::ICON_WEB_ASSET;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::{
    DockDialog, DockEntry, DockNode, DockPane, DockTab, DockWindow, DockingLayout, TabId,
};
use block_editor_beui::beui::{NodeId, Rect, Vec2, pos2, vec2};
use block_editor_beui::block_ui::{BlockCatalog, BlockLabel, BlockTypes};
use block_editor_beui::{
    AccessLevel, BlockFilter, BlockPick, ChildState, Contacts, Editor, EditorHost, FocusedBlock,
    HostPanel, HostWindow, HostWindowId, HostWindows, PickedBlock, Pushed, ShellDialog,
    WindowAction,
};
use block_editor_beui::{BlockInfo, BlockList, BlockParent, BlockQuery, Blocks};
use uuid::Uuid;

use super::dialogs::OpenDialog;
use super::host_panel::{HostPanelView, panel_icon, panel_tab, panel_window, tab_panel};
use super::kept::{self, Kept, KeptWindow};
use super::panel::BlockPanel;
use super::picker::{Pick, PickAction, PickOutcome};
use super::saved::{self, LAYOUT};
use super::share::{Share, ShareAction};
use super::tab::TabItem;
use super::window::{
    WindowPanel, floating_window, kept_window, shown_size, tab_window, window_picture, window_tab,
    window_title,
};

pub const FILES: TabId = TabId::new(1);
pub const SETTINGS: TabId = TabId::new(1 << 39);
pub const REVISIONS: TabId = TabId::new((1 << 39) + 1);
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

struct State {
    layout: DockingLayout<TabId>,
    restored: Cell<bool>,
    handles: RefCell<HashMap<Uuid, BlockList>>,
    block_types: RefCell<HashMap<Uuid, Uuid>>,
    opened_via: RefCell<HashMap<Uuid, Uuid>>,
    next_tab: Cell<u64>,
    active: Cell<Option<Uuid>>,
    every_block: RefCell<Option<BlockList>>,
}

#[derive(Clone, Copy)]
pub struct Workspace {
    pub(crate) revisions: super::revisions::RevisionHistory,
    state: Stored<State>,
    editor: Editor,
    with_files: bool,
    tabs: Signal<Tabs>,
    views: Signal<HashMap<TabId, Uuid>>,
    titles: Signal<HashMap<TabId, String>>,
    simulated: Signal<HashMap<Uuid, AccessLevel>>,
    error: Signal<Option<String>>,
    files: Signal<Option<Uuid>>,
    routes: Signal<u64>,
    pub(crate) phone: Signal<bool>,
    pub(crate) sheet: Signal<PhoneSheet>,
    pub(crate) picks: Signal<Vec<Pick>>,
    pub(crate) dialog: Signal<Option<OpenDialog>>,
    pub(crate) share: Signal<Option<Share>>,
    windows: Memo<Vec<HostWindow>>,
    keeps_windows: bool,
    kept: Signal<Kept>,
    desk: Memo<kept::Desk>,
    held: Signal<bool>,
    panels: Signal<Vec<HostPanel>>,
    default_editors: Memo<EditorSettings>,
}

impl Workspace {
    pub fn new(editor: Editor, with_files: bool, keeps_windows: bool) -> Self {
        let tabs = create_signal(Tabs::new());
        let views = create_signal(HashMap::new());
        let titles = create_signal(HashMap::new());
        let simulated = create_signal(HashMap::new());
        let error = create_signal(None);
        let files = create_signal(None);
        let routes = create_signal(0);
        let phone = create_signal(false);
        let sheet = create_signal(PhoneSheet::Closed);
        let picks = create_signal(Vec::new());
        let dialog = create_signal(None);
        let share = create_signal(None);
        let panels = create_signal(Vec::new());
        let windows = editor.host_value::<HostWindows>();
        let kept = create_signal(Kept::default());
        let desk = create_memo(move || {
            kept.with(|kept| windows.with(|windows| kept::sort_out(kept, windows)))
        });
        let held = create_signal(false);
        let default_editors = editor.effective_settings::<EditorSettings>();
        let workspace = Self {
            revisions: super::revisions::RevisionHistory::new(editor),
            state: store(State {
                layout: DockingLayout::new(),
                restored: Cell::new(false),
                handles: RefCell::new(HashMap::new()),
                block_types: RefCell::new(HashMap::new()),
                opened_via: RefCell::new(HashMap::new()),
                next_tab: Cell::new(FIRST_BLOCK_TAB),
                active: Cell::new(None),
                every_block: RefCell::new(None),
            }),
            editor,
            with_files,
            tabs,
            views,
            titles,
            simulated,
            error,
            files,
            routes,
            phone,
            sheet,
            picks,
            default_editors,
            dialog,
            share,
            windows,
            keeps_windows,
            kept,
            desk,
            held,
            panels,
        };
        let shows = workspace.editor.pushed(Pushed::Shows);
        create_effect(move || {
            shows.get();
            untrack(|| workspace.show_requested());
        });
        create_effect(move || {
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
        let contacts = workspace.editor.host_value::<Contacts>();
        create_effect(move || {
            contacts.with(|_| ());
            untrack(|| workspace.relist_share());
        });
        workspace.editor.on_reply(move || {
            if workspace.state.is_alive() {
                workspace.share_listed();
                workspace.imports_answered();
            }
        });
        create_effect(move || {
            workspace.watch_artifacts();
        });
        let seen = RefCell::new(HashMap::new());
        create_effect(move || {
            workspace.show_raised(&mut seen.borrow_mut());
        });
        create_effect(move || {
            workspace.settle_kept();
        });
        workspace
    }

    fn settle_kept(&self) {
        let desk = self.desk.get();
        let kept = self.kept.get();
        if !kept.restored {
            return;
        }
        self.windows.with(|windows| {
            for (window, key) in &desk.claims {
                let fullscreen = kept
                    .records
                    .iter()
                    .any(|record| record.key == *key && record.fullscreen);
                if fullscreen {
                    self.host().act(WindowAction::Fullscreen {
                        window: *window,
                        fullscreen: true,
                    });
                }
            }
            let next = kept::settle(&kept, &desk, windows);
            untrack(|| self.kept.set(next));
        });
    }

    pub fn desk(&self) -> Memo<Vec<HostWindowId>> {
        let desk = self.desk;
        create_memo(move || desk.with(|desk| desk.keys.clone()))
    }

    pub fn bound(&self, key: HostWindowId) -> Memo<Option<HostWindowId>> {
        let desk = self.desk;
        create_memo(move || desk.with(|desk| desk.window(key)))
    }

    fn window_key(&self, window: HostWindowId) -> HostWindowId {
        self.desk.with_untracked(|desk| desk.key(window))
    }

    pub fn placeholders(&self) -> Memo<Vec<HostWindowId>> {
        let desk = self.desk;
        create_memo(move || desk.with(|desk| desk.placeholders.clone()))
    }

    pub fn placeholder(&self, key: HostWindowId) -> Option<KeptWindow> {
        self.kept.with(|kept| {
            kept.records
                .iter()
                .find(|record| record.key == key)
                .cloned()
        })
    }

    pub fn dismiss(&self, key: HostWindowId) {
        if !self
            .desk
            .with_untracked(|desk| desk.placeholders.contains(&key))
        {
            return;
        }
        self.kept
            .update(|kept| kept.records.retain(|record| record.key != key));
    }

    pub fn hold_saving(&self, held: bool) {
        self.held.set(held);
    }

    fn show_raised(&self, seen: &mut HashMap<HostWindowId, u64>) {
        let inner = self.state.value();
        let (raised, parents): (
            HashMap<HostWindowId, u64>,
            HashMap<HostWindowId, HostWindowId>,
        ) = self.windows.with(|windows| {
            (
                windows
                    .iter()
                    .map(|window| (window.id, window.raised))
                    .collect(),
                windows
                    .iter()
                    .filter_map(|window| Some((window.id, window.parent?)))
                    .collect(),
            )
        });
        let forward: Vec<HostWindowId> = raised
            .iter()
            .filter(|(window, count)| seen.get(window).copied().unwrap_or_default() < **count)
            .map(|(window, _)| *window)
            .collect();
        *seen = raised;
        untrack(|| {
            for window in forward {
                let mut root = window;
                for _ in 0..parents.len() {
                    match parents.get(&root) {
                        Some(parent) => root = *parent,
                        None => break,
                    }
                }
                inner.layout.show(&window_tab(self.window_key(root)));
            }
        });
    }

    pub fn editor(&self) -> Editor {
        self.editor
    }

    pub fn layout(&self) -> DockingLayout<TabId> {
        let inner = self.state.value();
        inner.layout
    }

    pub fn phone(&self) -> ReadSignal<bool> {
        self.phone.read_only()
    }

    pub fn error(&self) -> ReadSignal<Option<String>> {
        self.error.read_only()
    }

    pub fn files(&self) -> ReadSignal<Option<Uuid>> {
        self.files.read_only()
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

    pub fn windows(&self) -> Memo<Vec<HostWindowId>> {
        let windows = self.windows;
        create_memo(move || {
            windows.with(|windows| {
                windows
                    .iter()
                    .filter(|window| window.parent.is_none())
                    .map(|window| window.id)
                    .collect::<Vec<_>>()
            })
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

    pub fn open_block(&self, id: Uuid, block_type: Uuid) {
        self.open(TabItem::new(id, block_type), None);
    }

    pub(crate) fn host(&self) -> EditorHost {
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
            .into_iter()
            .next()
            .or_else(|| self.blocks().info(id))
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

    fn show_requested(&self) {
        let requested: Vec<Pick> = self
            .host()
            .take_pick_requests()
            .into_iter()
            .map(Pick::new)
            .collect();
        if !requested.is_empty() {
            self.picks.update(|picks| picks.extend(requested));
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
                TabItem::new(request.block_id, request.block_type).edited_with(request.editor),
                request.via,
            );
        }
    }

    pub(crate) fn open_dialog(&self, dialog: OpenDialog) {
        self.forget_share();
        if let OpenDialog::Share(block) = dialog {
            let request = self.host().list_access(block);
            self.share.set(Some(Share::new(block, request)));
        }
        self.dialog.set(Some(dialog));
    }

    pub(crate) fn close_dialog(&self) {
        self.forget_share();
        self.dialog.set(None);
    }

    fn forget_share(&self) {
        if let Some(share) = self.share.get_untracked() {
            for request in share.request.into_iter().chain(share.inviting) {
                self.host().forget_request(request);
            }
        }
        self.share.set(None);
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
        if let ShareAction::InviteNew = action {
            if share.inviting.is_none() {
                share.invite_link = None;
                share.error = None;
                share.inviting = Some(self.host().contact_link(share.block, share.pending_access));
            }
            self.share.set(Some(share));
            return;
        }
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
        self.share.set(Some(share));
    }

    fn relist_share(&self) {
        if self.share.with_untracked(Option::is_some) {
            self.share_action(ShareAction::Refresh);
        }
    }

    fn share_listed(&self) {
        let Some(mut share) = self.share.get_untracked() else {
            return;
        };
        if let Some(made) = share
            .inviting
            .and_then(|request| self.host().take_contact_link(request))
        {
            share.inviting = None;
            match made {
                block_editor_beui::ContactLinkResult::Made(link) => share.invite_link = Some(link),
                block_editor_beui::ContactLinkResult::Failed(error) => share.error = Some(error),
            }
            self.share.set(Some(share.clone()));
        }
        let Some(listing) = share
            .request
            .and_then(|request| self.host().take_access_listing(request))
        else {
            return;
        };
        share.listed(listing);
        self.share.set(Some(share));
    }

    pub(crate) fn my_files(&self) -> Option<Uuid> {
        let account = self.host().account_id();
        self.every_block()
            .into_iter()
            .filter(|block| {
                block.block_type == WorkspaceContent::CONTENT_TYPE
                    && block.parent == Some(BlockParent::Root)
                    && block.author == Some(account)
                    && block.metadata.get(&MY_FILES) == Some(true)
            })
            .map(|block| block.id)
            .min()
    }

    pub(crate) fn every_block(&self) -> Vec<BlockInfo> {
        let inner = self.state.value();
        let mut held = inner.every_block.borrow_mut();
        held.get_or_insert_with(|| self.blocks().watch(BlockQuery::All))
            .read()
    }

    pub(crate) fn pick(&self, pick: u64) -> Option<Pick> {
        self.picks
            .with(|picks| picks.iter().find(|held| held.pick == pick).cloned())
    }

    pub(crate) fn pick_action(&self, pick: u64, action: PickAction) {
        let Some(mut held) = self.pick(pick) else {
            return;
        };
        if let PickAction::Import = action {
            if held.importing.is_none() {
                held.importing = Some(self.host().import_blocks(held.import_parent()));
                self.resolve(PickOutcome::Open(Box::new(held)));
            }
            return;
        }
        match held.act(action) {
            PickOutcome::Open(mut held) => {
                self.send_commit(&mut held);
                self.resolve(PickOutcome::Open(held));
            }
            answered => self.resolve(answered),
        }
    }

    fn imports_answered(&self) {
        let importing = self.picks.with_untracked(|picks| {
            picks
                .iter()
                .filter_map(|held| Some((held.pick, held.importing?)))
                .collect::<Vec<_>>()
        });
        let host = self.host();
        for (pick, request) in importing {
            let Some(imported) = host.take_import(request) else {
                continue;
            };
            if let Some(held) = self.pick(pick) {
                self.resolve(held.imported(imported));
            }
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
                    self.picks.update(|picks| {
                        if let Some(slot) = picks.iter_mut().find(|slot| slot.pick == held.pick) {
                            *slot = *held;
                        }
                    });
                }
            }
            PickOutcome::Answered(pick, answer, into) => {
                self.picks
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
        let inner = self.state.value();
        match self.panels.with_untracked(|panels| panels.contains(&panel)) {
            true => inner.layout.show(&panel_tab(panel)),
            false => self.panels.update(|panels| panels.push(panel)),
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
            block_editor_beui::be_block::BlockMetadata::default(),
        ))
    }

    fn restore(&self) {
        let inner = self.state.value();
        if inner.restored.get() {
            return;
        }
        let Some(view) = self.editor.view_content() else {
            return;
        };
        let Some(layout) = view.read(|held| held.root().state(LAYOUT).cloned()) else {
            return;
        };
        inner.restored.set(true);
        untrack(|| self.adopt(layout.as_ref().and_then(saved::restore)));
    }

    fn adopt(&self, restored: Option<saved::Restored>) {
        let inner = self.state.value();
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
            inner.next_tab.set(next);
            let panels: Vec<HostPanel> = restored
                .dock
                .keys()
                .filter_map(|tab| tab_panel(*tab))
                .collect();
            batch(|| {
                self.tabs.set(tabs);
                self.views.set(views);
                self.panels.set(panels);
                self.keep(restored.windows);
                inner.layout.restore(restored.dock);
            });
        }
        if !self.kept.with_untracked(|kept| kept.restored) {
            self.keep(Vec::new());
        }
        if self.with_files {
            let files = files.or_else(|| self.create_view(FILES_EDITOR, None));
            self.files.set(files);
        }
    }

    fn keep(&self, records: Vec<KeptWindow>) {
        if !self.keeps_windows {
            return;
        }
        let seen = self
            .windows
            .with_untracked(|windows| windows.iter().map(|window| window.id).collect());
        self.kept.set(Kept {
            restored: true,
            records,
            seen,
        });
    }

    fn save(&self) {
        let inner = self.state.value();
        let layout = inner.layout.snapshot();
        let tabs = self.tabs.get();
        let views = self.views.get();
        let files = self.files.get();
        let windows = match self.keeps_windows {
            true => self.kept.with(|kept| {
                self.desk.with(|desk| {
                    self.windows
                        .with(|windows| kept::saved(kept, desk, windows))
                })
            }),
            false => Vec::new(),
        };
        if !inner.restored.get() || self.held.get() {
            return;
        }
        let state = saved::save(&layout, inner.next_tab.get(), files, &tabs, &views, windows);
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
        self.titles.set(titles);
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
        let history_target = self.tabs.with(|tabs| match shown {
            Some(tab) if tab == FILES || tab == SETTINGS || tab == REVISIONS => self
                .revisions
                .target()
                .filter(|item| tabs.values().any(|open| open.id == item.id)),
            Some(tab) => tabs.get(&tab).copied(),
            None => active.and_then(|id| tabs.values().find(|item| item.id == id).copied()),
        });
        self.revisions.follow(history_target);
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

    pub(crate) fn open(&self, item: TabItem, via: Option<Uuid>) {
        let inner = self.state.value();
        self.record_via(item.id, via);
        self.record_type(item.id, item.block_type);
        if let Some(tab) = self.tab_showing(item.id) {
            if item.editor.is_some() {
                self.switch_editor(tab, item);
            }
            inner.layout.show(&tab);
            inner.active.set(Some(item.id));
            return;
        }
        let tab = TabId::new(inner.next_tab.get());
        inner.next_tab.set(inner.next_tab.get() + 1);
        let view = match VIEW_EDITORS.contains(&item.block_type) {
            true => Some(item.id),
            false => self.create_view(item.block_type, Some(item.id)),
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
        if !VIEW_EDITORS.contains(&item.block_type) {
            let mut views = self.views.get_untracked();
            if let Some(view) = views.remove(&tab)
                && view != item.id
            {
                self.blocks().set_parent(view, BlockParent::Detached);
            }
            if let Some(view) = self.create_view(item.block_type, Some(item.id)) {
                views.insert(tab, view);
            }
            self.views.set(views);
        }
        self.tabs.set(tabs);
    }

    pub(crate) fn editor_of(&self, item: &TabItem) -> Option<Uuid> {
        let types = self.types();
        item.editor
            .filter(|editor| {
                types
                    .editor(*editor)
                    .is_some_and(|entry| entry.edits(item.block_type))
            })
            .or_else(|| {
                self.default_editors
                    .with(|defaults| defaults.default_editor(item.block_type))
                    .filter(|editor| {
                        types
                            .editor(*editor)
                            .is_some_and(|entry| entry.edits(item.block_type))
                    })
            })
            .or_else(|| types.built_in_editor(item.block_type))
    }

    pub(crate) fn set_sheet(&self, sheet: PhoneSheet) {
        self.sheet.set(sheet);
    }

    pub(crate) fn dismiss_sheet(&self, sheet: PhoneSheet) {
        if self.sheet.get_untracked() == sheet {
            self.sheet.set(PhoneSheet::Closed);
        }
    }

    pub(crate) fn tab_showing(&self, id: Uuid) -> Option<TabId> {
        self.tabs.with_untracked(|tabs| {
            tabs.iter()
                .find(|(_, item)| item.id == id)
                .map(|(tab, _)| *tab)
        })
    }

    fn close(&self, tab: TabId) {
        if let Some(key) = tab_window(tab) {
            match self.desk.with_untracked(|desk| desk.window(key)) {
                Some(window) => self.host().act(WindowAction::Close(window)),
                None if self.keeps_windows => self.dismiss(key),
                None => self.host().act(WindowAction::Close(key)),
            }
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
                self.blocks().set_parent(view, BlockParent::Detached);
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
        self.sheet.set(PhoneSheet::Closed);
        self.phone.set(phone);
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
        let mut simulated = self.simulated.get_untracked();
        simulated.insert(id, level);
        self.simulated.set(simulated);
        self.host().simulate_access(id, level);
    }

    pub(crate) fn label(&self, id: Uuid, block_type: Uuid) -> BlockLabel {
        let types = self.types();
        self.info(id).map_or_else(
            || BlockLabel::new(types.as_ref(), block_type, None, false),
            |info| info.label(types.as_ref()),
        )
    }

    pub(crate) fn new_file_near(&self, id: Uuid) {
        let holds_children = self
            .known_type(id)
            .is_some_and(|block_type| self.types().child_edits(block_type).add)
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
        self.sheet.set(PhoneSheet::Closed);
        self.error.set(None);
        let parent = match parent {
            BlockParent::Root | BlockParent::Detached => self
                .my_files()
                .map_or(BlockParent::Root, BlockParent::Block),
            parent => parent,
        };
        let excluded = match parent {
            BlockParent::Block(id) => vec![id.into_bytes()],
            BlockParent::Root | BlockParent::Detached => Vec::new(),
        };
        let workspace = *self;
        self.editor.pick_block(
            BlockFilter {
                name: "Block".to_owned(),
                block_types: Vec::new(),
                excluded,
                templates: false,
                place: Some(parent.encode()),
            },
            move |picked: Result<PickedBlock, String>| {
                if !workspace.state.is_alive() {
                    return;
                }
                let picked = match picked {
                    Ok(picked) => picked,
                    Err(error) => {
                        workspace.error.set(Some(error));
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
                workspace.open(TabItem::new(picked.id, picked.block_type), container);
            },
        );
    }

    pub(crate) fn open_picker(&self, parent: Uuid) {
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
            move |picked: Result<PickedBlock, String>| {
                if !workspace.state.is_alive() {
                    return;
                }
                match picked {
                    Ok(picked) => workspace.host().place_block(
                        picked.id,
                        picked.block_type,
                        parent,
                        picked.linked,
                    ),
                    Err(error) => workspace.error.set(Some(error)),
                }
            },
        );
    }
}

#[component]
pub fn BlockTab(workspace: Workspace, tab: TabId) -> DockEntry<TabId> {
    let titles = workspace.titles;
    let title = create_memo(move || {
        titles.with(|titles| {
            titles
                .get(&tab)
                .cloned()
                .unwrap_or_else(|| "Untitled".to_owned())
        })
    });
    let icon = create_memo(move || {
        workspace
            .tab(tab)
            .and_then(|item| {
                block_editor_beui::label_glyph(&workspace.label(item.id, item.block_type))
            })
            .unwrap_or_default()
            .to_owned()
    });
    view! {
        <DockTab id=tab title icon on_close={move || workspace.close(tab)}>
            <BlockPanel workspace={workspace} tab />
        </DockTab>
    }
}

#[component]
pub fn WindowTab(
    workspace: Workspace,
    key: HostWindowId,
    #[prop(default = false)] offers_fullscreen: bool,
    placeholder: Option<RenderFn<HostWindowId>>,
) -> DockEntry<TabId> {
    let bound = workspace.bound(key);
    let windows = workspace.windows;
    let info = create_memo(move || {
        let window = bound.get()?;
        windows.with(|windows| windows.iter().find(|listed| listed.id == window).cloned())
    });
    let name = create_memo(move || match info.get() {
        Some(info) => window_title(&info),
        None => workspace
            .placeholder(key)
            .map(|kept| match kept.title.is_empty() {
                true => kept.app_id,
                false => kept.title,
            })
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| "Window".to_owned()),
    });
    let responding = create_memo(move || info.get().is_none_or(|info| info.responding));
    let title = create_memo(move || match responding.get() {
        true => name.get(),
        false => format!("{} (not responding)", name.get()),
    });
    let fullscreen = create_memo(move || {
        let area = info.get()?.fullscreen?;
        Some(Rect::from_min_size(
            pos2(area.x, area.y),
            vec2(area.width, area.height),
        ))
    });
    let focused = create_memo(move || info.get().is_some_and(|info| info.focused));
    let size = create_memo(move || {
        info.get()
            .map(|info| vec2(info.size.width, info.size.height))
    });
    let picture = create_memo(move || info.get()?.icon.as_ref().and_then(window_picture));
    let editor = workspace.editor();
    let tab = window_tab(key);
    view! {
        <DockTab id={tab} title icon=ICON_WEB_ASSET picture on_close={move || workspace.close(tab)}>
            <Layers>
                <Dynamic value={bound}>|bound: Option<HostWindowId>| {
                    match (bound, placeholder) {
                        (Some(window), _) => view! {
                            <WindowPanel
                                editor={editor}
                                window
                                offers_fullscreen
                                fullscreen={fullscreen}
                                name={name}
                                responding={responding}
                                focused={focused}
                                size={size}
                            />
                        },
                        (None, Some(placeholder)) => placeholder.call(key),
                        (None, None) => view! {
                            <Frame />
                        },
                    }
                }</Dynamic>
            </Layers>
        </DockTab>
    }
}

#[component]
pub fn FloatingWindow(
    workspace: Workspace,
    key: HostWindowId,
    placeholder: RenderFn<HostWindowId>,
) -> DockNode<TabId> {
    let window = workspace.bound(key).get_untracked();
    let rect = workspace
        .windows
        .get_untracked()
        .iter()
        .find(|listed| Some(listed.id) == window)
        .map_or_else(|| kept_window(key), floating_window);
    let pane = format!("window.{}", key.0);
    view! {
        <DockWindow id={pane.clone()} rect>
            <DockPane id={pane}>
                <WindowTab workspace key offers_fullscreen=true placeholder={placeholder} />
            </DockPane>
        </DockWindow>
    }
}

#[component]
pub fn AttachedWindow(
    workspace: Workspace,
    window: HostWindowId,
    parent: HostWindowId,
) -> DockNode<TabId> {
    let listed = workspace.windows;
    let info = create_memo(move || {
        listed.with(|windows| windows.iter().find(|listed| listed.id == window).cloned())
    });
    let title = create_memo(move || {
        info.get()
            .map_or_else(|| "Window".to_owned(), |info| window_title(&info))
    });
    let size = create_memo(move || {
        info.get().map_or(Vec2::ZERO, |info| {
            shown_size(vec2(info.size.width, info.size.height))
        })
    });
    let followed = create_memo(|| None);
    let modal = create_memo(move || info.get().is_some_and(|info| info.modal));
    let responding = create_memo(move || info.get().is_none_or(|info| info.responding));
    let focused = create_memo(move || info.get().is_some_and(|info| info.focused));
    let fullscreen = create_memo(|| None);
    let editor = workspace.editor();
    view! {
        <DockDialog
            id={window_tab(window)}
            parent={window_tab(workspace.window_key(parent))}
            title={title}
            icon=ICON_WEB_ASSET
            size
            modal
            on_close={move || workspace.close(window_tab(window))}
        >
            <WindowPanel
                editor={editor}
                window
                offers_fullscreen=false
                fullscreen={fullscreen}
                name={title}
                responding={responding}
                focused={focused}
                size={followed}
            />
        </DockDialog>
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
pub fn Failure(failed: Memo<bool>, reason: Memo<String>) -> NodeId {
    view! {
        <Frame visible={failed}>
            <Text style=TextStyle::CAPTION string={reason} color=TextColor::Danger />
        </Frame>
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
        <Frame visible={shown} padding=PANEL_PADDING>
            <List spacing=PANEL_SPACING>
                <Text style=TextStyle::CAPTION string={message} />
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
        </Frame>
    }
}
