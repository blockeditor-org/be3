use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use beui::NodeId;
use beui::dock::unstyled::{
    DockAlign, DockBarControl, DockBarMove, DockBarPlacement, DockBodies, DockDeclaration, DockDeclaredBar,
    DockDeclaredDialog, DockDeclaredTab, DockDragStage, DockEdge, DockHostEvent, DockHostLink,
    DockHostSlot, DockHosted, DockMapping, DockMode, DockOutside, DockOutsidePlace, DockPlain,
    DockPlainBar, DockPlainEntry, DockPlainHome, DockPlainNode, DockPlainPlace, DockPlainWindow,
    DockRequest, DockSpec, DockSpecEntry, DockSpecNode, DockSpecPane, DockSpecWindow, DockState,
    TabId,
};
use beui::dock::unstyled::{DockMenus, DockTabControl, DockTabMenu, dock_menu};
use beui::icons::Glyph;
use beui::reactive::*;
use beui::styled::WindowHandle;
use block_editor_plugin::{EditorHost, Pushed, ViewId, ViewRef};
use block_plugin_api as api;
use block_plugin_api::{ChildId, DockTabPath};
use uuid::Uuid;

use crate::child::{ChildDockTab, MenuMirror, MenuPicks};
use crate::{Editor, Subregion};

mod saved;

const EMPTY_BODIES: u64 = 1 << 62;
const BAR_ROOM: f32 = 4096.0;
const EMPTY_BAR: beui::Vec2 = beui::vec2(1.0, 1.0);

pub(crate) struct DockedTabs {
    link: RefCell<Option<Rc<ProtocolLink>>>,
    revision: OwnedSignal<u64>,
    slot: RefCell<Option<DockHostSlot>>,
    built: Cell<Option<bool>>,
    asking: OwnedSignal<Option<TabId>>,
    windows: RefCell<HashMap<u64, beui::WindowId>>,
}

impl Default for DockedTabs {
    fn default() -> Self {
        Self {
            link: RefCell::default(),
            revision: OwnedSignal::new(0),
            slot: RefCell::default(),
            built: Cell::new(None),
            asking: OwnedSignal::new(None),
            windows: RefCell::default(),
        }
    }
}

impl DockedTabs {
    pub(crate) fn provide_slot(
        &self,
        link: Rc<dyn DockHostLink>,
        docked: bool,
        host: &EditorHost,
        editor: Editor,
    ) {
        provide_context(saved::keeper(host.clone(), editor));
        let slot = match docked {
            true => DockHostSlot::new(Some(link)),
            false => DockHostSlot::bars(link),
        };
        *self.slot.borrow_mut() = Some(slot.clone());
        self.built.set(Some(docked));
        provide_context(slot);
    }

    pub(crate) fn key(&self, docked: bool) -> bool {
        let asked = self.slot.borrow().as_ref().is_some_and(DockHostSlot::asked);
        match (asked, self.built.get()) {
            (false, Some(built)) => built,
            _ => docked,
        }
    }

    fn bump(&self) {
        self.revision.handle().update(|revision| *revision += 1);
    }

    pub(crate) fn link(
        self: &Rc<Self>,
        host: EditorHost,
        editor: Editor,
        frame: ReadSignal<Vec<MenuNode>>,
        docked: bool,
    ) -> Rc<dyn DockHostLink> {
        let frame = create_memo(move || match editor.frame_child_menu() {
            Some(child) => child.get(),
            None => frame.get(),
        });
        let link = Rc::new_cyclic(|this| ProtocolLink {
            this: Weak::clone(this),
            host,
            view: ViewId::MAIN,
            docked,
            pushed: editor.pushed(Pushed::Docks),
            tabs: Rc::downgrade(self),
            bodies: RefCell::default(),
            placings: Cell::new(0),
            panels: RefCell::default(),
            menus: OwnedSignal::new(DockMenus::new()),
            frame,
            declared: RefCell::default(),
            watched: Cell::new(None),
            empties: RefCell::default(),
            asking: self.asking.handle(),
            bar_sizes: OwnedSignal::new(HashMap::new()),
            declares: OwnedSignal::new(0),
            declaring: OwnedSignal::new(false),
            saving: saved::Saving::default(),
            menued: RefCell::default(),
        });
        *self.link.borrow_mut() = Some(Rc::clone(&link));
        if docked {
            saved::keep_joined(&link, editor);
        }
        let menus = link.menus.handle();
        let sizes = link.bar_sizes.read_only();
        let menued = Rc::downgrade(&link);
        create_effect(move || {
            frame.with(|_| ());
            sizes.with(|_| ());
            menus.with(|menus| {
                for (_, items) in menus.values() {
                    items.with(|_| ());
                }
            });
            if let Some(link) = menued.upgrade() {
                untrack(|| link.send());
            }
        });
        let picking = Rc::downgrade(&link);
        let pushed = link.pushed;
        let picks = link.host.clone();
        create_effect(move || {
            pushed.get();
            if !docked {
                return;
            }
            let taken = untrack(|| picks.take_view_menu_picks());
            if let Some(link) = picking.upgrade() {
                for (view, id) in taken {
                    untrack(|| link.pick(TabId::new(view.0), &id));
                }
            }
        });
        let tabs = Rc::downgrade(self);
        let held = Rc::downgrade(&link);
        on_cleanup(move || {
            let Some(tabs) = tabs.upgrade() else {
                return;
            };
            let mine = tabs
                .link
                .borrow()
                .as_ref()
                .is_some_and(|link| Weak::ptr_eq(&Rc::downgrade(link), &held));
            if mine {
                if let Some(link) = held.upgrade() {
                    for view in link.menued.take() {
                        link.host.set_menu(view, Vec::new());
                    }
                }
                tabs.link.borrow_mut().take();
                tabs.bump();
            }
        });
        link
    }

    fn panel(&self, tab: TabId, window: Option<WindowHandle>) -> Option<NodeId> {
        let link = self.link.borrow().clone()?;
        link.panel(tab, window)
    }

    fn leave(&self, tab: TabId, window: WindowHandle) {
        if let Some(link) = self.link.borrow().clone() {
            link.leave(tab, window);
        }
    }

    pub(crate) fn is_bar(&self, tab: u64) -> bool {
        self.link.borrow().as_ref().is_some_and(|link| {
            link.declared
                .borrow()
                .as_ref()
                .is_some_and(|declared| declared.bar(TabId::new(tab)).is_some())
        })
    }

    pub(crate) fn window_id(&self, tab: u64) -> beui::WindowId {
        *self
            .windows
            .borrow_mut()
            .entry(tab)
            .or_insert_with(beui::WindowId::next)
    }

    pub(crate) fn presenting(&self, host: &EditorHost) -> Option<beui::WindowId> {
        let asking = self.asking.get();
        let link = self.link.borrow().clone()?;
        let hosted = link.watched.get()?.get()?;
        let tab = asking
            .or(hosted.focused)
            .filter(|tab| hosted.visible.contains(tab))
            .or_else(|| hosted.visible.first().copied())?;
        crate::module::Module::of(host)?.window(ViewRef {
            instance: host.instance()?,
            view: ViewId(tab.value()),
        })
    }

    pub(crate) fn release(&self) {
        if self.asking.get_untracked().is_some() {
            self.asking.handle().set(None);
        }
    }
}

#[component]
pub(crate) fn DockedTabBody(tabs: Rc<DockedTabs>, tab: u64) -> NodeId {
    let revised = tabs.revision.read_only();
    let node = create_signal(None);
    let window = beui::styled::use_window();
    let left = Rc::clone(&tabs);
    create_effect(move || {
        revised.get();
        let panel = untrack(|| tabs.panel(TabId::new(tab), window));
        node.set(panel);
    });
    on_cleanup(move || {
        if let Some(window) = window {
            left.leave(TabId::new(tab), window);
        }
    });
    view! {
        <beui::styled::List spacing=beui::styled::Space::None>
            <beui::styled::Portal @sizing=beui::ItemSize::Percent(100.0) node={node} />
        </beui::styled::List>
    }
}

struct Panel {
    node: NodeId,
    window: Option<Signal<Option<WindowHandle>>>,
    _scope: Scope,
}

struct ProtocolLink {
    this: Weak<ProtocolLink>,
    host: EditorHost,
    view: ViewId,
    docked: bool,
    pushed: ReadSignal<u64>,
    tabs: Weak<DockedTabs>,
    bodies: RefCell<Option<DockBodies>>,
    placings: Cell<u64>,
    panels: RefCell<HashMap<TabId, Panel>>,
    menus: OwnedSignal<DockMenus>,
    frame: Memo<Vec<MenuNode>>,
    declared: RefCell<Option<DockDeclaration>>,
    watched: Cell<Option<Memo<Option<DockHosted>>>>,
    empties: RefCell<HashMap<String, u64>>,
    asking: Signal<Option<TabId>>,
    bar_sizes: OwnedSignal<HashMap<TabId, beui::Vec2>>,
    declares: OwnedSignal<u64>,
    declaring: OwnedSignal<bool>,
    saving: saved::Saving,
    menued: RefCell<Vec<ViewId>>,
}

impl ProtocolLink {
    fn empty_body(&self, key: &str) -> u64 {
        let mut empties = self.empties.borrow_mut();
        let next = EMPTY_BODIES + empties.len() as u64;
        *empties.entry(key.to_owned()).or_insert(next)
    }

    fn empty_key(&self, tab: TabId) -> Option<String> {
        self.empties
            .borrow()
            .iter()
            .find(|(_, held)| **held == tab.value())
            .map(|(key, _)| key.clone())
    }

    fn pick(&self, tab: TabId, id: &str) {
        let menus = self.menus.handle();
        let own = menus.with_untracked(|menus| {
            menus
                .get(&tab)
                .and_then(|(_, items)| menu_action(&items.get_untracked(), id))
        });
        let Some(action) = own.or_else(|| menu_action(&self.frame.get_untracked(), id)) else {
            return;
        };
        self.asking.set(Some(tab));
        action.run();
    }

    fn send(&self) {
        let declared = self.declared.borrow();
        let Some(declaration) = declared.as_ref() else {
            return;
        };
        for pane in spec_panes(&declaration.spec)
            .into_iter()
            .filter(|pane| pane.keep)
        {
            self.empty_body(&pane.key);
        }
        let empties = self.empties.borrow().clone();
        let menus = self.menus.handle();
        let frame = self.frame.get_untracked();
        let menu = |tab: TabId| {
            let mut nodes = menus.with_untracked(|menus| {
                menus
                    .get(&tab)
                    .map(|(_, items)| items.get_untracked())
                    .unwrap_or_default()
            });
            if !nodes.is_empty() && !frame.is_empty() {
                nodes.push(MenuNode::Separator);
            }
            nodes.extend(frame.iter().cloned());
            crate::beui_frame::menu_entries(&nodes)
        };
        let empty = |pane: &DockSpecPane| {
            pane.keep
                .then(|| empties.get(&pane.key).copied())
                .flatten()
                .map(DockTabPath::own)
        };
        let sizes = self.bar_sizes.handle();
        let size = |bar: TabId| {
            let size = sizes
                .with_untracked(|sizes| sizes.get(&bar).copied())
                .unwrap_or(beui::Vec2::ZERO);
            api::Size {
                width: size.x,
                height: size.y,
            }
        };
        let block_type = self.host.block_type();
        let kind = |kind: &str| match block_type {
            Some(block_type) => format!("{}/{kind}", block_type.simple()),
            None => kind.to_owned(),
        };
        let declared = declaration_to_api(declaration, &own_path, &empty, &size, &kind);
        let views: Vec<ViewId> = match self.docked {
            true => declared
                .tabs
                .iter()
                .map(|tab| ViewId(tab.tab.tab))
                .collect(),
            false => Vec::new(),
        };
        for view in &views {
            self.host.set_menu(*view, menu(TabId::new(view.0)));
        }
        for gone in self.menued.replace(views.clone()) {
            if !views.contains(&gone) {
                self.host.set_menu(gone, Vec::new());
            }
        }
        self.host.declare_dock(self.view, Some(declared));
    }

    fn bump(&self) {
        if let Some(tabs) = self.tabs.upgrade() {
            tabs.bump();
        }
    }

    fn panel(&self, tab: TabId, window: Option<WindowHandle>) -> Option<NodeId> {
        if let Some(panel) = self.panels.borrow().get(&tab) {
            if let (Some(shown), Some(window)) = (panel.window, window) {
                shown.set(Some(window));
            }
            return Some(panel.node);
        }
        let bodies = self.bodies.borrow().clone()?;
        let scope = bodies.scope.child()?;
        let menu = DockTabMenu {
            tab,
            menu: self.menus.handle(),
        };
        let this = Weak::clone(&self.this);
        let watched = self.watched.get();
        let empty = self.empty_key(tab);
        let bar = self
            .declared
            .borrow()
            .as_ref()
            .is_some_and(|declared| declared.bar(tab).is_some());
        let sizes = self.bar_sizes.handle();
        let mut shown = None;
        let panel = scope.context().run(|| {
            if let Some(window) = window {
                let current = create_signal(Some(window));
                shown = Some(current);
                provide_context(WindowHandle::following(window.id, move || current.get()));
            }
            if let Some(key) = empty {
                return bodies.empty.call(key);
            }
            provide_context(menu);
            let request = Callback::new(move |request: DockRequest| {
                if let Some(link) = this.upgrade() {
                    link.request(request);
                }
            });
            if let Some(hosted) = watched {
                provide_context(DockTabControl::hosted(tab, hosted, request));
                if bar {
                    provide_context(DockBarControl::hosted(tab, hosted, request));
                }
            }
            let body = bodies.body.call(tab);
            if let (Some(hosted), false) = (watched, bar) {
                let covers = create_memo(move || {
                    hosted.with(|hosted| {
                        hosted.as_ref().map_or_else(Vec::new, |hosted| {
                            hosted
                                .covers
                                .iter()
                                .filter(|(covered, _)| *covered == tab)
                                .map(|(_, rect)| *rect)
                                .collect::<Vec<_>>()
                        })
                    })
                });
                keep_clear(body, covers);
            }
            let measured = RenderFn::new(move |body: NodeId| {
                view! {
                    <MeasuredBar body tab sizes />
                }
            });
            Some(match bar {
                true => measured.call(body),
                false => body,
            })
        })?;
        self.panels.borrow_mut().insert(
            tab,
            Panel {
                node: panel,
                window: shown,
                _scope: scope,
            },
        );
        Some(panel)
    }

    fn leave(&self, tab: TabId, window: WindowHandle) {
        let shown = self
            .panels
            .borrow()
            .get(&tab)
            .and_then(|panel| panel.window);
        if let Some(shown) = shown
            && shown.get_untracked() == Some(window)
        {
            shown.set(None);
        }
    }

    fn keep(&self, tabs: &[TabId]) {
        let mut tabs = tabs.to_vec();
        if let Some(bodies) = self.bodies.borrow().as_ref() {
            tabs.extend(bodies.hosts.get_untracked());
        }
        if let Some(declaration) = self.declared.borrow().as_ref() {
            tabs.extend(declaration.dialogs.iter().map(|dialog| dialog.tab));
            tabs.extend(declaration.bars.iter().map(|bar| bar.tab));
        }
        tabs.extend(self.empties.borrow().values().copied().map(TabId::new));
        let dropped: Vec<Panel> = {
            let mut panels = self.panels.borrow_mut();
            let gone: Vec<TabId> = panels
                .keys()
                .filter(|tab| !tabs.contains(tab))
                .copied()
                .collect();
            gone.into_iter()
                .filter_map(|tab| panels.remove(&tab))
                .collect()
        };
        if !dropped.is_empty() {
            drop(dropped);
            self.bump();
        }
    }
}

#[component]
fn MeasuredBar(body: NodeId, tab: TabId, sizes: Signal<HashMap<TabId, beui::Vec2>>) -> NodeId {
    use beui::styled::{Canvas, CanvasItem, Frame, Portal};
    let measured = measure(body);
    create_effect(move || {
        let Some(size) = measured.get().map(|measured| measured.size) else {
            return;
        };
        if sizes.with_untracked(|sizes| sizes.get(&tab) != Some(&size)) {
            sizes.update(|sizes| {
                sizes.insert(tab, size);
            });
        }
    });
    view! {
        <Canvas>
            <CanvasItem x=0.0 y=0.0 width=BAR_ROOM height=BAR_ROOM clip=false>
                <Frame align=beui::Align2::START>
                    <Portal node={Some(body)} />
                </Frame>
            </CanvasItem>
        </Canvas>
    }
}

fn own(tab: &DockTabPath) -> Option<TabId> {
    tab.is_own().then(|| TabId::new(tab.tab))
}

fn own_path(tab: TabId) -> Option<DockTabPath> {
    Some(DockTabPath::own(tab.value()))
}

impl DockHostLink for ProtocolLink {
    fn declare(&self, declaration: DockDeclaration) {
        *self.declared.borrow_mut() = Some(declaration);
        self.send();
        self.declares.update(|declares| *declares += 1);
        if !self.declaring.get_untracked() {
            self.declaring.set(true);
        }
    }

    fn hosted(&self) -> Memo<Option<DockHosted>> {
        let host = self.host.clone();
        let view = self.view;
        let pushed = self.pushed;
        create_memo(move || {
            pushed.get();
            let hosted = host.dock_hosted(view)?;
            Some(hosted_from_api(&hosted, &mut |path| own(path)))
        })
    }

    fn request(&self, request: DockRequest) {
        if let Some(request) = request_to_api(&request, &own_path) {
            self.host.request_dock(self.view, request);
        }
    }

    fn listen(&self, events: Callback<DockHostEvent>) {
        let host = self.host.clone();
        let view = self.view;
        let pushed = self.pushed;
        create_effect(move || {
            pushed.get();
            for tab in untrack(|| host.take_dock_closes(view)) {
                if let Some(tab) = own(&tab) {
                    untrack(|| events.call(DockHostEvent::Close(tab)));
                }
            }
        });
    }

    fn place(&self, bodies: DockBodies) {
        let placed = self.placings.get() + 1;
        self.placings.set(placed);
        *self.bodies.borrow_mut() = Some(bodies);
        let leaving = Weak::clone(&self.this);
        on_cleanup(move || {
            if let Some(link) = leaving.upgrade() {
                link.unplace(placed);
            }
        });
        let hosted = self.hosted();
        self.watched.set(Some(hosted));
        let this = Weak::clone(&self.this);
        let hosts = self.bodies.borrow().as_ref().map(|bodies| bodies.hosts);
        create_effect(move || {
            if let Some(hosts) = hosts {
                hosts.get();
            }
            let held = hosted.with(|hosted| {
                hosted
                    .as_ref()
                    .map(|hosted| hosted.tabs.clone())
                    .unwrap_or_default()
            });
            if let Some(link) = this.upgrade() {
                untrack(|| link.keep(&held));
            }
        });
        self.bump();
    }

    fn withdraw(&self) {
        self.declared.borrow_mut().take();
        self.host.declare_dock(self.view, None);
        if self.declaring.get_untracked() {
            self.declaring.set(false);
        }
    }
}

impl ProtocolLink {
    fn unplace(&self, placed: u64) {
        if self.placings.get() != placed {
            return;
        }
        self.bodies.borrow_mut().take();
        let panels = std::mem::take(&mut *self.panels.borrow_mut());
        drop(panels);
        self.bump();
    }
}

#[derive(Default)]
struct Paths {
    tabs: HashMap<DockTabPath, TabId>,
    paths: HashMap<TabId, DockTabPath>,
    next: u64,
}

impl Paths {
    fn tab(&mut self, path: &DockTabPath) -> TabId {
        if let Some(tab) = self.tabs.get(path) {
            return *tab;
        }
        self.next += 1;
        let tab = TabId::new(self.next);
        self.tabs.insert(path.clone(), tab);
        self.paths.insert(tab, path.clone());
        tab
    }

    fn path(&self, tab: TabId) -> Option<DockTabPath> {
        self.paths.get(&tab).cloned()
    }
}

fn group_key(nodes: &api::DockNodes, owner: &[ChildId]) -> Option<String> {
    fn find(node: &api::DockNode, child: ChildId) -> Option<(&api::DockGroup, &api::DockNode)> {
        match node {
            api::DockNode::Split { first, second, .. } => {
                find(first, child).or_else(|| find(second, child))
            }
            api::DockNode::Pane { entries, .. } => entries.iter().find_map(|entry| match entry {
                api::DockEntry::Tab(_) => None,
                api::DockEntry::Group { group, root } if group.child == Some(child) => {
                    Some((group, root.as_ref()))
                }
                api::DockEntry::Group { root, .. } => find(root, child),
            }),
        }
    }
    let roots: Vec<&api::DockNode> = nodes
        .main
        .iter()
        .chain(
            nodes
                .windows
                .iter()
                .filter_map(|window| window.root.as_ref()),
        )
        .collect();
    let (first, rest) = owner.split_first()?;
    let (mut group, mut root) = roots.iter().find_map(|root| find(root, *first))?;
    for child in rest {
        (group, root) = find(root, *child)?;
    }
    Some(group.key.clone())
}

pub(crate) fn bridge_child_dock(
    editor: Editor,
    link: Rc<dyn DockHostLink>,
    state: ReadSignal<crate::ChildState>,
    docked_tab: Option<RenderFn<ChildDockTab, NodeId>>,
) {
    let host = editor.host();
    let pushed = editor.pushed(Pushed::Docks);
    let child = create_signal(None::<ChildId>);
    let last = Rc::new(Cell::new(None::<ChildId>));
    {
        let last = Rc::clone(&last);
        create_effect(move || {
            let placed = state.with(|state| state.child);
            if placed.is_some() && placed != last.get() {
                last.set(placed);
                child.set(placed);
            }
        });
    }
    let paths = Rc::new(RefCell::new(Paths::default()));
    let declared = Rc::new(Cell::new(false));
    {
        let link = Rc::clone(&link);
        let host = host.clone();
        let paths = Rc::clone(&paths);
        create_effect(move || {
            pushed.get();
            let Some(child) = child.get() else {
                return;
            };
            untrack(|| {
                let declaration = host.child_dock(child);
                let converted = declaration.as_ref().and_then(|declaration| {
                    declaration_from_api(declaration, &mut paths.borrow_mut())
                });
                match converted {
                    Some(converted) => {
                        link.declare(converted);
                        declared.set(true);
                    }
                    None if declared.replace(false) => link.withdraw(),
                    None => {}
                }
                for (owner, request) in host.take_child_dock_requests(child) {
                    let request = match owner.is_empty() {
                        true => request_from_api(&request, &mut paths.borrow_mut()),
                        false => declaration.as_ref().and_then(|declaration| {
                            nested_restore(&*link, declaration, &owner, &request, &paths)
                        }),
                    };
                    if let Some(request) = request {
                        link.request(request);
                    }
                }
            });
        });
    }
    let hosted = link.hosted();
    {
        let host = host.clone();
        let paths = Rc::clone(&paths);
        create_effect(move || {
            let Some(child) = child.get() else {
                return;
            };
            let hosted = hosted.get();
            untrack(|| {
                let paths = paths.borrow();
                let hold = match hosted {
                    Some(hosted) => {
                        api::DockHold::Hosted(hosted_to_api(&hosted, &|tab| paths.path(tab)))
                    }
                    None => api::DockHold::Placing,
                };
                host.host_child_dock(child, hold);
            });
        });
    }
    {
        let host = host.clone();
        let paths = Rc::clone(&paths);
        let closing = Rc::clone(&last);
        link.listen(Callback::new(move |event: DockHostEvent| {
            let DockHostEvent::Close(tab) = event;
            let path = paths.borrow().path(tab);
            if let (Some(child), Some(path)) = (closing.get(), path) {
                host.close_child_dock_tab(child, path);
            }
        }));
    }
    if let Some(scope) = owner_scope() {
        let paths = Rc::clone(&paths);
        let shown = Rc::clone(&last);
        let declared = host.clone();
        let menus = host.clone();
        let host = host.clone();
        let body = RenderFn::new(move |tab: TabId| {
            let path = paths.borrow().path(tab);
            let child = shown.get();
            let declaration = child.and_then(|child| untrack(|| declared.child_dock(child)));
            let dialog = declaration
                .as_ref()
                .zip(path.as_ref())
                .is_some_and(|(declaration, path)| declaration.dialog(path).is_some());
            let bar = declaration
                .as_ref()
                .zip(path.as_ref())
                .is_some_and(|(declaration, path)| declaration.bar(path).is_some());
            if let (true, Some(shown)) = (bar, child.zip(path.clone())) {
                let host = declared.clone();
                return view! {
                    <DockBarBody editor host pushed shown />
                };
            }
            let owner = declaration
                .as_ref()
                .zip(path.as_ref())
                .and_then(|(declaration, path)| {
                    declaration.owner_block(&path.via).map(Uuid::from_bytes)
                });
            let menu = tab_menu(editor, menus.clone(), pushed, child, path.clone());
            let shown = child.zip(path);
            let body = view! {
                <DockTabBody editor shown />
            };
            match (dialog, docked_tab) {
                (true, _) => body,
                (false, Some(framed)) => framed.call(ChildDockTab { menu, body, owner }),
                (false, None) => {
                    dock_menu(menu);
                    body
                }
            }
        });
        let emptied = Rc::clone(&last);
        let empty = RenderFn::new(move |key: String| {
            let child = emptied.get()?;
            let declaration = untrack(|| host.child_dock(child))?;
            let nodes = declaration.nest().ok()?;
            let path = nodes
                .panes()
                .into_iter()
                .find(|pane| pane.key == key)?
                .empty
                .clone()?;
            let shown = Some((child, path));
            Some(view! {
                <DockTabBody editor shown />
            })
        });
        link.place(DockBodies {
            scope,
            body,
            empty,
            hosts: create_memo(Vec::new),
        });
    }
    on_cleanup(move || {
        if let Some(child) = last.get() {
            host.host_child_dock(child, api::DockHold::Released);
        }
        link.withdraw();
    });
}

#[component]
fn DockBarBody(
    editor: Editor,
    host: EditorHost,
    pushed: ReadSignal<u64>,
    shown: (ChildId, DockTabPath),
) -> NodeId {
    let (child, path) = shown.clone();
    let size = create_memo(move || {
        pushed.get();
        untrack(|| host.child_dock(child))
            .and_then(|declaration| declaration.bar(&path).map(|bar| bar.size))
            .filter(|size| size.width > 0.0 && size.height > 0.0)
            .map_or(EMPTY_BAR, |size| beui::vec2(size.width, size.height))
    });
    let width =
        create_memo(move || beui::styled::Length::Size(beui::styled::Size::Custom(size.get().x)));
    let height =
        create_memo(move || beui::styled::Length::Size(beui::styled::Size::Custom(size.get().y)));
    let shown = Some(shown);
    view! {
        <beui::styled::Frame width=width height=height>
            <DockTabBody editor shown />
        </beui::styled::Frame>
    }
}

#[component]
fn DockTabBody(editor: Editor, shown: Option<(ChildId, DockTabPath)>) -> NodeId {
    let placed = shown.map(|(child, path)| crate::SubregionContent::DockTab(path.through(child)));
    view! {
        <beui::styled::List spacing=beui::styled::Space::None>
            <Subregion
                @sizing=beui::ItemSize::Percent(100.0)
                editor={editor}
                placed={Prop::Static(placed)}
                mode=api::ChildMode::Live
            />
        </beui::styled::List>
    }
}

fn tab_menu(
    editor: Editor,
    host: EditorHost,
    pushed: ReadSignal<u64>,
    child: Option<ChildId>,
    path: Option<DockTabPath>,
) -> Memo<Vec<MenuNode>> {
    let nodes = create_signal(Vec::<MenuNode>::new());
    let picks = path.clone().map(MenuPicks::DockTab);
    let mirror = RefCell::new(MenuMirror::picking(
        editor,
        picks.unwrap_or(MenuPicks::Child),
    ));
    let shown = child.zip(path);
    create_effect(move || {
        pushed.get();
        let Some((child, path)) = shown.clone() else {
            return;
        };
        let entries = untrack(|| host.child_dock(child))
            .and_then(|declaration| declaration.tab(&path).map(|tab| tab.menu.clone()))
            .and_then(|menu| api::MenuEntry::nest(menu).ok())
            .unwrap_or_default();
        let updated = mirror.borrow_mut().update(Some(child), &entries);
        if let Some(updated) = updated {
            untrack(|| nodes.set(updated));
        }
    });
    create_memo(move || nodes.get())
}

fn nested_restore(
    link: &dyn DockHostLink,
    declaration: &api::DockDeclaration,
    owner: &[ChildId],
    request: &api::DockRequest,
    paths: &Rc<RefCell<Paths>>,
) -> Option<DockRequest> {
    let api::DockRequest::Restore(layout) = request else {
        return request_from_api(request, &mut paths.borrow_mut());
    };
    let nodes = declaration.nest().ok()?;
    let group = group_key(&nodes, owner)?;
    let from = {
        let mut paths = paths.borrow_mut();
        DockState::from_plain(&layout_to_plain(layout, &mut |path| Some(paths.tab(path))))
    };
    let hosted = untrack(|| link.hosted().get_untracked())?;
    let mut state = hosted.state;
    let found = state.find_group(&group)?;
    state.graft(
        found,
        &from,
        &DockMapping {
            tab: &|tab| Some(tab),
            key: &|key| Some(key.to_owned()),
            group: &|_| None,
        },
    );
    Some(DockRequest::Restore(state))
}

type PathOf<'a> = &'a dyn Fn(TabId) -> Option<DockTabPath>;
type TabOf<'a> = &'a mut dyn FnMut(&DockTabPath) -> Option<TabId>;

fn direction_to_api(direction: beui::Direction) -> api::DockDirection {
    match direction {
        beui::Direction::Horizontal => api::DockDirection::Horizontal,
        beui::Direction::Vertical => api::DockDirection::Vertical,
    }
}

fn direction_from_api(direction: api::DockDirection) -> beui::Direction {
    match direction {
        api::DockDirection::Horizontal => beui::Direction::Horizontal,
        api::DockDirection::Vertical => beui::Direction::Vertical,
    }
}

fn spec_panes(spec: &DockSpec) -> Vec<&DockSpecPane> {
    fn walk<'a>(node: &'a DockSpecNode, out: &mut Vec<&'a DockSpecPane>) {
        match node {
            DockSpecNode::Split { first, second, .. } => {
                walk(first, out);
                walk(second, out);
            }
            DockSpecNode::Pane(pane) => {
                out.push(pane);
                for entry in &pane.entries {
                    if let DockSpecEntry::Group { root, .. } = entry {
                        walk(root, out);
                    }
                }
            }
        }
    }
    let mut panes = Vec::new();
    for root in spec.main.iter().chain(
        spec.windows
            .iter()
            .filter_map(|window| window.root.as_ref()),
    ) {
        walk(root, &mut panes);
    }
    panes
}

type EmptyOf<'a> = &'a dyn Fn(&DockSpecPane) -> Option<DockTabPath>;

fn spec_node(node: &DockSpecNode, path: PathOf<'_>, empty: EmptyOf<'_>) -> api::DockNode {
    match node {
        DockSpecNode::Split {
            key,
            direction,
            fraction,
            first,
            second,
        } => api::DockNode::Split {
            split: api::DockSplit {
                key: key.clone(),
                direction: direction_to_api(*direction),
                fraction: *fraction,
            },
            first: Box::new(spec_node(first, path, empty)),
            second: Box::new(spec_node(second, path, empty)),
        },
        DockSpecNode::Pane(pane) => {
            let entries: Vec<(Option<TabId>, api::DockEntry)> = pane
                .entries
                .iter()
                .filter_map(|entry| match entry {
                    DockSpecEntry::Tab(tab) => Some((Some(*tab), api::DockEntry::Tab(path(*tab)?))),
                    DockSpecEntry::Group { key, pinned, root } => Some((
                        None,
                        api::DockEntry::Group {
                            group: api::DockGroup {
                                key: key.clone(),
                                pinned: *pinned,
                                child: None,
                            },
                            root: Box::new(spec_node(root, path, empty)),
                        },
                    )),
                })
                .collect();
            let active = pane.active.and_then(|active| {
                entries
                    .iter()
                    .position(|(tab, _)| *tab == Some(active))
                    .map(|index| index as u32)
            });
            api::DockNode::Pane {
                pane: api::DockPane {
                    key: pane.key.clone(),
                    active,
                    vertical: pane.vertical,
                    icons_only: pane.icons_only,
                    sidebar: pane.sidebar,
                    keep: pane.keep,
                    area: pane.locked.then(|| pane.key.clone()),
                    empty: empty(pane),
                    bare: pane.bare,
                },
                entries: entries.into_iter().map(|(_, entry)| entry).collect(),
            }
        }
    }
}

pub(crate) fn declaration_to_api(
    declaration: &DockDeclaration,
    path: PathOf<'_>,
    empty: EmptyOf<'_>,
    size: &dyn Fn(TabId) -> api::Size,
    kind: &dyn Fn(&str) -> String,
) -> api::DockDeclaration {
    let spec = &declaration.spec;
    let nodes = api::DockNodes {
        main: spec.main.as_ref().map(|main| spec_node(main, path, empty)),
        windows: spec
            .windows
            .iter()
            .map(|window| api::DockFloating {
                window: api::DockWindow {
                    key: window.key.clone(),
                    rect: window.rect,
                },
                root: window
                    .root
                    .as_ref()
                    .map(|root| spec_node(root, path, empty)),
            })
            .collect(),
    };
    api::DockDeclaration {
        tree: api::DockTree::flatten(&nodes, spec.focus.and_then(path)),
        tabs: declaration
            .tabs
            .iter()
            .filter_map(|tab| {
                Some(api::DockTab {
                    tab: path(tab.tab)?,
                    key: tab.key.clone(),
                    title: tab.title.clone(),
                    glyph: tab
                        .icon
                        .map(|icon| icon.as_str().to_owned())
                        .unwrap_or_default(),
                    closable: tab.closable,
                    menu: Vec::new(),
                })
            })
            .collect(),
        dialogs: declaration
            .dialogs
            .iter()
            .filter_map(|dialog| {
                Some(api::DockDialog {
                    tab: path(dialog.tab)?,
                    parent: path(dialog.parent)?,
                    title: dialog.title.clone(),
                    glyph: dialog
                        .icon
                        .map(|icon| icon.as_str().to_owned())
                        .unwrap_or_default(),
                    size: api::Size {
                        width: dialog.size.x,
                        height: dialog.size.y,
                    },
                    modal: dialog.modal,
                    dismiss_on_outside_press: dialog.dismiss_on_outside_press,
                    closable: dialog.closable,
                })
            })
            .collect(),
        bars: declaration
            .bars
            .iter()
            .filter_map(|bar| {
                Some(api::DockBar {
                    tab: path(bar.tab)?,
                    key: bar.key.clone(),
                    kind: kind(&bar.kind),
                    owner: path(bar.owner)?,
                    placement: placement_to_api(bar.placement),
                    size: size(bar.tab),
                })
            })
            .collect(),
    }
}

fn api_node(node: &api::DockNode, paths: &mut Paths) -> DockSpecNode {
    match node {
        api::DockNode::Split {
            split,
            first,
            second,
        } => DockSpecNode::Split {
            key: split.key.clone(),
            direction: direction_from_api(split.direction),
            fraction: split.fraction,
            first: Box::new(api_node(first, paths)),
            second: Box::new(api_node(second, paths)),
        },
        api::DockNode::Pane { pane, entries } => {
            let entries: Vec<DockSpecEntry> = entries
                .iter()
                .map(|entry| match entry {
                    api::DockEntry::Tab(tab) => DockSpecEntry::Tab(paths.tab(tab)),
                    api::DockEntry::Group { group, root } => DockSpecEntry::Group {
                        key: group.key.clone(),
                        pinned: group.pinned,
                        root: Box::new(api_node(root, paths)),
                    },
                })
                .collect();
            let active = pane
                .active
                .and_then(|active| entries.get(active as usize))
                .and_then(|entry| match entry {
                    DockSpecEntry::Tab(tab) => Some(*tab),
                    DockSpecEntry::Group { .. } => None,
                });
            DockSpecNode::Pane(DockSpecPane {
                key: pane.key.clone(),
                entries,
                active,
                vertical: pane.vertical,
                icons_only: pane.icons_only,
                sidebar: pane.sidebar,
                keep: pane.keep,
                locked: pane.area.as_ref() == Some(&pane.key),
                bare: pane.bare,
            })
        }
    }
}

fn declaration_from_api(
    declaration: &api::DockDeclaration,
    paths: &mut Paths,
) -> Option<DockDeclaration> {
    let nodes = declaration.nest().ok()?;
    let spec = DockSpec {
        main: nodes.main.as_ref().map(|main| api_node(main, paths)),
        windows: nodes
            .windows
            .iter()
            .map(|window| DockSpecWindow {
                key: window.window.key.clone(),
                rect: window.window.rect,
                root: window.root.as_ref().map(|root| api_node(root, paths)),
            })
            .collect(),
        focus: declaration
            .tree
            .focus
            .as_ref()
            .map(|focus| paths.tab(focus)),
        see_through: Vec::new(),
        bars: Vec::new(),
    };
    let tabs = declaration
        .tabs
        .iter()
        .map(|tab| DockDeclaredTab {
            tab: paths.tab(&tab.tab),
            key: tab.key.clone(),
            title: tab.title.clone(),
            icon: Glyph::from_codepoint(&tab.glyph),
            picture: None,
            closable: tab.closable,
        })
        .collect();
    let dialogs = declaration
        .dialogs
        .iter()
        .map(|dialog| DockDeclaredDialog {
            tab: paths.tab(&dialog.tab),
            parent: paths.tab(&dialog.parent),
            title: dialog.title.clone(),
            icon: Glyph::from_codepoint(&dialog.glyph),
            size: beui::vec2(dialog.size.width, dialog.size.height),
            modal: dialog.modal,
            dismiss_on_outside_press: dialog.dismiss_on_outside_press,
            closable: dialog.closable,
        })
        .collect();
    let bars = declaration
        .bars
        .iter()
        .map(|bar| DockDeclaredBar {
            tab: paths.tab(&bar.tab),
            key: bar.key.clone(),
            kind: bar.kind.clone(),
            owner: paths.tab(&bar.owner),
            placement: placement_from_api(&bar.placement),
        })
        .collect();
    Some(DockDeclaration {
        spec,
        tabs,
        dialogs,
        bars,
    })
}

fn plain_node(node: &DockPlainNode, path: PathOf<'_>) -> api::DockNode {
    match node {
        DockPlainNode::Split {
            key,
            direction,
            fraction,
            first,
            second,
        } => api::DockNode::Split {
            split: api::DockSplit {
                key: key.clone(),
                direction: direction_to_api(*direction),
                fraction: *fraction,
            },
            first: Box::new(plain_node(first, path)),
            second: Box::new(plain_node(second, path)),
        },
        DockPlainNode::Pane {
            key,
            active,
            vertical,
            icons_only,
            sidebar,
            keep,
            area,
            bare,
            entries,
        } => {
            let mut kept = Vec::new();
            let mut shown = None;
            for (index, entry) in entries.iter().enumerate() {
                let entry = match entry {
                    DockPlainEntry::Tab(tab) => path(*tab).map(api::DockEntry::Tab),
                    DockPlainEntry::Group { key, pinned, root } => Some(api::DockEntry::Group {
                        group: api::DockGroup {
                            key: key.clone(),
                            pinned: *pinned,
                            child: None,
                        },
                        root: Box::new(plain_node(root, path)),
                    }),
                };
                if let Some(entry) = entry {
                    if index == *active {
                        shown = Some(kept.len() as u32);
                    }
                    kept.push(entry);
                }
            }
            api::DockNode::Pane {
                pane: api::DockPane {
                    key: key.clone(),
                    active: shown.or((!kept.is_empty()).then_some(0)),
                    vertical: *vertical,
                    icons_only: *icons_only,
                    sidebar: *sidebar,
                    keep: *keep,
                    area: area.clone(),
                    empty: None,
                    bare: *bare,
                },
                entries: kept,
            }
        }
    }
}

fn plain_to_layout(plain: &DockPlain, path: PathOf<'_>) -> api::DockLayout {
    let nodes = api::DockNodes {
        main: plain.main.as_ref().map(|main| plain_node(main, path)),
        windows: plain
            .windows
            .iter()
            .map(|window| api::DockFloating {
                window: api::DockWindow {
                    key: window.key.clone(),
                    rect: window.rect,
                },
                root: window.root.as_ref().map(|root| plain_node(root, path)),
            })
            .collect(),
    };
    api::DockLayout {
        tree: api::DockTree::flatten(&nodes, plain.focus.and_then(path)),
        homes: plain
            .homes
            .iter()
            .filter_map(|home| {
                Some(api::DockHome {
                    tab: path(home.tab)?,
                    place: api_place(&home.place),
                })
            })
            .collect(),
        bars: plain
            .bars
            .iter()
            .filter_map(|bar| {
                Some(api::DockPlacedBar {
                    bar: path(bar.bar)?,
                    owner: path(bar.owner)?,
                    placement: placement_to_api(bar.placement),
                })
            })
            .collect(),
        outside: plain
            .outside
            .iter()
            .filter_map(|outside| {
                Some(api::DockOutside {
                    tab: path(outside.tab)?,
                    place: match &outside.place {
                        DockOutsidePlace::Pane { key, index } => {
                            api::DockOutsidePlace::Pane(api::DockOutsidePane {
                                key: key.clone(),
                                index: *index as u32,
                            })
                        }
                        DockOutsidePlace::Window { rect } => api::DockOutsidePlace::Window(*rect),
                    },
                })
            })
            .collect(),
        unlocked: plain.unlocked.iter().map(api_place).collect(),
    }
}

fn api_place(place: &DockPlainPlace) -> api::DockPlace {
    match place {
        DockPlainPlace::Area(area) => api::DockPlace::Area(area.clone()),
        DockPlainPlace::Group(group) => api::DockPlace::Group(group.clone()),
    }
}

fn plain_place(place: &api::DockPlace) -> DockPlainPlace {
    match place {
        api::DockPlace::Area(area) => DockPlainPlace::Area(area.clone()),
        api::DockPlace::Group(group) => DockPlainPlace::Group(group.clone()),
    }
}

fn api_plain_node(node: &api::DockNode, tab: TabOf<'_>) -> DockPlainNode {
    match node {
        api::DockNode::Split {
            split,
            first,
            second,
        } => DockPlainNode::Split {
            key: split.key.clone(),
            direction: direction_from_api(split.direction),
            fraction: split.fraction,
            first: Box::new(api_plain_node(first, tab)),
            second: Box::new(api_plain_node(second, tab)),
        },
        api::DockNode::Pane { pane, entries } => {
            let mut kept = Vec::new();
            let mut active = 0;
            for (index, entry) in entries.iter().enumerate() {
                let entry = match entry {
                    api::DockEntry::Tab(path) => tab(path).map(DockPlainEntry::Tab),
                    api::DockEntry::Group { group, root } => Some(DockPlainEntry::Group {
                        key: group.key.clone(),
                        pinned: group.pinned,
                        root: Box::new(api_plain_node(root, tab)),
                    }),
                };
                if let Some(entry) = entry {
                    if pane.active == Some(index as u32) {
                        active = kept.len();
                    }
                    kept.push(entry);
                }
            }
            DockPlainNode::Pane {
                key: pane.key.clone(),
                active,
                vertical: pane.vertical,
                icons_only: pane.icons_only,
                sidebar: pane.sidebar,
                keep: pane.keep,
                area: pane.area.clone(),
                bare: pane.bare,
                entries: kept,
            }
        }
    }
}

fn layout_to_plain(layout: &api::DockLayout, tab: TabOf<'_>) -> DockPlain {
    let Ok(nodes) = layout.tree.nest() else {
        return DockPlain::default();
    };
    DockPlain {
        main: nodes.main.as_ref().map(|main| api_plain_node(main, tab)),
        windows: nodes
            .windows
            .iter()
            .map(|window| DockPlainWindow {
                key: window.window.key.clone(),
                rect: window.window.rect,
                root: window.root.as_ref().map(|root| api_plain_node(root, tab)),
            })
            .collect(),
        focus: layout.tree.focus.as_ref().and_then(&mut *tab),
        homes: layout
            .homes
            .iter()
            .filter_map(|home| {
                Some(DockPlainHome {
                    tab: tab(&home.tab)?,
                    place: plain_place(&home.place),
                })
            })
            .collect(),
        bars: layout
            .bars
            .iter()
            .filter_map(|bar| {
                Some(DockPlainBar {
                    bar: tab(&bar.bar)?,
                    owner: tab(&bar.owner)?,
                    placement: placement_from_api(&bar.placement),
                })
            })
            .collect(),
        outside: layout
            .outside
            .iter()
            .filter_map(|outside| {
                Some(DockOutside {
                    tab: tab(&outside.tab)?,
                    place: match &outside.place {
                        api::DockOutsidePlace::Pane(pane) => DockOutsidePlace::Pane {
                            key: pane.key.clone(),
                            index: pane.index as usize,
                        },
                        api::DockOutsidePlace::Window(rect) => {
                            DockOutsidePlace::Window { rect: *rect }
                        }
                    },
                })
            })
            .collect(),
        unlocked: layout.unlocked.iter().map(plain_place).collect(),
    }
}

fn hosted_from_api(hosted: &api::DockHosted, tab: TabOf<'_>) -> DockHosted {
    let state = DockState::from_plain(&layout_to_plain(&hosted.layout, tab));
    let mut list =
        |paths: &[DockTabPath]| -> Vec<TabId> { paths.iter().filter_map(&mut *tab).collect() };
    let tabs = list(&hosted.tabs);
    let visible = list(&hosted.visible);
    let covers = hosted
        .covers
        .iter()
        .filter_map(|cover| Some((tab(&cover.tab)?, cover.rect)))
        .collect();
    DockHosted {
        state,
        tabs,
        focused: hosted.focused.as_ref().and_then(&mut *tab),
        shown: hosted.shown.as_ref().and_then(&mut *tab),
        visible,
        fullscreen: hosted.fullscreen.as_ref().and_then(&mut *tab),
        mode: match hosted.stacked {
            true => DockMode::Stacked,
            false => DockMode::Tiled,
        },
        covers,
    }
}

fn hosted_to_api(hosted: &DockHosted, path: PathOf<'_>) -> api::DockHosted {
    let list = |tabs: &[TabId]| tabs.iter().filter_map(|tab| path(*tab)).collect();
    api::DockHosted {
        layout: plain_to_layout(&hosted.state.plain(), path),
        tabs: list(&hosted.tabs),
        focused: hosted.focused.and_then(path),
        shown: hosted.shown.and_then(path),
        visible: list(&hosted.visible),
        fullscreen: hosted.fullscreen.and_then(path),
        stacked: hosted.mode == DockMode::Stacked,
        covers: hosted
            .covers
            .iter()
            .filter_map(|(tab, rect)| {
                Some(api::DockCover {
                    tab: path(*tab)?,
                    rect: *rect,
                })
            })
            .collect(),
    }
}

fn request_to_api(request: &DockRequest, path: PathOf<'_>) -> Option<api::DockRequest> {
    Some(match request {
        DockRequest::Show(tab) => api::DockRequest::Show(path(*tab)?),
        DockRequest::Restore(state) => {
            api::DockRequest::Restore(plain_to_layout(&state.plain(), path))
        }
        DockRequest::Fullscreen(None) => api::DockRequest::Fullscreen(None),
        DockRequest::Fullscreen(Some((tab, area))) => {
            api::DockRequest::Fullscreen(Some(api::DockFullscreen {
                tab: path(*tab)?,
                area: *area,
            }))
        }
        DockRequest::PlaceBar(bar, placement) => api::DockRequest::PlaceBar(api::DockBarPlace {
            bar: path(*bar)?,
            placement: placement.map(placement_to_api),
        }),
        DockRequest::MoveBar(bar, moved) => api::DockRequest::MoveBar(api::DockBarMove {
            bar: path(*bar)?,
            at: moved.at,
            stage: match moved.stage {
                DockDragStage::Move => api::DockDragStage::Move,
                DockDragStage::Drop => api::DockDragStage::Drop,
                DockDragStage::Cancel => api::DockDragStage::Cancel,
            },
        }),
    })
}

fn request_from_api(request: &api::DockRequest, paths: &mut Paths) -> Option<DockRequest> {
    Some(match request {
        api::DockRequest::Show(tab) => DockRequest::Show(paths.tab(tab)),
        api::DockRequest::Restore(layout) => DockRequest::Restore(DockState::from_plain(
            &layout_to_plain(layout, &mut |path| Some(paths.tab(path))),
        )),
        api::DockRequest::Fullscreen(None) => DockRequest::Fullscreen(None),
        api::DockRequest::Fullscreen(Some(fullscreen)) => {
            DockRequest::Fullscreen(Some((paths.tab(&fullscreen.tab), fullscreen.area)))
        }
        api::DockRequest::PlaceBar(place) => DockRequest::PlaceBar(
            paths.tab(&place.bar),
            place.placement.as_ref().map(placement_from_api),
        ),
        api::DockRequest::MoveBar(moved) => DockRequest::MoveBar(
            paths.tab(&moved.bar),
            DockBarMove {
                at: moved.at,
                stage: match moved.stage {
                    api::DockDragStage::Move => DockDragStage::Move,
                    api::DockDragStage::Drop => DockDragStage::Drop,
                    api::DockDragStage::Cancel => DockDragStage::Cancel,
                },
            },
        ),
    })
}

fn edge_to_api(edge: DockEdge) -> api::DockEdge {
    match edge {
        DockEdge::Top => api::DockEdge::Top,
        DockEdge::Bottom => api::DockEdge::Bottom,
        DockEdge::Left => api::DockEdge::Left,
        DockEdge::Right => api::DockEdge::Right,
    }
}

fn edge_from_api(edge: api::DockEdge) -> DockEdge {
    match edge {
        api::DockEdge::Top => DockEdge::Top,
        api::DockEdge::Bottom => DockEdge::Bottom,
        api::DockEdge::Left => DockEdge::Left,
        api::DockEdge::Right => DockEdge::Right,
    }
}

fn placement_to_api(placement: DockBarPlacement) -> api::DockBarPlacement {
    match placement {
        DockBarPlacement::Over(edge, align) => api::DockBarPlacement::Over(api::DockBarOver {
            edge: edge_to_api(edge),
            align: match align {
                DockAlign::Start => api::DockAlign::Start,
                DockAlign::Center => api::DockAlign::Center,
                DockAlign::End => api::DockAlign::End,
            },
        }),
        DockBarPlacement::Beside(edge) => api::DockBarPlacement::Beside(edge_to_api(edge)),
        DockBarPlacement::Window { edge, origin } => {
            api::DockBarPlacement::Window(api::DockBarWindow {
                edge: edge_to_api(edge),
                origin,
            })
        }
    }
}

fn placement_from_api(placement: &api::DockBarPlacement) -> DockBarPlacement {
    match placement {
        api::DockBarPlacement::Over(over) => DockBarPlacement::Over(
            edge_from_api(over.edge),
            match over.align {
                api::DockAlign::Start => DockAlign::Start,
                api::DockAlign::Center => DockAlign::Center,
                api::DockAlign::End => DockAlign::End,
            },
        ),
        api::DockBarPlacement::Beside(edge) => DockBarPlacement::Beside(edge_from_api(*edge)),
        api::DockBarPlacement::Window(window) => DockBarPlacement::Window {
            edge: edge_from_api(window.edge),
            origin: window.origin,
        },
    }
}
