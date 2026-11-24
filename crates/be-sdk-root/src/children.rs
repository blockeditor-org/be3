//! The children the editors of an app a root embeds place (guides/apps/children.md), as the root
//! shows them (plans/host_as_platform_adapter.md, 4g7b3): each block child placed to run its
//! editor is that block's editor, which the root's host opens and serves
//! (`Embedded::spawn_block`), in an app of its own, asked for one screen the size of its place
//! and composited there: under its placer's layer, which leaves the hole it cut for it, or over
//! it for a child placed `ChildLayer::Above`. A child that takes input where it is placed
//! (active, live or in place) takes the presses there; a passive one leaves them to its placer.
//! The children those editors place are shown the same way, any number of levels deep.
//!
//! The host tells each placer how its children are as its own children's statuses
//! (`be3:root`'s `host.show-children`): available once an editor shows the child, with what its
//! app's manifest says of that editor, or why the root shows nothing of it.
//!
//! An editor's gifts of the keyboard to its children (`focus-child`, `focus-block-child`) reach
//! the root through `host.gifts` and are given once the child shows (`ask`, `give`), as
//! be-host-runtime's `gift` gives its own: counted from the child up, missed only for a child
//! that was placed and went. Each placer above a child the keyboard goes into hears so
//! (`focused_into`, `child-focused`). A press anywhere but in an active child, or Escape in its
//! placer's view, takes it back (`revoke`).
//!
//! Left for later (4g7b3's later parts): the keys held for a gift until its child is ready, a
//! gift given only while no popover or keyboard-taking layer of the editor's is open, the press
//! that promotes a child replayed in it, a child that takes its placer's frame, docks that join
//! across apps (a child is asked for its own dock), drags, a block shown in several places
//! sharing one editor, occluders, a child sized to its content (`fit`), one placed in place at its
//! zoom, previews, and every other kind of child (web views, host panels, program windows,
//! pickers, creations, artifact settings, rename forms and dock tab bodies), which the placer
//! hears are unavailable.

use std::{
    cell::RefCell,
    collections::VecDeque,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
    time::Duration,
};

use be_lib_geometry::{Rect, pos2};
use be_lib_layers::{Affine, Area, Layer, LayerId};
use be_lib_stable_hash::{HashMap, HashSet};
use be_sdk_api::{
    ChildContent, ChildId, ChildLayer, ChildMode, ChildPlacement, EditorInstanceId, FrameChrome,
    FrameSpec, Fonts, GIFT_COMMITS, GIFT_DEPARTED_KEPT, GIFT_WAIT, ParentView, Pos2 as ViewPos2, ScreenId, ScreenRequest, SurfaceSpec,
    Vec2 as ViewVec2, ViewId, ViewPlacement, ViewportMetrics,
};
use be_wasm_root_api::{Gift, GiftTo, OpenedIn, PlacerEvent, ShownBlock, ShownChild, guest::host};

use crate::{BlockSpawn, Embedded, Embedding, Hosted, Pressed, SpawnFailure};

/// The one screen a child's app is asked for: its editor's main view, the size of its place.
pub const CHILD_SCREEN: ScreenId = ScreenId(1);

/// Said of a child whose block is open in an editor that places it, which would show itself
/// without end: the words be-host says it in.
pub const ALREADY_OPEN_ABOVE: &str = "the block is already open above this editor";

/// Said of a child of a kind the root shows nothing of yet.
pub const NOT_SHOWN_YET: &str = "the root does not show this kind of child yet";

/// What a root spawns its children's apps with, and how it hears that one started.
#[derive(Clone)]
pub struct Spawning {
    /// The surface each app draws to.
    pub surface: SurfaceSpec,
    /// The fonts each app is told once it runs.
    pub fonts: Rc<Fonts>,
    /// Asks the root for a frame: a child's editor opened, or could not be.
    pub wake: Rc<dyn Fn()>,
}

/// The children one embedded app's editors placed, as the root shows them.
#[derive(Default)]
pub struct Children {
    /// The handle of the app whose editors place these, as of the last `settle`.
    instance: u64,
    /// The layers showing that app's screens, with the editor and view each shows.
    placers: Vec<(LayerId, EditorInstanceId, ViewId)>,
    /// How many layout commits each of its editors made that carried its children or a frame
    /// report (`Embedded::commits`), as of the last `settle`.
    commits: HashMap<EditorInstanceId, u32>,
    shown: Vec<Child>,
    /// Every child its editors place now, of any kind, shown or not.
    placed: HashSet<(EditorInstanceId, ChildId)>,
    /// The latest children their changes took away (at most `GIFT_DEPARTED_KEPT`): a gift to
    /// one is missed.
    departed: VecDeque<(EditorInstanceId, ChildId)>,
    /// What the host was told last of each view's children (`host.show-children`).
    told: HashMap<(EditorInstanceId, ViewId), Vec<ShownChild>>,
    /// The children placed active that were taken back (`revoke`): they are passive until placed
    /// other than active.
    revoked: HashSet<(EditorInstanceId, ViewId, ChildId)>,
    /// The keyboard each editor asked be given to one of its children, the latest of each, until
    /// it is given or ends (`give`).
    gifts: Vec<Pending>,
}

/// A gift an editor asked for, waiting for its child to show.
#[derive(Clone, Debug, PartialEq)]
struct Pending {
    editor: EditorInstanceId,
    to: GiftTo,
    /// Once its child is up: the frame clock's time it came up, and its editor's commits then.
    /// The gift waits `GIFT_WAIT` from there, or `GIFT_COMMITS` more commits.
    up: Option<(Duration, u32)>,
}

/// A block child placed to run its editor.
struct Child {
    editor: EditorInstanceId,
    view: ViewId,
    child: ChildId,
    /// Its block, with the editor and view block its placement names.
    shown: ShownBlock,
    block: [u8; 16],
    /// Its placement, as its placer's latest layout said.
    placement: ChildPlacement,
    /// The pixels per point its placer's screen is laid out at, which its own is.
    raster: f32,
    showing: Rc<RefCell<Showing>>,
    /// The layer showing its screen, once its editor opened.
    layer: Option<LayerId>,
    /// Whether it is up for a gift: its app runs, embedded.
    up: bool,
    /// The children its own editor places.
    nested: Children,
}

/// How far a child's editor got.
enum Showing {
    /// The host has yet to name and open its editor.
    Opening,
    Shown {
        hosted: Hosted,
        /// Whether its app is embedded, so input routed to its screen reaches it.
        embedded: bool,
    },
    /// Why it shows nothing: its app could not start or stopped, or its block is open above it.
    Failed(String),
}

impl Child {
    /// The child `placement` places in `editor`'s view `view`, laid out at `raster` pixels per
    /// point, when it is a block child placed to run its editor: its editor not opened yet.
    fn placed(
        editor: EditorInstanceId,
        view: ViewId,
        raster: f32,
        placement: ChildPlacement,
    ) -> Option<Self> {
        let ChildContent::Block {
            block_id,
            view_block,
            editor: named,
            ..
        } = placement.content
        else {
            return None;
        };
        if !runs_editor(placement.mode) {
            return None;
        }
        Some(Self {
            editor,
            view,
            child: placement.child,
            shown: ShownBlock {
                block: block_id,
                editor: named,
                view_block,
            },
            block: block_id,
            placement,
            raster,
            showing: Rc::new(RefCell::new(Showing::Opening)),
            layer: None,
            up: false,
            nested: Children::default(),
        })
    }

    /// What names the editor it keeps while placed: its placer's view, its id there, and what
    /// it shows. A child placed showing another block, or with another editor or view block
    /// named, is another editor.
    fn key(&self) -> ChildKey {
        (
            self.editor,
            self.view,
            self.child,
            self.shown.block,
            self.shown.editor,
            self.shown.view_block,
        )
    }

    /// Whether `layer` shows it or one of its own children, any number of levels deep.
    fn holds(&self, layer: LayerId) -> bool {
        self.layer == Some(layer) || self.nested.holds(layer)
    }

    /// Whether it takes the input where it is placed: placed to, and not taken back (`revoked`).
    fn takes_input(&self, revoked: &HashSet<(EditorInstanceId, ViewId, ChildId)>) -> bool {
        takes_input(self.placement.mode) && !revoked.contains(&(self.editor, self.view, self.child))
    }
}

type ChildKey = (
    EditorInstanceId,
    ViewId,
    ChildId,
    [u8; 16],
    Option<[u8; 16]>,
    Option<[u8; 16]>,
);

/// What a `settle` makes of the children shown and those placed now (`reconcile`).
struct Reconciled {
    /// The children placed now, in their order, each keeping the editor it had if it showed
    /// the same before.
    kept: Vec<Child>,
    /// Those of `kept` (by index) whose editor is to be opened.
    opened: Vec<usize>,
    /// Those shown before and placed no longer, to let go of.
    gone: Vec<Child>,
}

/// Matches the children `wanted` now with those `shown` before by what names their editor
/// (`Child::key`): one that matches keeps its editor and its own children at its new place, one
/// that does not is opened, unless its block is open `above` it, which it shows instead, and the
/// rest go.
fn reconcile(shown: Vec<Child>, wanted: Vec<Child>, above: &[[u8; 16]]) -> Reconciled {
    let mut before: HashMap<ChildKey, Child> =
        shown.into_iter().map(|child| (child.key(), child)).collect();
    let mut kept = Vec::with_capacity(wanted.len());
    let mut opened = Vec::new();
    for mut child in wanted {
        match before.remove(&child.key()) {
            Some(shown) => {
                child.showing = shown.showing;
                child.layer = shown.layer;
                child.up = shown.up;
                child.nested = shown.nested;
            }
            None if above.contains(&child.block) => {
                child.showing = Rc::new(RefCell::new(Showing::Failed(
                    ALREADY_OPEN_ABOVE.to_owned(),
                )));
            }
            None => opened.push(kept.len()),
        }
        kept.push(child);
    }
    Reconciled {
        kept,
        opened,
        gone: before.into_values().collect(),
    }
}

impl Children {
    /// Follows what `parent`'s editors placed now, at the frame clock's `now`: spawns the editor
    /// of each block child newly placed to run one, lets go of those no longer placed, asks each
    /// shown for its screen at its place, and tells the host how each view's children are once
    /// that changed. `above` are the blocks open in `parent` and the editors that place it,
    /// which none of its children may show again. The children of the children shown are
    /// followed the same way.
    pub fn settle(
        &mut self,
        parent: &Embedded,
        above: &[[u8; 16]],
        embedding: &mut Embedding,
        spawning: &Spawning,
        now: Duration,
    ) {
        let screens = parent.screens();
        self.instance = parent.instance();
        self.placers = screens
            .iter()
            .map(|screen| (parent.layer_id(screen.screen), screen.instance, screen.view))
            .collect();
        self.commits = screens
            .iter()
            .map(|screen| (screen.instance, parent.commits(screen.instance)))
            .collect();
        let mut wanted = Vec::new();
        let mut placed = HashSet::default();
        let mut views: HashMap<(EditorInstanceId, ViewId), Vec<ShownChild>> = HashMap::default();
        for placements in parent.placed() {
            let raster = screens
                .iter()
                .find(|screen| {
                    (screen.instance, screen.view) == (placements.instance, placements.view)
                })
                .map_or(1.0, |screen| screen.metrics.raster);
            let told = views
                .entry((placements.instance, placements.view))
                .or_default();
            for placement in placements.children {
                let child = placement.child;
                placed.insert((placements.instance, child));
                match Child::placed(placements.instance, placements.view, raster, placement) {
                    Some(child) => wanted.push(child),
                    None => told.push(ShownChild {
                        child: child.0,
                        shown_in: None,
                        active: false,
                        error: Some(NOT_SHOWN_YET.to_owned()),
                    }),
                }
            }
        }
        self.depart(placed);
        let Reconciled { kept, opened, gone } =
            reconcile(std::mem::take(&mut self.shown), wanted, above);
        for child in &opened {
            spawn(&kept[*child], spawning, now);
        }
        for mut gone in gone {
            release(&mut gone, embedding);
        }
        self.shown = kept;
        self.forget_revoked();
        let mut nested = above.to_vec();
        let revoked = &self.revoked;
        for child in &mut self.shown {
            let failed = {
                let showing = child.showing.borrow();
                match &*showing {
                    Showing::Shown { hosted, .. } => hosted.app.failure(),
                    Showing::Opening | Showing::Failed(_) => None,
                }
            };
            if let Some(why) = failed {
                release(child, embedding);
                *child.showing.borrow_mut() = Showing::Failed(why);
            }
            let app = match &mut *child.showing.borrow_mut() {
                Showing::Shown { hosted, embedded } => {
                    if !*embedded {
                        embedding.embed(hosted.app.clone());
                        *embedded = true;
                    }
                    hosted.app.show(vec![child_screen(
                        &child.placement,
                        hosted.editor,
                        child.raster,
                    )]);
                    Some((hosted.app.clone(), hosted.editor))
                }
                Showing::Opening | Showing::Failed(_) => None,
            };
            child.layer = app.as_ref().map(|(app, _)| app.layer_id(CHILD_SCREEN));
            child.up = app.as_ref().is_some_and(|(app, _)| app.started());
            views
                .entry((child.editor, child.view))
                .or_default()
                .push(ShownChild {
                    child: child.child.0,
                    shown_in: app.as_ref().map(|(app, editor)| OpenedIn {
                        instance: app.instance(),
                        editor: editor.0,
                    }),
                    active: child.takes_input(revoked),
                    error: match &*child.showing.borrow() {
                        Showing::Failed(why) => Some(why.clone()),
                        Showing::Opening | Showing::Shown { .. } => None,
                    },
                });
            if let Some((app, _)) = app {
                nested.push(child.block);
                child.nested.settle(&app, &nested, embedding, spawning, now);
                nested.pop();
            }
        }
        self.tell(views);
    }

    /// Forgets that a child was taken back once it is no longer placed active: a child taken
    /// back stays passive only while it is.
    fn forget_revoked(&mut self) {
        let shown = &self.shown;
        self.revoked.retain(|(editor, view, id)| {
            shown.iter().any(|child| {
                (child.editor, child.view, child.child) == (*editor, *view, *id)
                    && child.placement.mode == ChildMode::Active
            })
        });
    }

    /// Takes `placed`, every child placed now, in place of those placed before: those no longer
    /// placed have departed, and one placed again has not.
    fn depart(&mut self, placed: HashSet<(EditorInstanceId, ChildId)>) {
        let before = std::mem::replace(&mut self.placed, placed);
        self.departed
            .retain(|child| !self.placed.contains(child));
        for child in before {
            if !self.placed.contains(&child) && !self.departed.contains(&child) {
                self.departed.push_back(child);
            }
        }
        while self.departed.len() > GIFT_DEPARTED_KEPT {
            self.departed.pop_front();
        }
    }

    /// Tells the host how each view's children are where that changed since it was told last,
    /// and that a view it was told of placed none once it places none.
    fn tell(&mut self, mut views: HashMap<(EditorInstanceId, ViewId), Vec<ShownChild>>) {
        for view in self.told.keys() {
            views.entry(*view).or_default();
        }
        for ((editor, view), children) in views {
            if self.told.get(&(editor, view)) == Some(&children) {
                continue;
            }
            let told = host::show_children(self.instance, editor.0, view.0, children.clone());
            be_wasm_ops::guest::spawn(async move {
                // A host that stopped answering stops the root too.
                let _ = told.await;
            });
            match children.is_empty() {
                true => self.told.remove(&(editor, view)),
                false => self.told.insert((editor, view), children),
            };
        }
    }

    /// Takes back active children, as be-host does: a pointer's press anywhere but in the child
    /// (in its placer's view or outside it, on the root's own surfaces too), or Escape in its
    /// placer's view. Each leaves the input to its placer, which hears it is no longer active
    /// with the next `settle`. Their own children's likewise.
    pub fn revoke(&mut self, pressed: &[Pressed]) {
        for child in &self.shown {
            if child.placement.mode != ChildMode::Active || !child.takes_input(&self.revoked) {
                continue;
            }
            let placer = self
                .placers
                .iter()
                .find(|(_, editor, view)| (*editor, *view) == (child.editor, child.view))
                .map(|(layer, ..)| *layer);
            let taken = pressed.iter().any(|press| match press.escape {
                true => Some(press.layer) == placer,
                false => !child.holds(press.layer),
            });
            if taken {
                self.revoked
                    .insert((child.editor, child.view, child.child));
            }
        }
        for child in &mut self.shown {
            child.nested.revoke(pressed);
        }
    }

    /// Takes the gifts asked by the editors of the app these are the children of, each in place
    /// of the one its editor asked before, but one asked again for the same child, which keeps
    /// waiting as it was; those of their own children's apps go to them.
    pub fn ask(&mut self, gifts: &mut Vec<Gift>) {
        let instance = self.instance;
        let mut kept = Vec::new();
        for gift in gifts.drain(..) {
            if gift.instance != instance {
                kept.push(gift);
                continue;
            }
            let editor = EditorInstanceId(gift.editor);
            if self
                .gifts
                .iter()
                .any(|pending| pending.editor == editor && pending.to == gift.to)
            {
                continue;
            }
            self.gifts.retain(|pending| pending.editor != editor);
            self.gifts.push(Pending {
                editor,
                to: gift.to,
                up: None,
            });
        }
        *gifts = kept;
        for child in &mut self.shown {
            if gifts.is_empty() {
                return;
            }
            child.nested.ask(gifts);
        }
    }

    /// Gives the keyboard where a gift asked, once its child is up and shows in a layer drawn
    /// (`drawn`): the layer to focus, the first gift's that can be given. A gift ends once
    /// given; when the keyboard (`focused`) is not within its editor's views and their children;
    /// when its child could not be shown, is placed as a kind the root shows nothing of, or
    /// departed (which its editor hears, `told`, as `gift-missed`); or when, once its child was
    /// up, `GIFT_WAIT` passed on the frame clock (`now`) or its editor committed `GIFT_COMMITS`
    /// layouts without the child showing. Until its child is up it waits, however long its app
    /// takes to start.
    pub fn give(
        &mut self,
        focused: Option<LayerId>,
        drawn: &dyn Fn(LayerId) -> bool,
        now: Duration,
        told: &mut Vec<(u64, u64, PlacerEvent)>,
    ) -> Option<LayerId> {
        let mut given = None;
        for mut pending in std::mem::take(&mut self.gifts) {
            if !focused.is_some_and(|focused| self.within(pending.editor, focused)) {
                continue;
            }
            let child = self.shown.iter().find(|child| {
                child.editor == pending.editor
                    && match pending.to {
                        GiftTo::Child(id) => child.child.0 == id,
                        GiftTo::Block(block) => child.block == block,
                    }
            });
            let Some(child) = child else {
                match pending.to {
                    GiftTo::Child(id)
                        if self.departed.contains(&(pending.editor, ChildId(id))) =>
                    {
                        told.push((
                            self.instance,
                            pending.editor.0,
                            PlacerEvent::GiftMissed(id),
                        ));
                    }
                    // Placed, but of a kind that shows no editor.
                    GiftTo::Child(id) if self.placed.contains(&(pending.editor, ChildId(id))) => {}
                    // Not placed yet: the layout that places it may come after the ask.
                    GiftTo::Child(_) | GiftTo::Block(_) => self.gifts.push(pending),
                }
                continue;
            };
            if matches!(&*child.showing.borrow(), Showing::Failed(_)) {
                continue;
            }
            if !child.up {
                pending.up = None;
                self.gifts.push(pending);
                continue;
            }
            let commits = self.commits.get(&pending.editor).copied().unwrap_or(0);
            let (since, at) = *pending.up.get_or_insert((now, commits));
            match child.layer.filter(|layer| drawn(*layer)) {
                Some(layer) if given.is_none() => given = Some(layer),
                _ if now.saturating_sub(since) >= GIFT_WAIT
                    || commits.wrapping_sub(at) >= GIFT_COMMITS => {}
                _ => self.gifts.push(pending),
            }
        }
        for child in &mut self.shown {
            let inner = child.nested.give(focused, drawn, now, told);
            given = given.or(inner);
        }
        given
    }

    /// Whether the layer `layer` shows one of `editor`'s views or a child it placed, any number
    /// of levels deep.
    fn within(&self, editor: EditorInstanceId, layer: LayerId) -> bool {
        self.placers
            .iter()
            .any(|(placer, shows, _)| *shows == editor && *placer == layer)
            || self
                .shown
                .iter()
                .any(|child| child.editor == editor && child.holds(layer))
    }

    /// Whether `layer` shows a view of the app these are the children of, or of a child of it.
    fn holds(&self, layer: LayerId) -> bool {
        self.placers.iter().any(|(placer, ..)| *placer == layer)
            || self.shown.iter().any(|child| child.holds(layer))
    }

    /// The children the keyboard went into when it went to the layer `layer`: for each placer
    /// from the innermost out, the handle of its app, the editor and the child of it that holds
    /// the layer. Empty for a layer that shows none of these children.
    pub fn focused_into(&self, layer: LayerId) -> Vec<(u64, EditorInstanceId, ChildId)> {
        for child in &self.shown {
            let mut into = match child.layer == Some(layer) {
                true => Vec::new(),
                false => child.nested.focused_into(layer),
            };
            if child.layer == Some(layer) || !into.is_empty() {
                into.push((self.instance, child.editor, child.child));
                return into;
            }
        }
        Vec::new()
    }

    /// Whether a child's editor is still being opened, any number of levels deep: its host has
    /// yet to open it, or the root to embed its app, so the root owes its host the frame that
    /// shows it (a host waiting for its root to settle waits for it). Once its app runs, the
    /// app's own pacing says what it owes.
    pub fn opening(&self) -> bool {
        self.shown.iter().any(|child| {
            matches!(
                &*child.showing.borrow(),
                Showing::Opening
                    | Showing::Shown {
                        embedded: false,
                        ..
                    }
            ) || child.nested.opening()
        })
    }

    /// Lets go of every child, as when their placer goes: each child's app stops once nothing
    /// else holds it.
    pub fn release(&mut self, embedding: &mut Embedding) {
        for mut child in std::mem::take(&mut self.shown) {
            release(&mut child, embedding);
        }
        self.told.clear();
        self.gifts.clear();
    }

    /// The apps of every child shown, any number of levels deep, each with the id of the layer
    /// showing its screen once it painted it: what the root's paintings carry.
    pub fn screens(&self) -> Vec<(Embedded, LayerId)> {
        let mut screens = Vec::new();
        for child in &self.shown {
            if let Showing::Shown { hosted, .. } = &*child.showing.borrow()
                && let Some(layer) = hosted.app.layer(CHILD_SCREEN, pos2(0.0, 0.0))
            {
                screens.push((hosted.app.clone(), layer.id));
            }
            screens.extend(child.nested.screens());
        }
        screens
    }

    /// Asks every child's app for a frame for the frame clock's `now` (`Embedded::frame`), any
    /// number of levels deep, and answers once each presented or needed none.
    pub fn frame(&self, now: Duration) -> impl Future<Output = ()> + use<> {
        let frames: Vec<Pin<Box<dyn Future<Output = ()>>>> = self
            .apps()
            .into_iter()
            .map(|app| -> Pin<Box<dyn Future<Output = ()>>> {
                Box::pin(async move { app.frame(now).await })
            })
            .collect();
        All(frames)
    }

    fn apps(&self) -> Vec<Embedded> {
        let mut apps = Vec::new();
        for child in &self.shown {
            if let Showing::Shown { hosted, .. } = &*child.showing.borrow() {
                apps.push(hosted.app.clone());
            }
            apps.extend(child.nested.apps());
        }
        apps
    }

    /// The layers showing `screen` of the app these are the children of, with its children:
    /// those placed below it, then `layer`, the placer's own, hit everywhere but where a child
    /// that takes input shows, then those placed above it. Each child is clipped to what its
    /// placement shows of it within what shows of its placer, and shows its own children the
    /// same way.
    pub fn layers(&self, screen: &ScreenRequest, mut layer: Layer, layers: &mut Vec<Layer>) {
        // From the placer's view's points to the window's: through its surface's pixels.
        let pixels = 1.0 / layer.transform.across().length().max(f32::EPSILON);
        let to_window = Affine::scaling(pixels).then(&layer.transform);
        let shown = layer.transform.bounds(layer.clip);
        let mut below = Vec::new();
        let mut over = Vec::new();
        let mut holes = Vec::new();
        for child in &self.shown {
            if (child.editor, child.view) != (screen.instance, screen.view) {
                continue;
            }
            let Showing::Shown { hosted, .. } = &*child.showing.borrow() else {
                continue;
            };
            let placement = &child.placement;
            let at = to_window.bounds(placement.rect);
            let clip = to_window
                .bounds(placement.clip.intersect(placement.rect))
                .intersect(shown);
            let Some(mut shown_child) = hosted.app.layer(CHILD_SCREEN, at.min) else {
                continue;
            };
            let Some(from_window) = shown_child.transform.inverse() else {
                continue;
            };
            shown_child.clip = shown_child.clip.intersect(from_window.bounds(clip));
            shown_child.input.placer = Some(layer.id);
            let active = child.takes_input(&self.revoked);
            if !active {
                shown_child.input.hit = Area::Nowhere;
            }
            if active && placement.layer == ChildLayer::Below {
                holes.push(hole(placement, pixels, layer.clip));
            }
            let child_screen = child_screen(placement, hosted.editor, child.raster);
            let into = match placement.layer {
                ChildLayer::Below => &mut below,
                ChildLayer::Above => &mut over,
            };
            child.nested.layers(&child_screen, shown_child, into);
        }
        layer.input.hit = placer_hit(layer.size, holes);
        layers.extend(below);
        layers.push(layer);
        layers.extend(over);
    }
}

/// The part of its placer's layer, in its pixels (`pixels` a point), that a child placed as
/// `placement` takes the presses of: what shows of it, within its clip and the placer's
/// `clip`. Presses on the rest, where the child is clipped away, stay its placer's.
fn hole(placement: &ChildPlacement, pixels: f32, clip: Rect) -> Rect {
    let shows = placement.rect.intersect(placement.clip);
    Rect::from_min_max(
        pos2(shows.min.x * pixels, shows.min.y * pixels),
        pos2(shows.max.x * pixels, shows.max.y * pixels),
    )
    .intersect(clip)
}

/// Where a placer's layer `size` pixels big is hit, with `holes` (in its pixels) where children
/// that take input are: everywhere else.
fn placer_hit(size: (u32, u32), holes: Vec<Rect>) -> Area {
    if holes.is_empty() {
        return Area::Whole;
    }
    let whole = Rect::from_min_max(pos2(0.0, 0.0), pos2(size.0 as f32, size.1 as f32));
    Area::Region(whole.subtract(&holes))
}

/// Whether a child placed in `mode` runs its block's editor: not one previewed, nor one kept
/// placed hidden.
fn runs_editor(mode: ChildMode) -> bool {
    matches!(
        mode,
        ChildMode::Passive | ChildMode::Active | ChildMode::Live | ChildMode::InPlace
    )
}

/// Whether a child placed in `mode` takes the input where it is placed rather than leaving it to
/// its placer.
fn takes_input(mode: ChildMode) -> bool {
    matches!(
        mode,
        ChildMode::Active | ChildMode::Live | ChildMode::InPlace
    )
}

/// The screen a child placed as `placement` asks of its editor `editor`: its main view the size
/// of its place, at its placer's `raster`, in a frame of its own when it was placed with one.
pub fn child_screen(placement: &ChildPlacement, editor: EditorInstanceId, raster: f32) -> ScreenRequest {
    let size = placement.rect.size();
    let origin = placement.rect.min.to_vec2();
    ScreenRequest {
        screen: CHILD_SCREEN,
        instance: editor,
        view: ViewId::MAIN,
        placement: ViewPlacement::Parent(ParentView::Main),
        frame: placement.own_frame.then(|| FrameSpec {
            chrome: FrameChrome::Drawn,
            top_bar: placement.top_bar,
            dialog: placement.dialog.clone(),
            anchor: placement.anchor.map(|anchor| anchor.translate(-origin)),
            ..FrameSpec::default()
        }),
        metrics: ViewportMetrics {
            logical: ViewVec2 {
                x: size.x,
                y: size.y,
            },
            visible: ViewPos2::ZERO,
            pixel_width: (size.x * raster).ceil().max(1.0) as u32,
            pixel_height: (size.y * raster).ceil().max(1.0) as u32,
            scale_factor: raster,
            raster,
        },
        windows: false,
        host_place: None,
        docked: false,
        anchor_space: None,
        fit: false,
        origin: None,
    }
}

/// Spawns the editor of `child`'s block, at the frame clock's `now`, which shows it once the
/// host opened it and its app runs.
fn spawn(child: &Child, spawning: &Spawning, now: Duration) {
    let showing = Rc::downgrade(&child.showing);
    let placement = child.placement.clone();
    let raster = child.raster;
    let spawned = Embedded::spawn_block(
        BlockSpawn {
            shown: child.shown.clone(),
            screens: Box::new(move |editor| vec![child_screen(&placement, editor, raster)]),
            surface: spawning.surface.clone(),
            fonts: Rc::clone(&spawning.fonts),
        },
        now,
    );
    let wake = Rc::clone(&spawning.wake);
    be_wasm_ops::guest::spawn(async move {
        let spawned = spawned.await;
        // A child let go of meanwhile: its app goes with this, which stops it.
        let Some(showing) = showing.upgrade() else {
            return;
        };
        *showing.borrow_mut() = match spawned {
            Ok(hosted) => Showing::Shown {
                hosted,
                embedded: false,
            },
            Err(SpawnFailure { name, why }) => Showing::Failed(match name {
                Some(name) => format!("{name} could not start: {why}"),
                None => why,
            }),
        };
        wake();
    });
}

/// Lets go of `child`'s app and of those of its own children, as embedded.
fn release(child: &mut Child, embedding: &mut Embedding) {
    if let Showing::Shown { hosted, .. } = &*child.showing.borrow() {
        embedding.remove(hosted.app.instance());
    }
    child.nested.release(embedding);
}

/// Every one of a list of futures, polled together until each finished.
struct All(Vec<Pin<Box<dyn Future<Output = ()>>>>);

impl Future for All {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        self.0
            .retain_mut(|future| future.as_mut().poll(context).is_pending());
        match self.0.is_empty() {
            true => Poll::Ready(()),
            false => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests;
