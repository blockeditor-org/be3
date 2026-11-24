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
//! An editor asks that a child take the keyboard in the child's placement (`keyboard-ask`), and
//! the root moves it at the settle that first carries the ask (`keyboard`), as be-host does,
//! never waiting on the child's app: keys are never held (decisions/keys_are_never_held.md).
//! Each placer above a child the keyboard goes into hears so (`focused_into`, `child-focused`).
//! A press anywhere but in an active child, or Escape in its placer's view, takes it back
//! (`revoke`).
//!
//! An active or live child placed without a frame of its own in a view that has one takes that
//! frame over, as under be-host (guides/apps/children.md, A child that takes the frame): it is
//! asked for a screen the size of its placer's, with its placer's frame and its placed rect as the
//! frame's `content`, and shown over it, while its placer is asked for its view with no chrome. A
//! press on the rest of the frame, or on its content outside its placed rect, reaches the placer
//! and takes the frame back, as Escape in the placer does.
//!
//! Docks join across apps (guides/apps/children.md, Docks across apps) through one
//! be-sdk-app-host `Docks` for every editor shown: a child placed in a dock tab's body is asked
//! for its frame `docked`, what each app's docks say is heard once its placements are, and what
//! the docks have for each editor is handed to the root's host (`be3:root`'s `host.dock`), which
//! answers the editor's dock watch with it. A docked child stays open while the dock that hosts
//! it keeps it, placed or not. The bodies of a joined dock's tabs and bars, which the outermost
//! dock places as `ChildContent::DockTab`, are shown as the views of the editor that owns them,
//! each another screen of its app (`ViewPlacement::Dock`), with a frame where it holds the
//! editor's content.
//!
//! A child placed as a preview runs its editor too, asked for its preview view
//! (`PREVIEW_SCREEN`, `preview_screen`) and shown under its placer, which leaves a hole for it
//! once the host says it is available (`ChildPreview::Hosted`), as a passive child is, hit
//! nowhere. One whose editor draws no preview runs no app: its placer draws its block's label.
//! Promoted (a canvas focusing its card), it keeps its editor, asked for its main view.
//!
//! Left for later (plans/host_as_platform_adapter.md, 4g7b3e to 4g7b3g): an ask acted on only
//! while no popover or keyboard-taking layer of the editor's is open (the editor's own popovers
//! are in its screen, so the root does not see them), the press that promotes a child replayed in
//! it, a joined dock's tab menus, picks and details (the editors' menus reach the root's host as
//! shell messages it does not take yet), drags, a block shown in several places sharing one
//! editor (a preview included, which be-host draws once at the largest), occluders, a child
//! sized to its content (`fit`), one placed in place at its zoom, rotation and opacity, and every
//! other kind of child (web views, host panels, program windows, pickers, creations, artifact
//! settings and rename forms), which the placer hears are unavailable.

use std::{
    cell::RefCell,
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
    ChildContent, ChildId, ChildLayer, ChildMode, ChildPlacement, ChildPlacements, DockUpdate,
    EditorInstanceId, Fonts, FrameChrome, FrameSpec, ParentView, Pos2 as ViewPos2, ScreenId,
    ScreenRequest, SurfaceSpec, Vec2 as ViewVec2, ViewId, ViewPlacement, ViewportMetrics,
};
use be_sdk_app_host::docks::{DockedChild, Docks};
use be_wasm_root_api::{OpenedIn, ShownBlock, ShownChild, ShownPreview, guest::host};
use uuid::Uuid;

use crate::{BlockSpawn, Embedded, Embedding, Hosted, Pressed, SpawnFailure};

/// The screen a child's app is asked for its editor's main view: the size of its place, or of
/// its placer's frame while it takes that over.
pub const CHILD_SCREEN: ScreenId = ScreenId(1);

/// The screen a child placed as a preview asks its app for its editor's preview view, the size
/// of its place.
pub const PREVIEW_SCREEN: ScreenId = ScreenId(2);

/// The first of the screens a child's app is asked for the views of its dock's tabs and bars
/// that a joined dock places, each keeping its screen while placed.
const FIRST_TAB_SCREEN: u64 = 3;

/// The most pixels along its longer side a preview is painted at, as be-host's
/// `preview_scale` caps it.
const PREVIEW_PIXELS: f32 = 2048.0;

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
    /// The screens that app was asked for, which a child taking one's frame takes the size and
    /// frame of.
    requests: Vec<ScreenRequest>,
    shown: Vec<Child>,
    /// The bodies of joined docks' tabs and bars its editors place (`ChildContent::DockTab`),
    /// each by the path to the editor that owns it.
    bodies: Vec<TabAsk>,
    /// What the host was told last of each view's children (`host.show-children`).
    told: HashMap<(EditorInstanceId, ViewId), Vec<ShownChild>>,
    /// The children placed active that were taken back (`revoke`): they are passive until placed
    /// other than active.
    revoked: HashSet<(EditorInstanceId, ViewId, ChildId)>,
    /// The children that took their placer's frame and were taken back from it (`revoke`): they
    /// show in their place until placed other than active or live.
    frame_revoked: HashSet<(EditorInstanceId, ViewId, ChildId)>,
    /// The highest `keyboard-ask` serial of each editor's placements the root acted on.
    asked: HashMap<EditorInstanceId, u64>,
    /// Each editor's latest ask, by the child it names, until the child takes the keyboard or
    /// the ask ends (`keyboard`).
    asks: Vec<Ask>,
    /// The docks of every editor shown, the outermost `Children`' alone: the others hand theirs
    /// what their editors place.
    docks: Docks,
}

/// A body of a joined dock's tab or bar an editor placed (`ChildContent::DockTab`): the view
/// `tab` of the editor its path leads to.
#[derive(Clone, Debug)]
struct TabAsk {
    /// The editor whose child `via` starts at, and the view it is placed in.
    editor: EditorInstanceId,
    view: ViewId,
    /// The id of the body's placement.
    body: ChildId,
    /// The children leading from `editor` to the one that owns the tab, through each one's.
    via: Vec<ChildId>,
    tab: u64,
    placement: ChildPlacement,
    raster: f32,
}

/// The view of a joined dock's tab or bar a child's app is asked for.
struct TabScreen {
    tab: u64,
    screen: ScreenId,
    /// Asked by the settle just made.
    asked: bool,
}

/// What a `settle` hands the children of every level.
struct Settling<'a> {
    embedding: &'a mut Embedding,
    spawning: &'a Spawning,
    now: Duration,
    docks: &'a mut Docks,
}

/// An editor's ask that its child take the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ask {
    editor: EditorInstanceId,
    child: ChildId,
    /// Not acted on yet: carried by the settle just made.
    new: bool,
}

/// Where an ask moves the keyboard (`Children::keyboard`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyboardMove {
    /// Into the layer showing the child asked for.
    Into(LayerId),
    /// Out of its placer's own screen, for the child to take once it shows: keys typed until
    /// then reach nothing.
    Out,
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
    /// Whether its app runs, embedded: up to take the keyboard.
    up: bool,
    /// Whether its placer places it now: one placed no longer stays while its dock is joined to
    /// the dock that hosts it, which shows it, and nothing shows its main view.
    placed: bool,
    /// Whether it takes its placer's frame over.
    framing: bool,
    /// Its main view as asked last, which a docked child kept unplaced is asked for still.
    main: Option<ScreenRequest>,
    /// The views of its dock a joined dock places, each a screen of its app.
    tabs: Vec<TabScreen>,
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
    /// Placed as a preview, its editor draws none: its placer draws its block's label, and no
    /// app runs for it.
    Label,
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
            placed: true,
            framing: false,
            main: None,
            tabs: Vec::new(),
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
        self.shows(layer) || self.nested.holds(layer)
    }

    /// Whether `layer` shows one of its own views: its main view or a view of its dock.
    fn shows(&self, layer: LayerId) -> bool {
        self.layer == Some(layer) || self.tab_layers().any(|shown| shown == layer)
    }

    /// Whether it shows its screen: its editor opened and its app painted it, so the frame it
    /// takes over is drawn.
    fn shows_frame(&self) -> bool {
        self.placed
            && self.up
            && self
                .hosted()
                .is_some_and(|(app, _)| app.layer(CHILD_SCREEN, pos2(0.0, 0.0)).is_some())
    }

    /// The editor its app shows, once the host opened it.
    fn hosted(&self) -> Option<(Embedded, EditorInstanceId)> {
        match &*self.showing.borrow() {
            Showing::Shown { hosted, .. } => Some((hosted.app.clone(), hosted.editor)),
            Showing::Opening | Showing::Label | Showing::Failed(_) => None,
        }
    }

    /// Whether it is placed as a preview, which shows its editor's preview view.
    fn previewed(&self) -> bool {
        self.placement.mode == ChildMode::Preview
    }

    /// The screen its app is asked for where it is placed: its editor's preview view while placed
    /// as a preview, else its main view.
    fn screen(&self) -> ScreenId {
        match self.previewed() {
            true => PREVIEW_SCREEN,
            false => CHILD_SCREEN,
        }
    }

    /// How it shows as a preview, which the host words its status from: not placed as one, its
    /// editor's preview view, or its block's label when its editor draws none.
    fn shown_preview(&self) -> ShownPreview {
        if !self.previewed() {
            return ShownPreview::None;
        }
        match &*self.showing.borrow() {
            Showing::Label => ShownPreview::Label,
            Showing::Shown { hosted, .. } if !hosted.preview => ShownPreview::Label,
            Showing::Opening | Showing::Shown { .. } | Showing::Failed(_) => ShownPreview::View,
        }
    }

    /// The layers showing the views of its dock a joined dock places.
    fn tab_layers(&self) -> impl Iterator<Item = LayerId> + '_ {
        let app = self.hosted().map(|(app, _)| app);
        self.tabs
            .iter()
            .filter_map(move |tab| Some(app.as_ref()?.layer_id(tab.screen)))
    }

    /// The layer showing the view that holds its editor's content: its main view, or the view
    /// of its dock's tab its content moved to once its dock joined its placer's.
    fn content_layer(&self) -> Option<LayerId> {
        let tab = self.hosted().and_then(|(app, _)| {
            self.tabs
                .iter()
                .find(|tab| app.content(tab.screen).is_some())
                .map(|tab| app.layer_id(tab.screen))
        });
        tab.or(self.layer.filter(|_| self.placed))
    }

    /// The screen its app is asked for the view `tab` of its dock, kept while placed.
    fn tab_screen(&mut self, tab: u64) -> ScreenId {
        if let Some(held) = self.tabs.iter_mut().find(|held| held.tab == tab) {
            held.asked = true;
            return held.screen;
        }
        let next = self
            .tabs
            .iter()
            .map(|held| held.screen.0 + 1)
            .max()
            .unwrap_or(FIRST_TAB_SCREEN);
        let screen = ScreenId(next);
        self.tabs.push(TabScreen {
            tab,
            screen,
            asked: true,
        });
        screen
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
    let mut before: HashMap<ChildKey, Child> = shown
        .into_iter()
        .map(|child| (child.key(), child))
        .collect();
    let mut kept = Vec::with_capacity(wanted.len());
    let mut opened = Vec::new();
    for mut child in wanted {
        match before.remove(&child.key()) {
            // One shown as its label, placed now to run its editor, is opened.
            Some(shown)
                if matches!(&*shown.showing.borrow(), Showing::Label) && !child.previewed() =>
            {
                opened.push(kept.len());
            }
            Some(shown) => {
                child.showing = shown.showing;
                child.layer = shown.layer;
                child.up = shown.up;
                child.nested = shown.nested;
            }
            None if above.contains(&child.block) => {
                child.showing =
                    Rc::new(RefCell::new(Showing::Failed(ALREADY_OPEN_ABOVE.to_owned())));
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
    /// which none of its children may show again; its last is `parent`'s own. The children of
    /// the children shown are followed the same way. Then the docks of every editor shown hear
    /// what each app's said, and the root's host is handed what they have for each editor.
    pub fn settle(
        &mut self,
        parent: &Embedded,
        above: &[[u8; 16]],
        embedding: &mut Embedding,
        spawning: &Spawning,
        now: Duration,
    ) {
        let mut docks = std::mem::take(&mut self.docks);
        let mut settling = Settling {
            embedding,
            spawning,
            now,
            docks: &mut docks,
        };
        self.settle_in(parent, above, &mut settling, Vec::new(), true);
        self.hear_docks(parent, &mut docks);
        self.docks = docks;
    }

    /// `settle` for one level: `asks` are the bodies of joined docks an editor above placed whose
    /// paths lead through these children, and `on_screen` whether `parent` shows any view.
    fn settle_in(
        &mut self,
        parent: &Embedded,
        above: &[[u8; 16]],
        settling: &mut Settling<'_>,
        asks: Vec<TabAsk>,
        on_screen: bool,
    ) {
        let screens = parent.screens();
        self.instance = parent.instance();
        self.placers = screens
            .iter()
            .map(|screen| (parent.layer_id(screen.screen), screen.instance, screen.view))
            .collect();
        let app = app_key(self.instance);
        let block = above.last().map(|block| Uuid::from_bytes(*block));
        let mut editors: Vec<EditorInstanceId> = Vec::new();
        // An editor shown only as a preview has no dock, as under be-host, which keeps previews
        // out of its docks: another editor of its block may be docked.
        for screen in screens.iter().filter(|screen| screen.view != ViewId::PREVIEW) {
            if !editors.contains(&screen.instance) {
                editors.push(screen.instance);
                settling.docks.register(screen.instance, &app, block);
            }
        }
        // The asks of editors no longer shown are forgotten: one opened again counts afresh.
        self.asked
            .retain(|editor, _| screens.iter().any(|screen| screen.instance == *editor));
        self.asks
            .retain(|ask| screens.iter().any(|screen| screen.instance == ask.editor));
        self.requests = screens;
        let placed = parent.placed();
        let mut wanted = Vec::new();
        let mut asking: HashMap<EditorInstanceId, (u64, ChildId)> = HashMap::default();
        let mut views: HashMap<(EditorInstanceId, ViewId), Vec<ShownChild>> = HashMap::default();
        let mut docked = docked_children(&placed);
        self.bodies.clear();
        for placements in &placed {
            let raster = self
                .requests
                .iter()
                .find(|screen| {
                    (screen.instance, screen.view) == (placements.instance, placements.view)
                })
                .map_or(1.0, |screen| screen.metrics.raster);
            let told = views
                .entry((placements.instance, placements.view))
                .or_default();
            for placement in &placements.children {
                let child = placement.child;
                let ask = asking.entry(placements.instance).or_insert((0, child));
                if placement.keyboard_ask > ask.0 {
                    *ask = (placement.keyboard_ask, child);
                }
                if let ChildContent::DockTab(path) = &placement.content {
                    // A body the docks do not show is not one of the placer's children yet, as
                    // under be-host: it hears nothing of it.
                    if let Some((_, tab)) = settling.docks.resolve(placements.instance, path) {
                        self.bodies.push(TabAsk {
                            editor: placements.instance,
                            view: placements.view,
                            body: child,
                            via: path.via.clone(),
                            tab,
                            placement: placement.clone(),
                            raster,
                        });
                    }
                    continue;
                }
                match Child::placed(
                    placements.instance,
                    placements.view,
                    raster,
                    in_view(placement, placements.view),
                ) {
                    Some(child) => wanted.push(child),
                    None => told.push(ShownChild {
                        child: child.0,
                        shown_in: None,
                        active: false,
                        error: Some(NOT_SHOWN_YET.to_owned()),
                        preview: ShownPreview::None,
                    }),
                }
            }
        }
        self.hear_asks(asking);
        let taking = self.framing_in(parent, &placed, &self.requests);
        for editor in &editors {
            match on_screen {
                true => {
                    let children = docked.remove(editor).unwrap_or_default();
                    let framed = taking.iter().find(|framed| framed.0 == *editor).map(
                        |(_, view, child, block)| DockedChild {
                            child: *child,
                            tab: view.0,
                            block: Uuid::from_bytes(*block),
                        },
                    );
                    settling.docks.placed(*editor, &children, framed);
                }
                false => settling.docks.offscreen(*editor),
            }
        }
        let Reconciled {
            mut kept,
            opened,
            gone,
        } = reconcile(std::mem::take(&mut self.shown), wanted, above);
        for child in &opened {
            spawn(&kept[*child], settling.spawning, settling.now);
        }
        for mut gone in gone {
            // A docked child the dock that hosts it keeps stays, shown where that dock places
            // its views.
            if gone
                .hosted()
                .is_some_and(|(_, editor)| settling.docks.docked(editor))
            {
                gone.placed = false;
                gone.framing = false;
                kept.push(gone);
                continue;
            }
            release(&mut gone, settling.embedding, Some(&mut *settling.docks));
        }
        self.shown = kept;
        self.forget_revoked();
        let mut asks: Vec<TabAsk> = asks.into_iter().chain(self.bodies.clone()).collect();
        let mut nested = above.to_vec();
        for child in &mut self.shown {
            child.framing = child.placed
                && taking.iter().any(|(editor, view, id, _)| {
                    (*editor, *view, *id) == (child.editor, child.view, child.child)
                });
            let failed = {
                let showing = child.showing.borrow();
                match &*showing {
                    Showing::Shown { hosted, .. } => hosted.app.failure(),
                    Showing::Opening | Showing::Label | Showing::Failed(_) => None,
                }
            };
            if let Some(why) = failed {
                release(child, settling.embedding, Some(&mut *settling.docks));
                *child.showing.borrow_mut() = Showing::Failed(why);
            }
            if let Showing::Shown { hosted, embedded } = &mut *child.showing.borrow_mut()
                && !*embedded
            {
                settling.embedding.embed(hosted.app.clone());
                *embedded = true;
            }
            let hosted = child.hosted();
            // The bodies of its dock's views a joined dock places, and those of its own
            // children's, whose paths lead on through it.
            let (mine, rest): (Vec<TabAsk>, Vec<TabAsk>) =
                std::mem::take(&mut asks).into_iter().partition(|ask| {
                    ask.editor == child.editor && ask.via.first() == Some(&child.child)
                });
            asks = rest;
            for tab in &mut child.tabs {
                tab.asked = false;
            }
            let mut deeper = Vec::new();
            if let Some((app, editor)) = &hosted {
                let mut screens = Vec::new();
                if child.placed && child.previewed() {
                    // A preview shows its editor's preview view, or nothing when it draws none,
                    // whose label its placer draws.
                    child.main = (child.shown_preview() == ShownPreview::View)
                        .then(|| preview_screen(&child.placement, *editor, child.raster));
                } else if child.placed {
                    let placer = self.requests.iter().find(|screen| {
                        (screen.instance, screen.view) == (child.editor, child.view)
                    });
                    let mut main = match placer.filter(|_| child.framing) {
                        Some(placer) => framing_screen(placer, &child.placement, *editor),
                        None => child_screen(&child.placement, *editor, child.raster),
                    };
                    main.docked = settling.docks.docked(*editor);
                    child.main = Some(main);
                }
                screens.extend(child.main.clone());
                for ask in mine {
                    match ask.via.len() {
                        1 => {
                            let screen = child.tab_screen(ask.tab);
                            screens.push(tab_screen(
                                screen,
                                *editor,
                                &ask,
                                app.content(screen).is_some(),
                            ));
                        }
                        _ => deeper.push(TabAsk {
                            editor: *editor,
                            via: ask.via[1..].to_vec(),
                            ..ask
                        }),
                    }
                }
                child.tabs.retain(|tab| tab.asked);
                child.nested.withdraw_chrome(app, &mut screens);
                app.show(screens);
            } else {
                child.tabs.clear();
            }
            child.layer = hosted.as_ref().map(|(app, _)| app.layer_id(child.screen()));
            child.up = hosted.as_ref().is_some_and(|(app, _)| app.started());
            if child.placed {
                views
                    .entry((child.editor, child.view))
                    .or_default()
                    .push(ShownChild {
                        child: child.child.0,
                        shown_in: hosted.as_ref().map(|(app, editor)| OpenedIn {
                            instance: app.instance(),
                            editor: editor.0,
                        }),
                        active: child.takes_input(&self.revoked),
                        error: match &*child.showing.borrow() {
                            Showing::Failed(why) => Some(why.clone()),
                            Showing::Opening | Showing::Label | Showing::Shown { .. } => None,
                        },
                        preview: child.shown_preview(),
                    });
            }
            if let Some((app, _)) = hosted {
                nested.push(child.block);
                let shows = child.placed || !child.tabs.is_empty();
                child
                    .nested
                    .settle_in(&app, &nested, settling, deeper, shows);
                nested.pop();
            }
        }
        // The bodies of joined docks its own editors place: available once the editor owning
        // each shows it.
        for ask in &self.bodies {
            let owner = self.owner(ask.editor, &ask.via);
            let shown_in = owner.and_then(|owner| {
                let (app, editor) = owner.hosted()?;
                owner
                    .tabs
                    .iter()
                    .any(|tab| tab.tab == ask.tab)
                    .then(|| OpenedIn {
                        instance: app.instance(),
                        editor: editor.0,
                    })
            });
            views
                .entry((ask.editor, ask.view))
                .or_default()
                .push(ShownChild {
                    child: ask.body.0,
                    shown_in,
                    active: true,
                    error: None,
                    preview: ShownPreview::None,
                });
        }
        self.tell(views);
    }

    /// Hears what every app shown said of its docks, once what each placed is in (`settle`),
    /// and hands the root's host what the docks have for each editor (`host.dock`). Menus, their
    /// picks and details asked of a joined dock wait for the root to take its editors' shell
    /// messages.
    fn hear_docks(&self, parent: &Embedded, docks: &mut Docks) {
        let apps: Vec<Embedded> = std::iter::once(parent.clone()).chain(self.apps()).collect();
        for app in &apps {
            docks.heard(&app_key(app.instance()), app.take_docks());
        }
        for app in &apps {
            let key = app_key(app.instance());
            let mut updates: Vec<(EditorInstanceId, Vec<DockUpdate>)> = Vec::new();
            for (editor, update) in docks.take(&key) {
                match updates.iter_mut().find(|(held, _)| *held == editor) {
                    Some((_, held)) => held.push(update),
                    None => updates.push((editor, vec![update])),
                }
            }
            for (editor, updates) in updates {
                let told = host::dock(app.instance(), editor.0, be_wasm_ops::encode(&updates));
                be_wasm_ops::guest::spawn(async move {
                    // A host that stopped answering stops the root too.
                    let _ = told.await;
                });
            }
            let _ = docks.take_picks(&key);
            let _ = docks.take_details(&key);
        }
        // be-host-runtime marks the apps `take_changed` names for a frame. Here every app's
        // updates were just taken and handed to the host, whose relayed answer to an editor's
        // dock watch wakes its app, and what the app lays out then reaches the root as any
        // layout does (`Embedded::wants`): there is nothing left to mark.
        let _ = docks.take_changed();
    }

    /// The child the path `via` leads to from `editor`'s children, through each one's own: the
    /// editor owning a body of a joined dock.
    fn owner(&self, editor: EditorInstanceId, via: &[ChildId]) -> Option<&Child> {
        let (first, rest) = via.split_first()?;
        let child = self
            .shown
            .iter()
            .find(|child| child.editor == editor && child.child == *first)?;
        if rest.is_empty() {
            return Some(child);
        }
        let (_, inner) = child.hosted()?;
        child.nested.owner(inner, rest)
    }

    /// Forgets that a child was taken back once it is no longer placed active: a child taken
    /// back stays passive only while it is.
    /// A child taken back from its placer's frame takes it again once placed other than active
    /// or live first.
    fn forget_revoked(&mut self) {
        let shown = &self.shown;
        let placed =
            |editor: &EditorInstanceId, view: &ViewId, id: &ChildId, modes: &[ChildMode]| {
                shown.iter().any(|child| {
                    (child.editor, child.view, child.child) == (*editor, *view, *id)
                        && child.placed
                        && modes.contains(&child.placement.mode)
                })
            };
        self.revoked
            .retain(|(editor, view, id)| placed(editor, view, id, &[ChildMode::Active]));
        self.frame_revoked.retain(|(editor, view, id)| {
            placed(editor, view, id, &[ChildMode::Active, ChildMode::Live])
        });
    }

    /// Takes the asks the placements just settled carry, `asking`: for each editor, its highest
    /// serial and the child that carries it. One above every serial acted on for that editor is
    /// a new ask, in place of the one it made before.
    fn hear_asks(&mut self, asking: HashMap<EditorInstanceId, (u64, ChildId)>) {
        for (editor, (serial, child)) in asking {
            let acted = self.asked.entry(editor).or_default();
            if serial <= *acted {
                continue;
            }
            *acted = serial;
            self.asks.retain(|ask| ask.editor != editor);
            self.asks.push(Ask {
                editor,
                child,
                new: true,
            });
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
    /// with the next `settle`. A child taking its placer's frame over gives it back the same
    /// way, and so does every child of that view that could take it, as be-host's
    /// `revoke_frame_child` does, so the frame goes to none of its siblings. Their own
    /// children's likewise.
    pub fn revoke(&mut self, pressed: &[Pressed]) {
        let mut frames_back = Vec::new();
        for child in &self.shown {
            let active =
                child.placement.mode == ChildMode::Active && child.takes_input(&self.revoked);
            if !active && !child.framing {
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
            if taken && active {
                self.revoked.insert((child.editor, child.view, child.child));
            }
            if taken && child.framing {
                frames_back.push((child.editor, child.view));
            }
        }
        for (editor, view) in frames_back {
            for child in &self.shown {
                if (child.editor, child.view) == (editor, view) && could_frame(&child.placement) {
                    self.frame_revoked
                        .insert((child.editor, child.view, child.child));
                }
            }
        }
        for child in &mut self.shown {
            child.nested.revoke(pressed);
        }
    }

    /// Withdraws the chrome of each of `screens`, the screens `app` (the app these are the
    /// children of) is asked for, that a child of these takes the frame of: the view draws none
    /// while the child draws it.
    pub fn withdraw_chrome(&self, app: &Embedded, screens: &mut [ScreenRequest]) {
        let taking = self.framing_in(app, &app.placed(), screens);
        for screen in screens {
            let taken = taking
                .iter()
                .any(|(editor, view, ..)| (*editor, *view) == (screen.instance, screen.view));
            if let Some(frame) = screen.frame.as_mut().filter(|_| taken) {
                frame.chrome = FrameChrome::None;
            }
        }
    }

    /// Whether the child `id` that `editor` placed in its view `view` shows its screen, which
    /// draws the frame it takes over: not one still opening, one that failed or one refused.
    fn shows_frame(&self, editor: EditorInstanceId, view: ViewId, id: ChildId) -> bool {
        self.shown.iter().any(|child| {
            (child.editor, child.view, child.child) == (editor, view, id) && child.shows_frame()
        })
    }

    /// `framing` for these children of `app`, placed as `placed` in the views `requests` asks
    /// for: in each editor's content view, as `app` reports it, a child that shows.
    fn framing_in(
        &self,
        app: &Embedded,
        placed: &[ChildPlacements],
        requests: &[ScreenRequest],
    ) -> Vec<(EditorInstanceId, ViewId, ChildId, [u8; 16])> {
        let content = |editor: EditorInstanceId| {
            content_view(editor, requests, &|screen| app.content(screen).is_some())
        };
        let shows = |editor, view, id| self.shows_frame(editor, view, id);
        framing(
            placed,
            requests,
            &Framing {
                frame_revoked: &self.frame_revoked,
                revoked: &self.revoked,
                content: &content,
                shows: &shows,
            },
        )
    }

    /// Moves the keyboard where an ask says, with the keyboard now in the layer `focused` and
    /// the layers just drawn told by `drawn`. An ask is acted on only while the keyboard is
    /// within its editor's views and the children they show (or nowhere, as one the ask took
    /// out leaves it), so it never takes the keyboard from elsewhere. A child that shows (its
    /// app runs and its layer is drawn) takes the keyboard at once. One that does not yet takes
    /// it once it does, while nothing else took it meanwhile; the keyboard leaves its placer's
    /// own screen at the settle that carries the ask (`KeyboardMove::Out`), and one in another of
    /// the editor's children stays there until then. An ask ends once acted on, or when its
    /// child goes, could not be shown, or is of a kind the root shows nothing of. The first move
    /// asked wins, its own children's after its own.
    pub fn keyboard(
        &mut self,
        focused: Option<LayerId>,
        drawn: &dyn Fn(LayerId) -> bool,
    ) -> Option<KeyboardMove> {
        let mut moved = None;
        for mut ask in std::mem::take(&mut self.asks) {
            let new = std::mem::replace(&mut ask.new, false);
            if focused.is_some_and(|focused| !self.within(ask.editor, focused)) {
                continue;
            }
            let Some(child) = self
                .shown
                .iter()
                .find(|child| child.editor == ask.editor && child.child == ask.child)
            else {
                continue;
            };
            if matches!(&*child.showing.borrow(), Showing::Failed(_)) {
                continue;
            }
            match child
                .content_layer()
                .filter(|layer| child.up && drawn(*layer))
            {
                Some(layer) => {
                    moved = moved.or(Some(KeyboardMove::Into(layer)));
                    continue;
                }
                None => self.asks.push(ask),
            }
            let in_placer = focused.is_some_and(|focused| {
                self.placers
                    .iter()
                    .any(|(layer, editor, _)| *editor == ask.editor && *layer == focused)
            });
            if new && in_placer {
                moved = moved.or(Some(KeyboardMove::Out));
            }
        }
        for child in &mut self.shown {
            let inner = child.nested.keyboard(focused, drawn);
            moved = moved.or(inner);
        }
        moved
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
            let mut into = match child.shows(layer) {
                true => Vec::new(),
                false => child.nested.focused_into(layer),
            };
            if child.shows(layer) || !into.is_empty() {
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
        self.release_in(embedding, None);
        self.docks = Docks::default();
    }

    /// `release`, forgetting the docks of every editor let go of, when `docks` holds them.
    fn release_in(&mut self, embedding: &mut Embedding, mut docks: Option<&mut Docks>) {
        for mut child in std::mem::take(&mut self.shown) {
            release(&mut child, embedding, docks.as_deref_mut());
        }
        self.told.clear();
        self.asks.clear();
        self.asked.clear();
        self.bodies.clear();
    }

    /// The apps of every child shown, any number of levels deep, each with every screen it
    /// painted and the id of the layer showing it: what the root's paintings carry.
    pub fn screens(&self) -> Vec<(Embedded, ScreenId, LayerId)> {
        let mut screens = Vec::new();
        for child in &self.shown {
            if let Some((app, _)) = child.hosted() {
                let shown =
                    std::iter::once(child.screen()).chain(child.tabs.iter().map(|tab| tab.screen));
                for screen in shown {
                    if let Some(layer) = app.layer(screen, pos2(0.0, 0.0)) {
                        screens.push((app.clone(), screen, layer.id));
                    }
                }
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
    /// that takes input shows, then those placed above it and the child taking its frame over.
    /// Each child is clipped to what its placement shows of it within what shows of its placer,
    /// and shows its own children the same way. The bodies of joined docks the placer places
    /// show the views of the editors owning them there.
    pub fn layers(&self, screen: &ScreenRequest, mut layer: Layer, layers: &mut Vec<Layer>) {
        // From the placer's view's points to the window's: through its surface's pixels.
        let pixels = 1.0 / layer.transform.across().length().max(f32::EPSILON);
        let to_window = Affine::scaling(pixels).then(&layer.transform);
        let shown = layer.transform.bounds(layer.clip);
        let mut below = Vec::new();
        let mut over = Vec::new();
        let mut holes = Vec::new();
        let placed_in = |editor: EditorInstanceId, view: ViewId| {
            (editor, view) == (screen.instance, screen.view)
        };
        for child in &self.shown {
            if !child.placed || !placed_in(child.editor, child.view) {
                continue;
            }
            let Some((app, editor)) = child.hosted() else {
                continue;
            };
            let placement = &child.placement;
            let (at, clip) = match child.framing {
                true => {
                    let whole = Rect::from_min_max(
                        pos2(0.0, 0.0),
                        pos2(screen.metrics.logical.x, screen.metrics.logical.y),
                    );
                    let at = to_window.bounds(whole);
                    (at, at.intersect(shown))
                }
                false => (
                    to_window.bounds(placement.rect),
                    to_window
                        .bounds(placement.clip.intersect(placement.rect))
                        .intersect(shown),
                ),
            };
            let Some(mut shown_child) = app.layer(child.screen(), at.min) else {
                continue;
            };
            let Some(from_window) = shown_child.transform.inverse() else {
                continue;
            };
            shown_child.clip = shown_child.clip.intersect(from_window.bounds(clip));
            shown_child.input.placer = Some(layer.id);
            let active = child.takes_input(&self.revoked);
            shown_child.input.hit = match (child.framing, active) {
                (true, _) => framing_hit(
                    shown_child.size,
                    pixels,
                    app.content(CHILD_SCREEN),
                    placement.rect.intersect(placement.clip),
                ),
                (false, true) => shown_child.input.hit,
                (false, false) => Area::Nowhere,
            };
            if active && !child.framing && placement.layer == ChildLayer::Below {
                holes.push(hole(placement, pixels, layer.clip));
            }
            let request = child.main.clone().unwrap_or_else(|| match child.previewed() {
                true => preview_screen(placement, editor, child.raster),
                false => child_screen(placement, editor, child.raster),
            });
            let into = match (child.framing, placement.layer) {
                (false, ChildLayer::Below) => &mut below,
                (true, _) | (false, ChildLayer::Above) => &mut over,
            };
            child.nested.layers(&request, shown_child, into);
        }
        for ask in &self.bodies {
            if !placed_in(ask.editor, ask.view) {
                continue;
            }
            let Some(owner) = self.owner(ask.editor, &ask.via) else {
                continue;
            };
            let (Some((app, editor)), Some(tab)) = (
                owner.hosted(),
                owner.tabs.iter().find(|tab| tab.tab == ask.tab),
            ) else {
                continue;
            };
            let placement = &ask.placement;
            let at = to_window.bounds(placement.rect);
            let clip = to_window
                .bounds(placement.clip.intersect(placement.rect))
                .intersect(shown);
            let Some(mut body) = app.layer(tab.screen, at.min) else {
                continue;
            };
            let Some(from_window) = body.transform.inverse() else {
                continue;
            };
            body.clip = body.clip.intersect(from_window.bounds(clip));
            body.input.placer = Some(layer.id);
            if placement.layer == ChildLayer::Below {
                holes.push(hole(placement, pixels, layer.clip));
            }
            let request = tab_screen(tab.screen, editor, ask, false);
            let into = match placement.layer {
                ChildLayer::Below => &mut below,
                ChildLayer::Above => &mut over,
            };
            owner.nested.layers(&request, body, into);
        }
        layer.input.hit = placer_hit(layer.size, holes);
        layers.extend(below);
        layers.push(layer);
        layers.extend(over);
    }
}

/// Where a child taking its placer's frame over, its layer `size` pixels big at `pixels` a
/// point, is hit: its whole frame but where its content shows outside its placed rect `placed`
/// (in its points), which is where the placer's world shows and which a press takes the frame
/// back from. The whole frame until its view says where its content is.
fn framing_hit(
    size: (u32, u32),
    pixels: f32,
    content: Option<be_sdk_api::Insets>,
    placed: Rect,
) -> Area {
    let Some(content) = content else {
        return Area::Whole;
    };
    let to_pixels = |rect: Rect| {
        Rect::from_min_max(
            pos2(rect.min.x * pixels, rect.min.y * pixels),
            pos2(rect.max.x * pixels, rect.max.y * pixels),
        )
    };
    let whole = Rect::from_min_max(pos2(0.0, 0.0), pos2(size.0 as f32, size.1 as f32));
    let points = Rect::from_min_max(
        pos2(0.0, 0.0),
        pos2(size.0 as f32 / pixels, size.1 as f32 / pixels),
    );
    let content = to_pixels(content.inside(points));
    let mut hit = whole.subtract(&[content]);
    let placed = to_pixels(placed).intersect(content);
    if placed.is_positive() {
        hit.push(placed);
    }
    Area::Region(hit)
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

/// Whether a child placed in `mode` runs its block's editor: to show it where it is placed, or
/// its preview view for a preview, but not one kept placed hidden.
fn runs_editor(mode: ChildMode) -> bool {
    matches!(
        mode,
        ChildMode::Passive
            | ChildMode::Active
            | ChildMode::Live
            | ChildMode::InPlace
            | ChildMode::Preview
    )
}

/// `placement` as its placer's view `view` shows it: a child placed in a preview is a preview
/// too, as be-host-runtime's `Instances` makes it, unless it is kept placed hidden.
fn in_view(placement: &ChildPlacement, view: ViewId) -> ChildPlacement {
    let mut placement = placement.clone();
    if view == ViewId::PREVIEW && placement.mode != ChildMode::Hidden {
        placement.mode = ChildMode::Preview;
    }
    placement
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
pub fn child_screen(
    placement: &ChildPlacement,
    editor: EditorInstanceId,
    raster: f32,
) -> ScreenRequest {
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

/// The screen a child placed as a preview, as `placement`, asks of its editor `editor`: its
/// preview view the size of its place, at its placer's `raster` but no more than
/// `PREVIEW_PIXELS` along its longer side, in no frame.
pub fn preview_screen(
    placement: &ChildPlacement,
    editor: EditorInstanceId,
    raster: f32,
) -> ScreenRequest {
    let size = placement.rect.size();
    let longest = size.x.max(size.y) * raster;
    let raster = match longest > PREVIEW_PIXELS {
        true => raster * PREVIEW_PIXELS / longest,
        false => raster,
    };
    ScreenRequest {
        screen: PREVIEW_SCREEN,
        view: ViewId::PREVIEW,
        placement: ViewPlacement::Parent(ParentView::Preview),
        frame: None,
        ..child_screen(placement, editor, raster)
    }
}

/// The screen a child placed as `placement` asks of its editor `editor` while it takes over the
/// frame of `placer`, the screen its placer is asked for: the whole of it, with its placer's
/// frame drawn and the child's placed rect, what shows of it, as the frame's content, where it
/// draws its content as it was placed.
pub fn framing_screen(
    placer: &ScreenRequest,
    placement: &ChildPlacement,
    editor: EditorInstanceId,
) -> ScreenRequest {
    ScreenRequest {
        screen: CHILD_SCREEN,
        instance: editor,
        view: ViewId::MAIN,
        placement: ViewPlacement::Parent(ParentView::Main),
        frame: placer.frame.clone().map(|frame| FrameSpec {
            chrome: FrameChrome::Drawn,
            content: Some(placement.rect.intersect(placement.clip)),
            ..frame
        }),
        metrics: ViewportMetrics {
            visible: ViewPos2::ZERO,
            ..placer.metrics
        },
        windows: false,
        host_place: None,
        docked: false,
        anchor_space: None,
        fit: false,
        origin: None,
    }
}

/// The screen `screen` an editor `editor` is asked for the view of its dock's tab or bar that
/// `ask` places, the size of the body's place at its placer's raster, with a frame where it holds
/// the editor's `content`: where its camera, its chrome and a child taking its frame over are.
fn tab_screen(
    screen: ScreenId,
    editor: EditorInstanceId,
    ask: &TabAsk,
    content: bool,
) -> ScreenRequest {
    let mut request = child_screen(&ask.placement, editor, ask.raster);
    let origin = ask.placement.rect.min.to_vec2();
    request.screen = screen;
    request.view = ViewId(ask.tab);
    request.placement = ViewPlacement::Dock;
    request.frame = content.then(|| FrameSpec {
        chrome: FrameChrome::Drawn,
        anchor: ask.placement.anchor.map(|anchor| anchor.translate(-origin)),
        ..FrameSpec::default()
    });
    request
}

/// What decides which child takes a view's frame over (`framing`).
struct Framing<'a> {
    /// The children taken back from their placers' frames, and as active children.
    frame_revoked: &'a HashSet<(EditorInstanceId, ViewId, ChildId)>,
    revoked: &'a HashSet<(EditorInstanceId, ViewId, ChildId)>,
    /// The view of each editor that holds its content (`content_view`).
    content: &'a dyn Fn(EditorInstanceId) -> ViewId,
    /// Whether a child shows its screen, which draws the frame it takes.
    shows: &'a dyn Fn(EditorInstanceId, ViewId, ChildId) -> bool,
}

/// Whether a child placed as `placement` would take its placer's frame over: active or live,
/// with no frame of its own, showing some of its block.
fn could_frame(placement: &ChildPlacement) -> bool {
    matches!(placement.mode, ChildMode::Active | ChildMode::Live)
        && !placement.own_frame
        && placement.rect.is_positive()
        && matches!(placement.content, ChildContent::Block { .. })
}

/// For each editor of those that placed `placed` in the views `requests` asks for, the child
/// that takes the frame of its content view over, if any, as be-host's `Instances::framing`
/// says: the first that could (`could_frame`), in a content view with a frame, not taken back,
/// and showing, so its placer never loses its chrome to a child that draws none. By the editor,
/// the view, the child and its block.
fn framing(
    placed: &[ChildPlacements],
    requests: &[ScreenRequest],
    rules: &Framing<'_>,
) -> Vec<(EditorInstanceId, ViewId, ChildId, [u8; 16])> {
    let mut framing: Vec<(EditorInstanceId, ViewId, ChildId, [u8; 16])> = Vec::new();
    for placements in placed {
        let (editor, view) = (placements.instance, placements.view);
        if framing.iter().any(|framed| framed.0 == editor) || (rules.content)(editor) != view {
            continue;
        }
        let framed = requests.iter().any(|screen| {
            (screen.instance, screen.view) == (editor, view) && screen.frame.is_some()
        });
        if !framed {
            continue;
        }
        let taking = placements.children.iter().find_map(|child| {
            let ChildContent::Block { block_id, .. } = child.content else {
                return None;
            };
            let key = (editor, view, child.child);
            (could_frame(child)
                && !rules.frame_revoked.contains(&key)
                && !rules.revoked.contains(&key)
                && (rules.shows)(editor, view, child.child))
            .then_some((editor, view, child.child, block_id))
        });
        framing.extend(taking);
    }
    framing
}

/// The view of `editor`, among those `requests` asks for, that holds its content, as be-host's
/// `Instances::content_view` says: its main view if that reports content (`holds`), else the
/// first view of its dock that does, else its main view.
fn content_view(
    editor: EditorInstanceId,
    requests: &[ScreenRequest],
    holds: &dyn Fn(ScreenId) -> bool,
) -> ViewId {
    let of_editor = || {
        requests
            .iter()
            .filter(move |screen| screen.instance == editor)
    };
    if of_editor().any(|screen| screen.view == ViewId::MAIN && holds(screen.screen)) {
        return ViewId::MAIN;
    }
    of_editor()
        .find(|screen| screen.placement == ViewPlacement::Dock && holds(screen.screen))
        .map_or(ViewId::MAIN, |screen| screen.view)
}

/// The block children each editor placed in its dock tabs' bodies (`child-placement.docks`):
/// what its docks hear it places, which joins each child's dock to its own.
fn docked_children(placed: &[ChildPlacements]) -> HashMap<EditorInstanceId, Vec<DockedChild>> {
    let mut docked: HashMap<EditorInstanceId, Vec<DockedChild>> = HashMap::default();
    for placements in placed {
        for placement in &placements.children {
            if let (Some(tab), ChildContent::Block { block_id, .. }) =
                (placement.docks, &placement.content)
            {
                docked
                    .entry(placements.instance)
                    .or_default()
                    .push(DockedChild {
                        child: placement.child,
                        tab,
                        block: Uuid::from_bytes(*block_id),
                    });
            }
        }
    }
    docked
}

/// How a root's docks name the app shown as `instance`, the handle of its instance.
fn app_key(instance: u64) -> String {
    instance.to_string()
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
            screens: Box::new(move |editor| {
                vec![match placement.mode == ChildMode::Preview {
                    true => preview_screen(&placement, editor, raster),
                    false => child_screen(&placement, editor, raster),
                }]
            }),
            surface: spawning.surface.clone(),
            fonts: Rc::clone(&spawning.fonts),
            preview: child.previewed(),
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
            Err(SpawnFailure {
                no_preview: true, ..
            }) => Showing::Label,
            Err(SpawnFailure { name, why, .. }) => Showing::Failed(match name {
                Some(name) => format!("{name} could not start: {why}"),
                None => why,
            }),
        };
        wake();
    });
}

/// Lets go of `child`'s app and of those of its own children, as embedded, and forgets their
/// editors' docks in `docks`.
fn release(child: &mut Child, embedding: &mut Embedding, mut docks: Option<&mut Docks>) {
    if let Some((app, editor)) = child.hosted() {
        embedding.remove(app.instance());
        if let Some(docks) = docks.as_deref_mut() {
            docks.forget(editor);
        }
    }
    child.tabs.clear();
    child.nested.release_in(embedding, docks);
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
