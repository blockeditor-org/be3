use be_lib_stable_hash::HashMap;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui_dock_state::{
    DockBarPlacement, DockMapping, DockMode, DockSpec, DockSpecNode, DockState, TabId, joined_host,
    joined_key, joined_prefix,
};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Image;
use beui_view::types::icons::Glyph;
use beui_view::types::{Pos2, Rect, Vec2};

#[derive(Clone, Debug, PartialEq)]
pub struct DockDeclaration {
    pub spec: DockSpec,
    pub tabs: Vec<DockDeclaredTab>,
    pub dialogs: Vec<DockDeclaredDialog>,
    pub bars: Vec<DockDeclaredBar>,
    pub groups: Vec<DockDeclaredGroup>,
    pub hosts: Vec<DockDeclaredHost>,
}

impl DockDeclaration {
    pub fn tab(&self, tab: TabId) -> Option<&DockDeclaredTab> {
        self.tabs.iter().find(|declared| declared.tab == tab)
    }

    pub fn dialog(&self, tab: TabId) -> Option<&DockDeclaredDialog> {
        self.dialogs.iter().find(|declared| declared.tab == tab)
    }

    pub fn bar(&self, tab: TabId) -> Option<&DockDeclaredBar> {
        self.bars.iter().find(|declared| declared.tab == tab)
    }

    pub fn host(&self, tab: TabId) -> Option<&DockDeclaredHost> {
        self.hosts.iter().find(|declared| declared.tab.tab == tab)
    }

    pub fn group_title(&self, key: &str) -> Option<&str> {
        self.groups
            .iter()
            .find(|declared| declared.key == key)
            .map(|declared| declared.title.as_str())
    }
}

/// `owner` is `None` for a dock bar: the root gives it to the hosting tab. `kind` is what the root
/// places bars by: the declaring dock's `key()` in-process, `{block type}/{key}` across apps, so the
/// same bar of every instance of an editor shares one placement.
#[derive(Clone, Debug, PartialEq)]
pub struct DockDeclaredBar {
    pub tab: TabId,
    pub key: String,
    pub kind: String,
    pub owner: Option<TabId>,
    pub placement: DockBarPlacement,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DockDeclaredTab {
    pub tab: TabId,
    pub key: String,
    pub title: String,
    pub icon: Option<Glyph>,
    pub picture: Option<Image>,
    pub closable: bool,
    pub back_closes: bool,
    pub group: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DockDeclaredDialog {
    pub tab: TabId,
    pub key: String,
    pub parent: TabId,
    pub title: String,
    pub icon: Option<Glyph>,
    pub size: Vec2,
    pub modal: bool,
    pub dismiss_on_outside_press: bool,
    pub closable: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DockDeclaredGroup {
    pub key: String,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DockDeclaredHost {
    pub tab: DockDeclaredTab,
    pub root: DockSpecNode,
    pub focus: Option<TabId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DockHosted {
    pub state: DockState,
    pub tabs: Vec<TabId>,
    pub focused: Option<TabId>,
    pub shown: Option<TabId>,
    pub visible: Vec<TabId>,
    pub fullscreen: Option<TabId>,
    pub mode: DockMode,
    pub covers: Vec<(TabId, Rect)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DockRequest {
    Show(TabId),
    Restore(DockState),
    Fullscreen(Option<(TabId, Option<Rect>)>),
    PlaceBar(TabId, Option<DockBarPlacement>),
    MoveBar(TabId, DockBarMove),
    HideBar(TabId, bool),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DockBarMove {
    pub at: Pos2,
    pub stage: DockDragStage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DockDragStage {
    Move,
    Drop,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DockHostEvent {
    Close(TabId),
}

#[derive(Clone)]
pub struct DockBodies {
    pub scope: ScopeContext,
    pub body: RenderFn<TabId, NodeId>,
    pub empty: RenderFn<String, Option<NodeId>>,
    pub hosts: Memo<Vec<TabId>>,
}

/// The seam every `Docking` declares through, plain data both ways. A host only ever sees one
/// declaration: nested docks are spliced in by the client.
///
/// `withdraw` takes back only the declaration; the bodies `place` gave and the `listen` callback stay
/// until the claiming scope ends, so a dock torn down and rebuilt is hosted again with the same
/// bodies.
pub trait DockHostLink {
    fn declare(&self, declaration: DockDeclaration);
    fn hosted(&self) -> Memo<Option<DockHosted>>;
    fn request(&self, request: DockRequest);
    fn listen(&self, events: Callback<DockHostEvent>);
    fn place(&self, bodies: DockBodies);
    fn withdraw(&self);
    fn view(&self) -> Option<NodeId> {
        None
    }
}

/// The context a `Docking` not inside another dock's tab declares through; a `Docking` with neither
/// a slot nor a `DockJoin` panics. `local` is a `DockHost`'s slot, which draws the dock in place of
/// the `Docking`; `new` is a platform's, which draws nothing there and hosts the dock elsewhere.
#[derive(Clone)]
pub struct DockHostSlot {
    link: Rc<dyn DockHostLink>,
    claimed: Rc<Cell<bool>>,
    asked: Rc<Cell<bool>>,
    local: bool,
}

impl DockHostSlot {
    pub fn new(link: Rc<dyn DockHostLink>) -> Self {
        Self {
            link,
            claimed: Rc::default(),
            asked: Rc::default(),
            local: false,
        }
    }

    pub fn local(link: Rc<dyn DockHostLink>) -> Self {
        Self {
            local: true,
            ..Self::new(link)
        }
    }

    pub fn is_local(&self) -> bool {
        self.local
    }

    pub fn asked(&self) -> bool {
        self.asked.get()
    }

    pub fn is_claimed(&self) -> bool {
        self.claimed.get()
    }

    pub fn claim(&self) -> Option<Rc<dyn DockHostLink>> {
        self.asked.set(true);
        if self.claimed.replace(true) {
            return None;
        }
        let claimed = Rc::clone(&self.claimed);
        on_cleanup(move || claimed.set(false));
        Some(Rc::clone(&self.link))
    }
}

#[derive(Clone, Copy)]
pub struct DockKept {
    pub layout: Memo<DockState>,
    pub keys: Memo<Vec<(TabId, String)>>,
    pub restore: Callback<DockState>,
}

/// Saves the layout of the dock a `DockHost` hosts below it, as it would a joined one: the first
/// locally hosted `Docking` claims it (a dock nested in that one joins it and claims none).
/// be-sdk-beui provides one in every editor's frame; an owner that saves its dock itself provides
/// `DockKeeper::none()` above it.
#[derive(Clone)]
pub struct DockKeeper {
    keep: Option<Rc<dyn Fn(DockKept)>>,
    claimed: Rc<Cell<bool>>,
}

impl DockKeeper {
    pub fn new(keep: impl Fn(DockKept) + 'static) -> Self {
        Self {
            keep: Some(Rc::new(keep)),
            claimed: Rc::default(),
        }
    }

    pub fn none() -> Self {
        Self {
            keep: None,
            claimed: Rc::default(),
        }
    }

    pub(crate) fn claim(&self) -> Option<Rc<dyn Fn(DockKept)>> {
        if self.claimed.replace(true) {
            return None;
        }
        let claimed = Rc::clone(&self.claimed);
        on_cleanup(move || claimed.set(false));
        self.keep.clone()
    }
}

#[derive(Default)]
pub(crate) struct Joined {
    pub(crate) declaration: RefCell<Option<DockDeclaration>>,
    pub(crate) bodies: RefCell<Option<DockBodies>>,
    pub(crate) events: RefCell<Option<Callback<DockHostEvent>>>,
    pub(crate) pending: RefCell<Option<DockState>>,
}

impl Joined {
    pub(crate) fn close(&self, tab: TabId) {
        let events = *self.events.borrow();
        if let Some(events) = events {
            events.call(DockHostEvent::Close(tab));
        }
    }
}

#[derive(Default)]
pub(crate) struct Foreign {
    outer: HashMap<(TabId, TabId), TabId>,
    inner: HashMap<TabId, (TabId, TabId)>,
}

impl Foreign {
    pub(crate) fn outer(&self, host: TabId, inner: TabId) -> Option<TabId> {
        self.outer.get(&(host, inner)).copied()
    }

    pub(crate) fn inner(&self, outer: TabId) -> Option<(TabId, TabId)> {
        self.inner.get(&outer).copied()
    }

    pub(crate) fn contains(&self, outer: TabId) -> bool {
        self.inner.contains_key(&outer)
    }

    pub(crate) fn newest(&self) -> u64 {
        self.inner.keys().map(|tab| tab.value()).max().unwrap_or(0)
    }

    pub(crate) fn insert(&mut self, host: TabId, inner: TabId, outer: TabId) {
        self.outer.insert((host, inner), outer);
        self.inner.insert(outer, (host, inner));
    }

    pub(crate) fn retain(&mut self, kept: impl Fn(TabId) -> bool) {
        self.inner.retain(|outer, _| kept(*outer));
        self.outer.retain(|_, outer| kept(*outer));
    }
}

pub(crate) struct Registry {
    pub(crate) version: Signal<u64>,
    pub(crate) joins: RefCell<HashMap<TabId, Rc<Joined>>>,
    pub(crate) foreign: RefCell<Foreign>,
    pub(crate) state: Signal<DockState>,
    pub(crate) above: Signal<Option<DockHosted>>,
    pub(crate) host: RefCell<Option<Rc<dyn DockHostLink>>>,
}

impl Registry {
    pub(crate) fn bump(&self) {
        if self.version.is_alive() {
            self.version.update(|version| *version += 1);
        }
    }

    pub(crate) fn joined(&self, host: TabId) -> Option<Rc<Joined>> {
        self.joins.borrow().get(&host).cloned()
    }

    pub(crate) fn keep_joined(&self) {
        let joins: Vec<(TabId, Rc<Joined>)> = self
            .joins
            .borrow()
            .iter()
            .map(|(host, joined)| (*host, Rc::clone(joined)))
            .collect();
        for (host, joined) in joins {
            if let Some(hosted) = untrack(|| self.hosted(host)) {
                *joined.pending.borrow_mut() = Some(hosted.state);
            }
        }
    }

    pub(crate) fn graft(&self, state: &mut DockState, host: TabId, from: &DockState) -> bool {
        let Some(group) = state.find_group(&joined_key(host)) else {
            return false;
        };
        let foreign = self.foreign.borrow();
        let prefix = joined_prefix(host);
        state.graft(
            group,
            from,
            &DockMapping {
                tab: &|inner| foreign.outer(host, inner),
                key: &|key| Some(format!("{prefix}{key}")),
                group: &|_| None,
            },
        );
        true
    }

    pub(crate) fn graft_pending(&self, state: &DockState) -> Option<DockState> {
        let joins: Vec<(TabId, Rc<Joined>)> = self
            .joins
            .borrow()
            .iter()
            .map(|(host, joined)| (*host, Rc::clone(joined)))
            .collect();
        let mut next = state.clone();
        let mut grafted = false;
        for (host, joined) in joins {
            if next.find_group(&joined_key(host)).is_none() {
                continue;
            }
            let Some(pending) = joined.pending.borrow_mut().take() else {
                continue;
            };
            grafted |= self.graft(&mut next, host, &pending);
        }
        grafted.then_some(next)
    }

    fn hosted(&self, host: TabId) -> Option<DockHosted> {
        let foreign = self.foreign.borrow();
        let inner = |outer: TabId| {
            foreign
                .inner(outer)
                .filter(|(held, _)| *held == host)
                .map(|(_, inner)| inner)
        };
        let above = self.above.get()?;
        self.state.with(|state| {
            let group = state.find_group(&joined_key(host))?;
            let prefix = joined_prefix(host);
            let mut subtree = state.subtree(
                group,
                &DockMapping {
                    tab: &inner,
                    key: &|key| key.strip_prefix(prefix.as_str()).map(str::to_owned),
                    group: &|key| joined_host(key).and_then(inner),
                },
            );
            subtree.set_outside(state.outside_of(group, &inner));
            let list = |tabs: &[TabId]| tabs.iter().copied().filter_map(inner).collect();
            Some(DockHosted {
                state: subtree,
                tabs: list(&above.tabs),
                focused: above.focused.and_then(inner),
                shown: above.shown.and_then(inner),
                visible: list(&above.visible),
                fullscreen: above.fullscreen.and_then(inner),
                mode: above.mode,
                covers: above
                    .covers
                    .iter()
                    .filter_map(|(tab, rect)| Some((inner(*tab)?, *rect)))
                    .collect(),
            })
        })
    }
}

/// Provided in each tab body: the first `Docking` built in it joins the outer dock as a pinned group
/// keyed `joined:<tab>`. A `DockHost` hides any `DockJoin` around it, so a dock under it stands
/// alone.
#[derive(Clone)]
pub struct DockJoin {
    pub(crate) host: TabId,
    pub(crate) registry: Option<Rc<Registry>>,
    pub(crate) claimed: Rc<Cell<bool>>,
}

impl DockJoin {
    pub(crate) fn new(host: TabId, registry: Rc<Registry>) -> Self {
        Self {
            host,
            registry: Some(registry),
            claimed: Rc::default(),
        }
    }

    pub fn none() -> Self {
        Self {
            host: TabId::new(0),
            registry: None,
            claimed: Rc::default(),
        }
    }

    pub fn host(&self) -> TabId {
        self.host
    }

    pub fn claim(&self) -> Option<Rc<dyn DockHostLink>> {
        let registry = self.registry.clone()?;
        if self.claimed.replace(true) {
            return None;
        }
        let link = Rc::new(JoinedLink {
            host: self.host,
            registry,
            claimed: Rc::clone(&self.claimed),
            joined: Rc::new(Joined::default()),
        });
        let held = Rc::clone(&link);
        on_cleanup(move || held.release());
        Some(link)
    }
}

pub fn use_dock_join() -> Option<DockJoin> {
    use_context::<DockJoin>().filter(|join| join.registry.is_some())
}

struct JoinedLink {
    host: TabId,
    registry: Rc<Registry>,
    claimed: Rc<Cell<bool>>,
    joined: Rc<Joined>,
}

impl JoinedLink {
    fn outer(&self, inner: TabId) -> Option<TabId> {
        self.registry.foreign.borrow().outer(self.host, inner)
    }

    fn forwarded(&self, request: DockRequest) -> Option<DockRequest> {
        Some(match request {
            DockRequest::Show(tab) => DockRequest::Show(self.outer(tab)?),
            DockRequest::Fullscreen(Some((tab, area))) => {
                DockRequest::Fullscreen(Some((self.outer(tab)?, area)))
            }
            DockRequest::Fullscreen(None) => {
                let ours = self.registry.above.with_untracked(|above| {
                    let fullscreen = above.as_ref()?.fullscreen?;
                    self.registry.foreign.borrow().inner(fullscreen)
                });
                if ours.map(|(host, _)| host) != Some(self.host) {
                    return None;
                }
                DockRequest::Fullscreen(None)
            }
            DockRequest::PlaceBar(bar, placement) => {
                DockRequest::PlaceBar(self.outer(bar)?, placement)
            }
            DockRequest::MoveBar(bar, moved) => DockRequest::MoveBar(self.outer(bar)?, moved),
            DockRequest::HideBar(bar, hidden) => DockRequest::HideBar(self.outer(bar)?, hidden),
            DockRequest::Restore(from) => {
                let registry = &self.registry;
                let mut state = registry.state.get_untracked();
                if !registry.graft(&mut state, self.host, &from) {
                    *self.joined.pending.borrow_mut() = Some(from);
                    registry.bump();
                    return None;
                }
                DockRequest::Restore(state)
            }
        })
    }

    fn release(&self) {
        self.withdraw();
        *self.joined.declaration.borrow_mut() = None;
        *self.joined.events.borrow_mut() = None;
        *self.joined.bodies.borrow_mut() = None;
        self.claimed.set(false);
    }
}

impl DockHostLink for JoinedLink {
    fn declare(&self, declaration: DockDeclaration) {
        *self.joined.declaration.borrow_mut() = Some(declaration);
        self.registry
            .joins
            .borrow_mut()
            .entry(self.host)
            .or_insert_with(|| Rc::clone(&self.joined));
        self.registry.bump();
    }

    fn hosted(&self) -> Memo<Option<DockHosted>> {
        let registry = Rc::clone(&self.registry);
        let host = self.host;
        let version = registry.version;
        create_memo(move || {
            if !version.is_alive() || !registry.state.is_alive() || !registry.above.is_alive() {
                return None;
            }
            version.get();
            registry.hosted(host)
        })
    }

    fn request(&self, request: DockRequest) {
        let above = self.registry.host.borrow().clone();
        if let Some(above) = above
            && let Some(request) = self.forwarded(request)
        {
            above.request(request);
        }
    }

    fn listen(&self, events: Callback<DockHostEvent>) {
        *self.joined.events.borrow_mut() = Some(events);
    }

    fn place(&self, bodies: DockBodies) {
        *self.joined.bodies.borrow_mut() = Some(bodies);
    }

    fn withdraw(&self) {
        {
            let mut joins = self.registry.joins.borrow_mut();
            if joins
                .get(&self.host)
                .is_some_and(|held| Rc::ptr_eq(held, &self.joined))
            {
                joins.remove(&self.host);
            }
        }
        self.registry.bump();
    }
}
