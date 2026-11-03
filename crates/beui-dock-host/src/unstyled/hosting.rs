mod manager;
mod snap;
mod switch;

use be_lib_stable_hash::{HashMap, HashSet};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui_dock::{
    DockBodies, DockDeclaration, DockDeclaredDialog, DockHostEvent, DockHostLink, DockHostSlot,
    DockHosted, DockJoin, DockRequest,
};
use beui_dock_state::{
    DockAlign, DockEdge, DockFullscreen, DockMode, DockSpec, DockSpecNode, DockState, GroupId,
    LeafId, TabId, joined_host, joined_key,
};
use beui_macros::{component, view};
use beui_view::component_ref::ComponentRef;
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::{Color, Modifiers, Pos2, Rect};

use super::dialog::{DialogParts, DockDialogHandle};
use super::{
    BarCovers, BarFloats, Dock, DockApi, DockAwayHandle, DockBarBandHandle, DockBarHandle,
    DockChromeHandle, DockConfig, DockGripHandle, DockPreviewHandle, DockSplitterHandle,
    DockStackHandle, DockStripHandle, DockSwitchHandle, DockSwitcherHandle, DockTabHandle,
};
use beui_components_unstyled::menu_popup::MenuStyle;
pub use manager::{DockWindowActions, holding_area};
pub use snap::DockSnap;
use snap::Snapped;

#[derive(PartialEq)]
struct Declared {
    declaration: DockDeclaration,
    tabs: HashMap<TabId, usize>,
    hosts: HashMap<TabId, usize>,
    dialogs: HashMap<TabId, usize>,
    bars: HashMap<TabId, usize>,
}

impl Declared {
    fn new(declaration: DockDeclaration) -> Self {
        let index = |tabs: Vec<TabId>| {
            tabs.into_iter()
                .enumerate()
                .map(|(at, tab)| (tab, at))
                .collect::<HashMap<_, _>>()
        };
        Self {
            tabs: index(declaration.tabs.iter().map(|tab| tab.tab).collect()),
            hosts: index(declaration.hosts.iter().map(|host| host.tab.tab).collect()),
            dialogs: index(
                declaration
                    .dialogs
                    .iter()
                    .map(|dialog| dialog.tab)
                    .collect(),
            ),
            bars: index(declaration.bars.iter().map(|bar| bar.tab).collect()),
            declaration,
        }
    }

    fn tab(&self, tab: TabId) -> Option<&beui_dock::DockDeclaredTab> {
        if let Some(at) = self.tabs.get(&tab) {
            return self.declaration.tabs.get(*at);
        }
        let at = self.hosts.get(&tab)?;
        self.declaration.hosts.get(*at).map(|host| &host.tab)
    }

    fn dialog(&self, tab: TabId) -> Option<&DockDeclaredDialog> {
        self.declaration.dialogs.get(*self.dialogs.get(&tab)?)
    }

    fn bar(&self, tab: TabId) -> Option<&beui_dock::DockDeclaredBar> {
        self.declaration.bars.get(*self.bars.get(&tab)?)
    }

    fn knows(&self, tab: TabId) -> bool {
        self.tabs.contains_key(&tab)
            || self.hosts.contains_key(&tab)
            || self.dialogs.contains_key(&tab)
            || self.bars.contains_key(&tab)
    }

    fn frames(&self, tab: TabId) -> bool {
        (self.tabs.contains_key(&tab) || self.hosts.contains_key(&tab))
            && !self.declaration.spec.see_through.contains(&tab)
    }
}

#[derive(Clone, PartialEq)]
struct Structure {
    spec: DockSpec,
    hosts: Vec<(TabId, DockSpecNode, Option<TabId>)>,
    bar_kinds: Vec<(TabId, String)>,
}

impl Structure {
    fn of(declaration: &DockDeclaration) -> Self {
        Self {
            spec: declaration.spec.clone(),
            hosts: declaration
                .hosts
                .iter()
                .map(|host| (host.tab.tab, host.root.clone(), host.focus))
                .collect(),
            bar_kinds: declaration
                .bars
                .iter()
                .map(|bar| (bar.tab, bar.kind.clone()))
                .collect(),
        }
    }
}

struct HeldDialog {
    described: Signal<DockDeclaredDialog>,
    parts: DialogParts,
    _scope: Scope,
}

struct HostInner {
    _scope: Scope,
    bodies: RefCell<Option<DockBodies>>,
    placed: Signal<u64>,
    events: RefCell<Option<Callback<DockHostEvent>>>,
    dialogs: RefCell<HashMap<TabId, HeldDialog>>,
    dock: Cell<Option<ComponentRef<DockApi>>>,
    bars_link: RefCell<Option<Rc<dyn DockHostLink>>>,
    bars_hosted: Signal<Option<DockHosted>>,
    structure: RefCell<Option<Structure>>,
    hosts: Signal<Vec<TabId>>,
}

pub struct DockHostLayout {
    state: Signal<DockState>,
    mode: Signal<DockMode>,
    covers: Signal<BarCovers>,
    floats: Signal<BarFloats>,
    covered: Signal<bool>,
    area: Signal<Rect>,
    snaps: Signal<HashMap<TabId, Snapped>>,
    areas: Signal<Vec<Rect>>,
    cleared: Signal<Option<Vec<TabId>>>,
    declared: Signal<Option<Rc<Declared>>>,
    inner: Stored<HostInner>,
}

impl Clone for DockHostLayout {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for DockHostLayout {}

impl Default for DockHostLayout {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn standing_in(state: &DockState, tab: TabId) -> TabId {
    if state.contains(tab) {
        return tab;
    }
    let Some(group) = state.find_group(&joined_key(tab)) else {
        return tab;
    };
    let inside = state.group_tabs(group);
    state
        .recent_tabs()
        .into_iter()
        .find(|recent| inside.contains(recent))
        .or_else(|| inside.first().copied())
        .unwrap_or(tab)
}

impl DockHostLayout {
    pub fn new() -> Self {
        let owner = RootScope::new();
        let inner = HostInner {
            _scope: owner.into_scope(),
            bodies: RefCell::default(),
            placed: create_signal(0),
            events: RefCell::default(),
            dialogs: RefCell::default(),
            dock: Cell::new(None),
            bars_link: RefCell::default(),
            bars_hosted: create_signal(None),
            structure: RefCell::default(),
            hosts: create_signal(Vec::new()),
        };
        Self {
            state: create_signal(DockState::default()),
            mode: create_signal(DockMode::Tiled),
            covers: create_signal(BarCovers::default()),
            floats: create_signal(BarFloats::default()),
            covered: create_signal(false),
            area: create_signal(Rect::ZERO),
            snaps: create_signal(HashMap::default()),
            areas: create_signal(Vec::new()),
            cleared: create_signal(None),
            declared: create_signal(None),
            inner: store(inner),
        }
    }

    fn inner(&self) -> Rc<HostInner> {
        self.inner.value()
    }

    pub fn state(&self) -> ReadSignal<DockState> {
        self.state.read_only()
    }

    pub fn mode(&self) -> ReadSignal<DockMode> {
        self.mode.read_only()
    }

    pub fn area(&self) -> Rect {
        self.area.get()
    }

    pub fn set_covered(&self, covered: bool) {
        if self.covered.get_untracked() != covered {
            self.covered.set(covered);
        }
    }

    pub fn over_bars_rect(&self, edge: DockEdge, align: DockAlign) -> Option<Rect> {
        self.floats
            .with(|floats| floats.get(&(edge, align)).copied())
    }

    pub fn window_rect(&self, tab: TabId) -> Option<Rect> {
        let rect = self.state.with(|state| {
            let surface = state.find(standing_in(state, tab))?.surface;
            state.window_rect(surface)
        })?;
        Some(rect.translate(self.area.get().min.to_vec2()))
    }

    pub fn move_window(&self, tab: TabId, to: Pos2) {
        let Some(rect) = untrack(|| self.window_rect(tab)) else {
            return;
        };
        self.place_window(tab, Rect::from_min_size(to, rect.size()));
    }

    pub fn restore(&self, state: DockState) {
        self.apply(DockRequest::Restore(state));
    }

    pub fn focused(&self) -> Option<TabId> {
        self.state.with(DockState::focused_tab)
    }

    pub fn fullscreen(&self) -> Option<TabId> {
        Some(self.state.with(DockState::fullscreen)?.tab)
    }

    pub fn enter_fullscreen(&self, tab: TabId, area: Option<Rect>) {
        self.edit(|state| state.set_fullscreen(Some(DockFullscreen { tab, area })));
    }

    pub fn leave_fullscreen(&self) {
        self.edit(|state| state.set_fullscreen(None));
    }

    pub fn show(&self, tab: TabId) {
        self.edit(|state| {
            let shown = standing_in(state, tab);
            state.show(shown);
        });
    }

    fn edit(&self, change: impl FnOnce(&mut DockState)) {
        let current = self.state.get_untracked();
        let mut next = current.clone();
        change(&mut next);
        if next != current {
            self.state.set(next);
        }
    }

    fn close(&self, tab: TabId) {
        let events = *self.inner().events.borrow();
        if let Some(events) = events {
            events.call(DockHostEvent::Close(tab));
        }
    }

    fn declared_group(&self, tab: TabId) -> Option<String> {
        self.declared
            .with(|declared| declared.as_ref()?.tab(tab)?.group.clone())
    }

    fn dialog_parent(&self, tab: TabId) -> Option<TabId> {
        self.declared
            .with(|declared| Some(declared.as_ref()?.dialog(tab)?.parent))
    }

    fn focused_dialog(&self) -> Option<TabId> {
        self.inner()
            .dock
            .get()
            .and_then(|dock| dock.try_get())
            .and_then(|dock| dock.focused_dialog())
    }

    fn bars_link(&self) -> Option<Rc<dyn DockHostLink>> {
        let inner = self.inner();
        let hosted = inner.bars_hosted.with_untracked(Option::is_some);
        inner.bars_link.borrow().clone().filter(|_| hosted)
    }

    fn apply(&self, request: DockRequest) {
        let bar = match &request {
            DockRequest::PlaceBar(bar, _)
            | DockRequest::MoveBar(bar, _)
            | DockRequest::HideBar(bar, _) => Some(*bar),
            DockRequest::Show(_) | DockRequest::Restore(_) | DockRequest::Fullscreen(_) => None,
        };
        if let Some(bar) = bar {
            let own = self.state.with_untracked(|state| state.is_dock_bar(bar));
            if let Some(link) = self.bars_link().filter(|_| !own) {
                link.request(request);
                return;
            }
            if let Some(dock) = self.inner().dock.get().and_then(|dock| dock.try_get()) {
                dock.ask_bar(request);
                return;
            }
        }
        match request {
            DockRequest::Show(tab) => self.show(tab),
            DockRequest::Restore(state) => {
                self.state.set(state);
                let structure = self.inner().structure.borrow().clone();
                if let Some(structure) = structure {
                    self.reconcile(&structure);
                }
            }
            DockRequest::Fullscreen(Some((tab, area))) => self.enter_fullscreen(tab, area),
            DockRequest::Fullscreen(None) => self.leave_fullscreen(),
            DockRequest::PlaceBar(bar, Some(placement)) => {
                self.edit(|state| state.set_bar_placement(bar, placement));
            }
            DockRequest::HideBar(bar, hidden) => {
                self.edit(|state| state.set_bar_hidden(bar, hidden));
            }
            DockRequest::PlaceBar(_, None) | DockRequest::MoveBar(..) => {}
        }
    }

    fn reconcile(&self, structure: &Structure) {
        let current = self.state.get_untracked();
        let mut next = current.clone();
        let live: HashSet<TabId> = structure.hosts.iter().map(|(host, _, _)| *host).collect();
        while let Some((group, host)) = next
            .joined_groups()
            .into_iter()
            .find(|(_, host)| !live.contains(host))
        {
            next.unhost_group(group, host);
        }
        for (host, root, focus) in &structure.hosts {
            let key = joined_key(*host);
            if next.find_group(&key).is_none() && next.contains(*host) {
                next.host_group(*host, &key, root, *focus);
            }
        }
        next.reconcile(&structure.spec);
        let kinds: HashMap<TabId, String> = structure.bar_kinds.iter().cloned().collect();
        let bar_key = |bar: TabId| super::bars::shared_key(kinds.get(&bar).cloned(), bar);
        next.adopt_bar_placements(&current, &bar_key);
        let hosts: Vec<TabId> = structure.hosts.iter().map(|(host, _, _)| *host).collect();
        let held = self.inner().hosts;
        if held.with_untracked(|held| *held != hosts) {
            held.set(hosts);
        }
        if next != current {
            self.state.set(next);
        }
    }

    fn hosted_now(&self) -> DockHosted {
        let mode = self.mode.get();
        let covers = self.covers.get();
        let above = self.inner().bars_hosted.get();
        self.state.with(|state| {
            let mut shared = state.clone();
            if let Some(above) = &above {
                for bar in state.bars() {
                    if state.is_dock_bar(bar) {
                        continue;
                    }
                    if let Some(placement) = above.state.bar_placement(bar) {
                        shared.set_bar_placement(bar, placement);
                    }
                    shared.set_bar_hidden(bar, above.state.is_bar_hidden(bar));
                }
            }
            let tabs = state.all_tabs();
            DockHosted {
                tabs: tabs.clone(),
                focused: state.focused_tab(),
                shown: state.stacked_tab(),
                visible: tabs
                    .iter()
                    .copied()
                    .filter(|tab| state.is_shown(*tab, mode))
                    .collect(),
                fullscreen: state.fullscreen().map(|shown| shown.tab),
                mode,
                covers: covers
                    .iter()
                    .map(|((tab, ..), rect)| (*tab, *rect))
                    .collect(),
                state: shared,
            }
        })
    }

    fn refresh_dialogs(&self) {
        let inner = self.inner();
        self.declared.with(|declared| {
            let Some(declared) = declared else {
                return;
            };
            for (tab, held) in inner.dialogs.borrow().iter() {
                if let Some(dialog) = declared.dialog(*tab)
                    && held
                        .described
                        .with_untracked(|described| described != dialog)
                {
                    held.described.set(dialog.clone());
                }
            }
        });
    }

    fn held_dialog(&self, tab: TabId) -> Option<DialogParts> {
        let declared = self
            .declared
            .with_untracked(|declared| declared.as_ref()?.dialog(tab).cloned())?;
        let inner = self.inner();
        if let Some(held) = inner.dialogs.borrow().get(&tab) {
            if held
                .described
                .with_untracked(|described| *described != declared)
            {
                held.described.set(declared);
            }
            return Some(held.parts);
        }
        let scope = inner._scope.context().child()?;
        let layout = *self;
        let closable = declared.closable;
        let (described, parts) = scope.run(|| {
            let described = create_signal(declared);
            let read = move |read: fn(&DockDeclaredDialog) -> bool| {
                create_memo(move || described.with(read))
            };
            let parts = DialogParts {
                title: create_memo(move || described.with(|described| described.title.clone())),
                icon: create_memo(move || described.with(|described| described.icon)),
                size: create_memo(move || described.with(|described| described.size)),
                modal: read(|described| described.modal),
                dismiss_on_outside_press: read(|described| described.dismiss_on_outside_press),
                on_close: closable.then(|| ClickCallback::new(move || layout.close(tab))),
                content: RenderFn::new(move |()| layout.body(tab)),
            };
            (described, parts)
        });
        inner.dialogs.borrow_mut().insert(
            tab,
            HeldDialog {
                described,
                parts,
                _scope: scope,
            },
        );
        Some(parts)
    }

    fn body(&self, tab: TabId) -> NodeId {
        let bodies = self.inner().bodies.borrow().clone();
        match bodies {
            Some(bodies) => bodies.body.call(tab),
            None => view! {
                <Frame />
            },
        }
    }
}

#[derive(Clone)]
struct HostStyle {
    api: ComponentRef<DockApi>,
    stack_below: f32,
    short_below: f32,
    short_narrower_than: f32,
    drag_modifier: Prop<Option<Modifiers>>,
    menu: MenuStyle,
    splitter_thickness: f32,
    splitter_reach: f32,
    group_inset: f32,
    tab_spacing: f32,
    drop_marker_width: f32,
    back_shade: Prop<Color>,
    icon_sidebar_width: f32,
    inset: Prop<f32>,
    tab: RenderFn<DockTabHandle>,
    frame: Option<RenderFn<NodeId>>,
    fullscreen: Option<RenderFn<NodeId>>,
    away: Option<RenderFn<DockAwayHandle>>,
    chrome: Option<RenderFn<DockChromeHandle>>,
    bar: Option<RenderFn<DockBarHandle>>,
    splitter: Option<RenderFn<DockSplitterHandle>>,
    strip: Option<RenderFn<DockStripHandle>>,
    grip: Option<RenderFn<DockGripHandle>>,
    highlight: Option<RenderFn<()>>,
    refused: Option<RenderFn<()>>,
    preview: Option<RenderFn<DockPreviewHandle>>,
    stack: Option<RenderFn<DockStackHandle>>,
    switcher: Option<RenderFn<DockSwitcherHandle>>,
    switch: Option<RenderFn<DockSwitchHandle>>,
    dialog_chrome: Option<RenderFn<DockDialogHandle>>,
    dialog_beak: f32,
    dialog_radius: f32,
    dialog_scrim: Prop<Color>,
    window_border: f32,
    bar_margin: f32,
    bar_band: Option<RenderFn<DockBarBandHandle>>,
}

#[derive(Clone)]
struct BarsAbove(Rc<dyn DockHostLink>);

struct LocalLink {
    layout: DockHostLayout,
    hosted: Memo<Option<DockHosted>>,
    style: HostStyle,
    bars: Option<Rc<dyn DockHostLink>>,
}

impl DockHostLink for LocalLink {
    fn declare(&self, declaration: DockDeclaration) {
        let declared = Rc::new(Declared::new(declaration));
        let changed = self
            .layout
            .declared
            .with_untracked(|held| held.as_deref() != Some(&*declared));
        if !changed {
            return;
        }
        self.layout.declared.set(Some(declared));
    }

    fn hosted(&self) -> Memo<Option<DockHosted>> {
        self.hosted
    }

    fn request(&self, request: DockRequest) {
        self.layout.apply(request);
    }

    fn listen(&self, events: Callback<DockHostEvent>) {
        *self.layout.inner().events.borrow_mut() = Some(events);
    }

    fn place(&self, bodies: DockBodies) {
        let inner = self.layout.inner();
        *inner.bodies.borrow_mut() = Some(bodies);
        inner.placed.update(|placed| *placed += 1);
        let joined = inner.bars_link.borrow().is_some();
        if let Some(bars) = self.bars.clone().filter(|_| !joined) {
            join_bars(self.layout, bars);
        }
    }

    fn withdraw(&self) {
        if self.layout.declared.is_alive() {
            self.layout.declared.set(None);
        }
    }

    fn view(&self) -> Option<NodeId> {
        let layout = self.layout;
        let style = self.style.clone();
        Some(view! {
            <HostedDock layout style />
        })
    }
}

#[component]
fn HostedDock(layout: DockHostLayout, style: HostStyle) -> NodeId {
    let HostStyle {
        api,
        stack_below,
        short_below,
        short_narrower_than,
        drag_modifier,
        menu,
        splitter_thickness,
        splitter_reach,
        group_inset,
        tab_spacing,
        drop_marker_width,
        back_shade,
        icon_sidebar_width,
        inset,
        tab,
        frame,
        fullscreen,
        away,
        chrome,
        bar,
        splitter,
        strip,
        grip,
        highlight,
        refused,
        preview,
        stack,
        switcher,
        switch,
        dialog_chrome,
        dialog_beak,
        dialog_radius,
        dialog_scrim,
        window_border,
        bar_margin,
        bar_band,
    } = style;
    let inner = layout.inner();
    inner.dock.set(Some(api));
    let declared = layout.declared;
    let placed = inner.placed;
    let bars_hosted = inner.bars_hosted;
    let bars_joined = inner.bars_link.borrow().is_some();
    let read = move |tab: TabId| declared.with(|declared| declared.as_ref()?.tab(tab).cloned());
    let title = Func::new(move |tab: TabId| read(tab).map(|tab| tab.title).unwrap_or_default());
    let icon = Func::new(move |tab: TabId| read(tab).and_then(|tab| tab.icon));
    let picture = Func::new(move |tab: TabId| read(tab).and_then(|tab| tab.picture));
    let closable = Func::new(move |tab: TabId| read(tab).is_some_and(|tab| tab.closable));
    let back_closes = Func::new(move |tab: TabId| read(tab).is_none_or(|tab| tab.back_closes));
    let group_host = Func::new(move |group: GroupId| {
        layout
            .state
            .with(|state| state.group_key(group).and_then(joined_host))
    });
    let group_title = Func::new(move |group: GroupId| {
        let key = layout
            .state
            .with(|state| state.group_key(group).map(str::to_owned))?;
        if let Some(host) = joined_host(&key) {
            return Some(title.call(host));
        }
        declared.with(|declared| {
            declared
                .as_ref()?
                .declaration
                .group_title(&key)
                .map(str::to_owned)
        })
    });
    let place = move |tab: TabId| {
        let node = layout.body(tab);
        let framed = declared.with_untracked(|declared| {
            declared
                .as_ref()
                .is_some_and(|declared| declared.frames(tab))
        });
        let node = match &frame {
            Some(frame) if framed => frame.call(node),
            _ => node,
        };
        if bars_joined {
            let covers = create_memo(move || {
                bars_hosted.with(|hosted| {
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
            keep_clear(node, covers);
        }
        node
    };
    let content = RenderFn::new(move |tab: TabId| {
        let known = move || {
            placed.get();
            layout.inner().bodies.borrow().is_some()
                && declared.with(|declared| {
                    declared
                        .as_ref()
                        .is_some_and(|declared| declared.knows(tab))
                })
        };
        if untrack(known) {
            return place(tab);
        }
        let ready = create_memo(known);
        view! {
            <Layers>
                <Dynamic value={ready}>|ready: bool| {
                    match ready {
                        true => place(tab),
                        false => view! {
                            <Frame />
                        },
                    }
                }</Dynamic>
            </Layers>
        }
    });
    let empty = RenderFn::new(move |leaf: LeafId| {
        let key = layout
            .state
            .with_untracked(|state| state.pane_key(leaf).map(str::to_owned));
        let bodies = layout.inner().bodies.borrow().clone();
        let built = key
            .zip(bodies)
            .and_then(|(key, bodies)| bodies.empty.call(key));
        built.unwrap_or_else(|| {
            view! {
                <Frame />
            }
        })
    });
    let held_hosts = inner.hosts;
    let hosts = create_memo(move || held_hosts.get());
    let dialogs = create_memo(move || {
        let held: Vec<(TabId, TabId)> = declared.with(|declared| {
            declared.as_ref().map_or_else(Vec::new, |declared| {
                declared
                    .declaration
                    .dialogs
                    .iter()
                    .map(|dialog| (dialog.tab, dialog.parent))
                    .collect()
            })
        });
        layout.state.with(|state| {
            held.into_iter()
                .map(|(dialog, parent)| (dialog, standing_in(state, parent)))
                .collect::<Vec<_>>()
        })
    });
    let dialog = Func::new(move |tab: TabId| layout.held_dialog(tab));
    let bar_key = Func::new(move |bar: TabId| {
        declared.with(|declared| Some(declared.as_ref()?.bar(bar)?.kind.clone()))
    });
    let tab_key = Func::new(move |tab: TabId| {
        declared.with(|declared| {
            let declared = declared.as_ref()?;
            match declared.tab(tab) {
                Some(found) => Some(found.key.clone()),
                None => Some(declared.dialog(tab)?.key.clone()).filter(|key| !key.is_empty()),
            }
        })
    });
    view! {
        <Dock
            config={DockConfig {
                api,
                state: Binding::from(layout.state),
                on_close: Callback::new(move |tab: TabId| layout.close(tab)),
                title,
                group_title,
                group_host,
                hosts,
                icon,
                picture,
                closable,
                back_closes,
                menu,
                stack_below,
                short_below,
                short_narrower_than,
                mode: layout.mode,
                drag_modifier,
                splitter_thickness,
                splitter_reach,
                group_inset,
                tab_spacing,
                marker_width: drop_marker_width,
                back_shade: create_memo(move || back_shade.get()),
                icon_sidebar_width,
                inset,
                tab,
                content,
                empty,
                fullscreen,
                away,
                chrome,
                bar,
                splitter,
                strip,
                grip,
                highlight,
                refused,
                preview,
                stack,
                switcher,
                switch,
                dialogs,
                dialog,
                dialog_chrome,
                dialog_beak,
                dialog_radius,
                dialog_scrim: create_memo(move || dialog_scrim.get()),
                window_border,
                bar_margin,
                bar_band,
                covers: layout.covers,
                floats: layout.floats,
                covered: layout.covered,
                area: layout.area,
                bars_away: create_memo(move || bars_hosted.with(Option::is_some)),
                bar_key,
                tab_key,
            }}
        />
    }
}

fn join_bars(layout: DockHostLayout, link: Rc<dyn DockHostLink>) {
    let inner = layout.inner();
    *inner.bars_link.borrow_mut() = Some(Rc::clone(&link));
    let hosted = link.hosted();
    let held = inner.bars_hosted;
    create_effect(move || {
        let hosted = hosted.get();
        untrack(|| held.set(hosted));
    });
    let declaring = Rc::clone(&link);
    let declared = layout.declared;
    create_effect(move || {
        let Some(mut declaration) = declared.with(|declared| {
            declared
                .as_ref()
                .map(|declared| declared.declaration.clone())
        }) else {
            return;
        };
        declaration.spec.bars.retain(|bar| bar.owner.is_some());
        declaration.bars.retain(|bar| bar.owner.is_some());
        if declaration.bars.is_empty() {
            return;
        }
        untrack(|| declaring.declare(declaration));
    });
    if let Some(scope) = owner_scope() {
        link.place(DockBodies {
            scope,
            body: RenderFn::new(move |bar: TabId| layout.body(bar)),
            empty: RenderFn::new(|_: String| None),
            hosts: create_memo(Vec::new),
        });
    }
    let kept = layout.inner;
    on_cleanup(move || {
        link.withdraw();
        if let Some(inner) = kept.try_value() {
            inner.bars_link.borrow_mut().take();
            if inner.bars_hosted.is_alive() {
                inner.bars_hosted.set(None);
            }
        }
    });
}

#[component]
pub fn DockHost(
    #[prop(default = DockHostLayout::new())] layout: DockHostLayout,
    #[prop(default = ComponentRef::new())] api: ComponentRef<DockApi>,
    bars: Option<Rc<dyn DockHostLink>>,
    #[prop(default = 0.0)] stack_below: f32,
    #[prop(default = 0.0)] short_below: f32,
    #[prop(default = 0.0)] short_narrower_than: f32,
    #[prop(default = None)] drag_modifier: Prop<Option<Modifiers>>,
    menu: MenuStyle,
    splitter_thickness: f32,
    #[prop(default = 0.0)] splitter_reach: f32,
    #[prop(default = 0.0)] group_inset: f32,
    tab_spacing: f32,
    drop_marker_width: f32,
    back_shade: Prop<Color>,
    icon_sidebar_width: f32,
    #[prop(default = 0.0)] inset: Prop<f32>,
    tab: RenderFn<DockTabHandle>,
    frame: Option<RenderFn<NodeId>>,
    fullscreen: Option<RenderFn<NodeId>>,
    away: Option<RenderFn<DockAwayHandle>>,
    chrome: Option<RenderFn<DockChromeHandle>>,
    bar: Option<RenderFn<DockBarHandle>>,
    splitter: Option<RenderFn<DockSplitterHandle>>,
    strip: Option<RenderFn<DockStripHandle>>,
    grip: Option<RenderFn<DockGripHandle>>,
    highlight: Option<RenderFn<()>>,
    refused: Option<RenderFn<()>>,
    preview: Option<RenderFn<DockPreviewHandle>>,
    stack: Option<RenderFn<DockStackHandle>>,
    switcher: Option<RenderFn<DockSwitcherHandle>>,
    switch: Option<RenderFn<DockSwitchHandle>>,
    dialog_chrome: Option<RenderFn<DockDialogHandle>>,
    #[prop(default = 0.0)] dialog_beak: f32,
    #[prop(default = 0.0)] dialog_radius: f32,
    #[prop(default = Color::TRANSPARENT)] dialog_scrim: Prop<Color>,
    #[prop(default = 0.0)] window_border: f32,
    #[prop(default = 0.0)] bar_margin: f32,
    bar_band: Option<RenderFn<DockBarBandHandle>>,
    #[prop(children)] content: Render,
) -> NodeId {
    let joined =
        use_context::<DockHostSlot>().is_some_and(|slot| !slot.is_local() && !slot.is_claimed());
    if joined {
        return content.call(());
    }
    let style = HostStyle {
        api,
        stack_below,
        short_below,
        short_narrower_than,
        drag_modifier,
        menu,
        splitter_thickness,
        splitter_reach,
        group_inset,
        tab_spacing,
        drop_marker_width,
        back_shade,
        icon_sidebar_width,
        inset,
        tab,
        frame,
        fullscreen,
        away,
        chrome,
        bar,
        splitter,
        strip,
        grip,
        highlight,
        refused,
        preview,
        stack,
        switcher,
        switch,
        dialog_chrome,
        dialog_beak,
        dialog_radius,
        dialog_scrim,
        window_border,
        bar_margin,
        bar_band,
    };
    let structure = create_memo(move || {
        layout
            .declared
            .with(|declared| Some(Structure::of(&declared.as_ref()?.declaration)))
    });
    create_effect(move || {
        let Some(structure) = structure.get() else {
            return;
        };
        untrack(|| {
            layout.reconcile(&structure);
            *layout.inner().structure.borrow_mut() = Some(structure);
        });
    });
    create_effect(move || layout.refresh_dialogs());
    let hosted = create_memo(move || Some(layout.hosted_now()));
    let bars = bars.or_else(|| use_context::<BarsAbove>().map(|above| above.0));
    if let Some(bars) = &bars {
        provide_context(BarsAbove(Rc::clone(bars)));
    }
    let link = Rc::new(LocalLink {
        layout,
        hosted,
        style,
        bars,
    });
    provide_context(DockJoin::none());
    provide_context(DockHostSlot::local(link));
    content.call(())
}
