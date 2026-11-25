//! The children the editors of an app a root embeds place (guides/apps/children.md), as the root
//! shows them (plans/host_as_platform_adapter.md, 4g7b3): each block child placed to run its
//! editor is that block's editor, which the root's host opens and serves
//! (`Embedded::open_editor`), asked for a screen the size of its place and composited there:
//! under its placer's layer, which leaves the hole it cut for it, or over it for a child placed
//! `ChildLayer::Above`. A child that takes input where it is placed
//! (active, live or in place) takes the presses there; a passive one leaves them to its placer.
//! The children those editors place are shown the same way, any number of levels deep.
//!
//! Each staged app runs once for every child (`Pool`), holding every editor of it the root
//! shows, each asked for screens of its own (`main_screen`, `preview_screen_of`), and closed in
//! it once its place lets go of it (`Embedded::close_editor`). Each place has a live editor of its
//! own: a block placed in several places gets one in each, as two editor views of one open block
//! each get their own (plans/host_as_platform_adapter.md, 4g7b3i).
//!
//! The host tells each placer how its children are as its own children's statuses
//! (`be3:root`'s `host.show-children`): available once an editor shows the child, with what its
//! app's manifest says of that editor, or why the root shows nothing of it.
//!
//! An editor asks that a child take the keyboard in the child's placement (`keyboard-ask`), and
//! the root moves it at the settle that first carries the ask (`keyboard`), as be-host does,
//! never waiting on the child's app: keys are never held (decisions/keys_are_never_held.md).
//! Each placer above a child the keyboard goes into hears so (`focused_into`, `child-focused`).
//! A press anywhere but in an active child takes it back (`revoke`); an Escape it leaves to its
//! placer does not.
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
//! once the host says it composites it (`ChildPreview::Composited`), as a passive child is, hit
//! nowhere. One whose editor draws no preview runs no app: its placer draws its block's label.
//! Promoted (a canvas focusing its card), it keeps its editor, asked for its main view.
//!
//! Each child's layer is placed through its placer's (`placed_transform`): turned by its
//! rotation, faded by its opacity, and drawn at the zoom of its place over its intrinsic size
//! when it has one (`placed_zoom`). A child placed to fit its content is asked for at most that
//! room, and a view's occluders give their presses to the placer.
//!
//! Left for later (plans/host_as_platform_adapter.md, 4g7b3e to 4g7b3g): an ask acted on only
//! while no popover or keyboard-taking layer of the editor's is open (the editor's own popovers
//! are in its screen, so the root does not see them), the press that promotes a child replayed in
//! it, and every other kind of child (web views, host panels, program windows, creations and
//! artifact settings), which the placer hears are unavailable.
//!
//! A picker, a rename form or an Export dialog is a dialog (`Shown::Dialog`), opened like a
//! block's editor in the editor that runs it, its placer's own: what it tells the shell that
//! its placer's status says, the root relays to the host (`host.tell-shell`).

use std::{
    cell::{Cell, RefCell},
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
    AnchorSpace, ChildContent, ChildId, ChildLayer, ChildMode, ChildPlacement, ChildPlacements,
    DockUpdate, EditorInstanceId, Fonts, FrameChrome, FrameSpec, HostTransform, MenuItem,
    ParentView, Pos2 as ViewPos2, ScreenId, ScreenRequest, SurfaceSpec, Vec2 as ViewVec2,
    ViewAnchor, ViewId, ViewPlacement, ViewportMetrics,
    app::{EditorEvent, MenuChoice},
    shell_types::{ChildMenuChoice, MenuSet, ShellMessage},
};
use be_sdk_app_host::docks::{DockedChild, Docks};
use be_wasm_root_api::{
    OpenedIn, PlacedContent, Shown, ShownBlock, ShownChild, ShownDialog, ShownPreview, ViewRect,
    ViewSize, guest::host,
};
use uuid::Uuid;

use crate::{Embedded, Embedding, Hosted, Pressed, tell_editor};

/// The screens a child's app is asked for of each of its editors are numbered from the editor's
/// instance (`screen_of`), at most this many: an app runs every editor of it the root shows.
const SCREENS_PER_EDITOR: u64 = 256;

/// An editor's main view, the size of its place, or of its placer's frame while it takes that
/// over (`main_screen`).
const MAIN_SCREEN: u64 = 1;

/// An editor's preview view, the size of its place (`preview_screen_of`).
const PREVIEW_SCREEN: u64 = 2;

/// The first of the screens of an editor's dock's tabs and bars that a joined dock places, each
/// keeping its screen while placed.
const FIRST_TAB_SCREEN: u64 = 3;

/// The `n`th screen of `editor` its app is asked for.
fn screen_of(editor: EditorInstanceId, n: u64) -> ScreenId {
    ScreenId(editor.0.wrapping_mul(SCREENS_PER_EDITOR).wrapping_add(n))
}

/// The screen a child's app is asked for its editor `editor`'s main view.
pub fn main_screen(editor: EditorInstanceId) -> ScreenId {
    screen_of(editor, MAIN_SCREEN)
}

/// The screen a child's app is asked for its editor `editor`'s preview view.
pub fn preview_screen_of(editor: EditorInstanceId) -> ScreenId {
    screen_of(editor, PREVIEW_SCREEN)
}

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
    /// The creations a placer committed (`commit-child`) whose editors have yet to open, with
    /// the placer (`relay_commits`).
    commits: Vec<(EditorInstanceId, be_sdk_api::shell_types::ChildCreation)>,
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
    /// The children placed that the root shows nothing of, by the view placing each, as the
    /// host is told them.
    unshown: Vec<((EditorInstanceId, ViewId), ShownChild)>,
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
    /// Where its placer's view may anchor views of its own, which the body's is placed in.
    space: Option<AnchorSpace>,
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

/// A child placed to run an editor: one place, showing an editor of its own (`Editor`).
struct Child {
    /// The editor that places it, its view and its id there.
    placer: EditorInstanceId,
    view: ViewId,
    child: ChildId,
    /// What it shows: its block, with the editor and view block its placement names, or a
    /// dialog (a picker, a rename form, an Export dialog), which its placement is.
    shown: Shown,
    /// Its block, for a block child: what no child of its editor's may show again.
    block: Option<[u8; 16]>,
    /// Its placement, as its placer's latest layout said.
    placement: ChildPlacement,
    /// The pixels per point its placer's screen is laid out at, which its own is.
    raster: f32,
    /// The editor it shows.
    editor: Rc<Editor>,
    /// The layer showing it, once its editor opened, as the last settle found it
    /// (`Child::shown_layer`).
    layer: Option<LayerId>,
    /// Whether its editor's app runs, embedded: up to take the keyboard.
    up: bool,
    /// Whether its placer places it now: one placed no longer stays while its dock is joined to
    /// the dock that hosts it, which shows it, and nothing shows its main view.
    placed: bool,
    /// Whether it takes its placer's frame over.
    framing: bool,
    /// The occluders of its placer's view placed after it, which hide it there: rects in the
    /// view's points where presses reach its placer.
    occluders: Vec<Rect>,
}

/// The editor a place shows, its own: a block placed in several places gets one in each.
struct Editor {
    /// What it shows, as the place that opened it named it.
    shown: Shown,
    showing: RefCell<Showing>,
    /// The children its own views place.
    nested: RefCell<Children>,
    /// The views of its dock a joined dock places, each a screen of its app.
    tabs: RefCell<Vec<TabScreen>>,
    /// Where its content shows in its main view and the size it wants, as the host was told
    /// last (`host.place-content`).
    told_content: Cell<Option<(Rect, Option<be_lib_geometry::Vec2>)>>,
    /// The size it is laid out at, placed with an intrinsic size, as the host was told last
    /// (`host.assign-size`).
    told_size: Cell<Option<be_lib_geometry::Vec2>>,
    /// What its place asked of it in the settle under way, while placed.
    asks: RefCell<Option<PlaceAsk>>,
    /// The views of its dock the joined docks place, asked in the settle under way.
    tab_asks: RefCell<Vec<TabAsk>>,
    /// Its main view and its preview view as asked last: a docked editor no place shows is asked
    /// for its main view still.
    main: RefCell<Option<ScreenRequest>>,
    preview: RefCell<Option<ScreenRequest>>,
}

/// What one place asked of the editor it shows in a settle.
#[derive(Clone)]
struct PlaceAsk {
    placement: ChildPlacement,
    raster: f32,
    /// Its placer's screen, while it takes that frame over.
    framing: Option<ScreenRequest>,
    /// Where its placer's view may anchor views of its own, which its own is placed in.
    space: Option<AnchorSpace>,
}

impl PlaceAsk {
    fn previewed(&self) -> bool {
        self.placement.mode == ChildMode::Preview
    }
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
    /// Placed only as a preview, its editor draws none: its placer draws its block's label, and
    /// no editor is opened for it.
    Label,
    /// Why it shows nothing: its app could not start or stopped, or its block is open above it.
    Failed(String),
}

impl Editor {
    fn new(shown: Shown, showing: Showing) -> Rc<Self> {
        Rc::new(Self {
            shown,
            showing: RefCell::new(showing),
            nested: RefCell::new(Children::default()),
            tabs: RefCell::new(Vec::new()),
            told_content: Cell::new(None),
            told_size: Cell::new(None),
            asks: RefCell::new(None),
            tab_asks: RefCell::new(Vec::new()),
            main: RefCell::new(None),
            preview: RefCell::new(None),
        })
    }

    /// The editor its app shows, once the host opened it.
    fn hosted(&self) -> Option<(Embedded, EditorInstanceId)> {
        match &*self.showing.borrow() {
            Showing::Shown { hosted, .. } => Some((hosted.app.clone(), hosted.editor)),
            Showing::Opening | Showing::Label | Showing::Failed(_) => None,
        }
    }

    /// Whether it pans and zooms its own world, once opened.
    fn pans(&self) -> bool {
        match &*self.showing.borrow() {
            Showing::Shown { hosted, .. } => hosted.pan_and_zoom,
            Showing::Opening | Showing::Label | Showing::Failed(_) => false,
        }
    }

    /// Whether it draws a preview: not known to draw none.
    fn draws_preview(&self) -> bool {
        match &*self.showing.borrow() {
            Showing::Shown { hosted, .. } => hosted.preview,
            Showing::Label => false,
            Showing::Opening | Showing::Failed(_) => true,
        }
    }

    /// The layers showing the views of its dock a joined dock places.
    fn tab_layers(&self) -> Vec<LayerId> {
        let Some((app, _)) = self.hosted() else {
            return Vec::new();
        };
        self.tabs
            .borrow()
            .iter()
            .map(|tab| app.layer_id(tab.screen))
            .collect()
    }

    /// The screen its app is asked for the view `tab` of its dock, kept while placed.
    fn tab_screen(&self, editor: EditorInstanceId, tab: u64) -> ScreenId {
        let mut tabs = self.tabs.borrow_mut();
        if let Some(held) = tabs.iter_mut().find(|held| held.tab == tab) {
            held.asked = true;
            return held.screen;
        }
        let first = screen_of(editor, FIRST_TAB_SCREEN).0;
        let last = screen_of(editor, SCREENS_PER_EDITOR - 1).0;
        let next = tabs
            .iter()
            .map(|held| held.screen.0 + 1)
            .max()
            .unwrap_or(first);
        // Past its editor's numbers, the lowest one free: a screen let go of is taken again
        // only once the numbers after the last one held run out.
        let next = match next <= last {
            true => next,
            false => (first..=last)
                .find(|free| tabs.iter().all(|held| held.screen.0 != *free))
                .unwrap_or(last),
        };
        let screen = ScreenId(next);
        tabs.push(TabScreen {
            tab,
            screen,
            asked: true,
        });
        screen
    }

    /// Asks its app for the screens its place asked for in the settle just made, once it is
    /// open (`Embedded::show_editor`): its main view, or its preview view while placed as a
    /// preview, and the views of its dock the joined docks place. Then tells the host where its
    /// content shows and the size it is laid out at.
    fn show(&self, docks: &Docks) {
        let asks = self.asks.borrow();
        let main_place = asks.as_ref().filter(|ask| !ask.previewed());
        let preview_place = asks.as_ref().filter(|ask| ask.previewed());
        let Some((app, editor)) = self.hosted() else {
            self.tabs.borrow_mut().clear();
            return;
        };
        let docked = docks.docked(editor);
        let main = match main_place {
            Some(ask) => {
                let zoom = placed_zoom(&ask.placement, self.pans()).map_or(1.0, |(_, zoom)| zoom);
                let mut main = match &ask.framing {
                    Some(placer) => framing_screen(placer, &ask.placement, editor),
                    None => {
                        let mut main = child_screen(&ask.placement, editor, ask.raster, zoom);
                        main.anchor_space = placed_space(ask.space, &ask.placement, zoom);
                        main
                    }
                };
                main.docked = docked;
                Some(main)
            }
            // A docked child no place shows stays open, asked for its main view still.
            None if docked => self.main.borrow().clone(),
            None => None,
        };
        let preview = preview_place
            .filter(|_| self.draws_preview())
            .map(|ask| preview_screen(&ask.placement, editor, ask.raster));
        let mut screens: Vec<ScreenRequest> = main.iter().chain(&preview).cloned().collect();
        for tab in self.tabs.borrow_mut().iter_mut() {
            tab.asked = false;
        }
        for ask in self.tab_asks.borrow().iter() {
            let screen = self.tab_screen(editor, ask.tab);
            screens.push(tab_screen(
                screen,
                editor,
                ask,
                app.content(screen).is_some(),
            ));
        }
        self.tabs.borrow_mut().retain(|tab| tab.asked);
        self.nested.borrow().withdraw_chrome(&app, &mut screens);
        app.show_editor(editor, screens);
        *self.main.borrow_mut() = main;
        *self.preview.borrow_mut() = preview;
        // Placed by its parent, as a card or taking its frame: an editor with a frame of its
        // own, or whose dock joined its placer's, keeps its own camera, as under be-host.
        match main_place {
            Some(ask) if !ask.placement.own_frame && !docked => {
                self.place_content(&app, editor, ask);
            }
            // Told afresh once placed so again.
            _ => self.told_content.set(None),
        }
        if let Some(ask) = main_place.filter(|ask| ask.framing.is_none()) {
            self.assign_size(&app, editor, ask);
        }
    }

    /// Tells the host where its content shows in its main view, and the size it wants, once
    /// either changed (`host.place-content`): its placed rect while it takes its placer's frame,
    /// else the whole view, which be-host's camera hands a placed child that pans and zooms as
    /// its view.
    fn place_content(&self, app: &Embedded, editor: EditorInstanceId, ask: &PlaceAsk) {
        let content = placed_content(&ask.placement, ask.framing.is_some());
        let intrinsic = app.intrinsic(editor);
        if self.told_content.get() == Some((content, intrinsic)) {
            return;
        }
        self.told_content.set(Some((content, intrinsic)));
        let told = host::place_content(PlacedContent {
            instance: app.instance(),
            editor: editor.0,
            content: ViewRect {
                min_x: content.min.x,
                min_y: content.min.y,
                max_x: content.max.x,
                max_y: content.max.y,
            },
            intrinsic: intrinsic.map(|size| ViewSize {
                width: size.x,
                height: size.y,
            }),
        });
        be_wasm_ops::guest::spawn(async move {
            // A host that stopped answering stops the root too.
            let _ = told.await;
        });
    }

    /// Tells the host the size it is laid out at, placed with an intrinsic size, once that
    /// changed (`host.assign-size`), as be-host tells a child it lays out so.
    fn assign_size(&self, app: &Embedded, editor: EditorInstanceId, ask: &PlaceAsk) {
        let Some((size, _)) = placed_zoom(&ask.placement, self.pans()) else {
            return;
        };
        if self.told_size.get() == Some(size) {
            return;
        }
        self.told_size.set(Some(size));
        let told = host::assign_size(
            app.instance(),
            editor.0,
            ViewSize {
                width: size.x,
                height: size.y,
            },
        );
        be_wasm_ops::guest::spawn(async move {
            // A host that stopped answering stops the root too.
            let _ = told.await;
        });
    }
}

/// The apps the children a root shows share, at every level: each staged app runs once, as one
/// instance holding every editor of it the root shows, one for each place.
#[derive(Default)]
struct Pool {
    /// Each staged app the root runs for its children, by its id.
    apps: HashMap<String, PooledApp>,
    /// The editors the settle under way followed, in order: each is asked for its screens
    /// once every level was.
    settled: Vec<Rc<Editor>>,
    /// Each editor's menus, by the view each is of, as it last told them
    /// (`shell-message.menu`): what a child's status shows of the editor drawing its frame.
    menus: HashMap<(EditorInstanceId, ViewId), Vec<MenuItem>>,
}

/// An app the root runs for its children's editors.
struct PooledApp {
    app: Embedded,
    /// The editors opened in it, or being opened: it stops once none is left.
    editors: usize,
    /// Whether it is embedded, so input routed to its screens reaches it.
    embedded: bool,
}

thread_local! {
    /// The root's pool: one root runs per guest.
    static POOL: RefCell<Pool> = RefCell::default();
}

/// The app running the staged app `app`, spawned with `module` if none does, holding one more
/// editor. One that stopped stays until the editors it held let go of it: an editor opened in
/// it meanwhile fails too.
fn reserve_app(app: String, module: String, spawning: &SpawnWith) -> Embedded {
    POOL.with_borrow_mut(|pool| {
        let pooled = pool.apps.entry(app.clone()).or_insert_with(|| PooledApp {
            app: Embedded::spawn_app(
                app,
                module,
                spawning.surface.clone(),
                Rc::clone(&spawning.fonts),
                spawning.now,
            ),
            editors: 0,
            embedded: false,
        });
        pooled.editors += 1;
        pooled.app.clone()
    })
}

/// The app `app` holds one editor fewer: once it holds none it stops at the next settle.
fn release_app(app: &Embedded) {
    POOL.with_borrow_mut(|pool| {
        if let Some(pooled) = pool
            .apps
            .values_mut()
            .find(|pooled| pooled.app.instance() == app.instance())
        {
            pooled.editors = pooled.editors.saturating_sub(1);
        }
    });
}

/// Embeds the app `app` once, so input routed to its screens reaches it.
fn embed_app(app: &Embedded, embedding: &mut Embedding) {
    POOL.with_borrow_mut(|pool| {
        if let Some(pooled) = pool
            .apps
            .values_mut()
            .find(|pooled| pooled.app.instance() == app.instance())
            && !pooled.embedded
        {
            embedding.embed(pooled.app.clone());
            pooled.embedded = true;
        }
    });
}

/// Lets go of the apps holding no editor, which stop once nothing else holds them.
fn sweep_apps(embedding: &mut Embedding) {
    POOL.with_borrow_mut(|pool| {
        pool.apps.retain(|_, pooled| {
            let keep = pooled.editors > 0;
            if !keep && pooled.embedded {
                embedding.remove(pooled.app.instance());
            }
            keep
        });
    });
}

/// What an editor's app is spawned with.
struct SpawnWith {
    surface: SurfaceSpec,
    fonts: Rc<Fonts>,
    now: Duration,
}

impl Child {
    /// The child `placement` places in `editor`'s view `view`, laid out at `raster` pixels per
    /// point, when it is a block child placed to run its editor: its editor not looked up yet
    /// (`reconcile`).
    fn placed(
        editor: EditorInstanceId,
        view: ViewId,
        raster: f32,
        placement: ChildPlacement,
        occluders: Vec<Rect>,
    ) -> Option<Self> {
        if !runs_editor(placement.mode) {
            return None;
        }
        let (shown, block) = match &placement.content {
            ChildContent::Block {
                block_id,
                view_block,
                editor: named,
                ..
            } => (
                Shown::Block(ShownBlock {
                    block: *block_id,
                    editor: *named,
                    view_block: *view_block,
                }),
                Some(*block_id),
            ),
            // A dialog an app runs for its placer, as be-host runs it in place of the child.
            ChildContent::Picker { .. }
            | ChildContent::Rename(_)
            | ChildContent::Export { .. }
            | ChildContent::Creation { .. } => {
                let dialog = ShownDialog {
                    content: be_wasm_ops::encode(&placement.content),
                    sheet: false,
                };
                (Shown::Dialog(dialog), None)
            }
            _ => return None,
        };
        Some(Self {
            placer: editor,
            view,
            child: placement.child,
            editor: Editor::new(shown.clone(), Showing::Opening),
            shown,
            block,
            placement,
            raster,
            layer: None,
            up: false,
            placed: true,
            framing: false,
            occluders,
        })
    }

    /// It, a dialog, opened as a sheet (`shown-dialog.sheet`).
    fn in_sheet(mut self) -> Self {
        if let Shown::Dialog(dialog) = &mut self.shown {
            dialog.sheet = true;
            self.editor = Editor::new(self.shown.clone(), Showing::Opening);
        }
        self
    }

    /// What names the editor it keeps while placed: its place, and what it shows. A child placed
    /// showing another block, or with another editor or view block named, is another editor.
    fn key(&self) -> ChildKey {
        let shows = match &self.shown {
            Shown::Block(shown) => {
                let mut key = shown.block.to_vec();
                key.extend(shown.editor.iter().flatten());
                key.push(0);
                key.extend(shown.view_block.iter().flatten());
                key
            }
            Shown::Dialog(dialog) => {
                let mut key = dialog.content.clone();
                key.push(u8::from(dialog.sheet));
                key
            }
        };
        (self.placer, self.view, self.child, shows)
    }

    /// The layer showing it, once its editor opened: its editor's screen's.
    fn shown_layer(&self) -> Option<LayerId> {
        let (app, editor) = self.hosted()?;
        Some(app.layer_id(self.screen(editor)))
    }

    /// Whether `layer` shows it or one of its own children, any number of levels deep.
    fn holds(&self, layer: LayerId) -> bool {
        self.shows(layer) || self.editor.nested.borrow().holds(layer)
    }

    /// Whether `layer` shows one of its views: its place, or a view of its editor's dock.
    fn shows(&self, layer: LayerId) -> bool {
        self.layer.filter(|_| self.placed) == Some(layer)
            || self.editor.tab_layers().contains(&layer)
    }

    /// Whether it shows its screen: its editor opened and its app painted it, so the frame it
    /// takes over is drawn.
    fn shows_frame(&self) -> bool {
        self.placed
            && self.up
            && self.hosted().is_some_and(|(app, editor)| {
                app.layer(main_screen(editor), pos2(0.0, 0.0)).is_some()
            })
    }

    /// The editor its app shows, once the host opened it.
    fn hosted(&self) -> Option<(Embedded, EditorInstanceId)> {
        self.editor.hosted()
    }

    /// Whether it is placed as a preview, which shows its editor's preview view.
    fn previewed(&self) -> bool {
        self.placement.mode == ChildMode::Preview
    }

    /// The screen of its editor `editor` it shows: the preview view while placed as a preview,
    /// else the main view.
    fn screen(&self, editor: EditorInstanceId) -> ScreenId {
        match self.previewed() {
            true => preview_screen_of(editor),
            false => main_screen(editor),
        }
    }

    /// The screen request its place shows: its editor's preview view or main view, as asked.
    fn request(&self) -> Option<ScreenRequest> {
        match self.previewed() {
            true => self.editor.preview.borrow().clone(),
            false => self.editor.main.borrow().clone(),
        }
    }

    /// The size it is laid out at and the zoom it is drawn at (`placed_zoom`), when it shows its
    /// editor's main view itself.
    fn zoom(&self) -> Option<(be_lib_geometry::Vec2, f32)> {
        if !self.placed || self.framing || self.previewed() {
            return None;
        }
        placed_zoom(&self.placement, self.editor.pans())
    }

    /// How it shows as a preview, which the host words its status from: not placed as one, its
    /// editor's preview view, or its block's label when its editor draws none.
    fn shown_preview(&self) -> ShownPreview {
        if !self.previewed() {
            return ShownPreview::None;
        }
        match self.editor.draws_preview() {
            true => ShownPreview::View,
            false => ShownPreview::Label,
        }
    }

    /// The size its placer hears it wants, as be-host's status says: the room it took when
    /// placed to fit its content (`fitted`), else the size its editor's content wants, as it
    /// told the shell (`Embedded::intrinsic`), which a canvas fits a new card to.
    fn intrinsic(&self) -> Option<ViewSize> {
        if self.placement.fit.is_some() {
            return self.fitted();
        }
        let (app, editor) = self.hosted()?;
        app.intrinsic(editor).map(|size| ViewSize {
            width: size.x,
            height: size.y,
        })
    }

    /// The room it took, placed to fit its content (`child-placement.fit`), in its placer's
    /// view's points, once it laid out: the room its layout said it used at its zoom, else its
    /// painted screen's size.
    fn fitted(&self) -> Option<ViewSize> {
        self.placement.fit?;
        let (app, editor) = self.hosted()?;
        let screen = self.screen(editor);
        let zoom = self.zoom().map_or(1.0, |(_, zoom)| zoom);
        // The room its layout said it used, as be-host hears it; else what it painted.
        if let Some(used) = app.used(screen) {
            return Some(ViewSize {
                width: used.x * zoom,
                height: used.y * zoom,
            });
        }
        let layer = app.layer(screen, pos2(0.0, 0.0))?;
        let points = layer.transform.across().length() * zoom;
        Some(ViewSize {
            width: layer.size.0 as f32 * points,
            height: layer.size.1 as f32 * points,
        })
    }

    /// The layer showing the view that holds its editor's content: its main view, or the view
    /// of its dock's tab its content moved to once its dock joined its placer's.
    fn content_layer(&self) -> Option<LayerId> {
        let tab = self.hosted().and_then(|(app, _)| {
            self.editor
                .tabs
                .borrow()
                .iter()
                .find(|tab| app.content(tab.screen).is_some())
                .map(|tab| app.layer_id(tab.screen))
        });
        tab.or(self.layer.filter(|_| self.placed))
    }

    /// The menu of the editor drawing the frame it takes over (`chrome_owner`), encoded as its
    /// status carries it, while it would take its placer's frame (placed active or live, not
    /// taken back from it, its editor open), as be-host's status says; empty otherwise.
    fn frame_menu(&self, frame_revoked: &HashSet<(EditorInstanceId, ViewId, ChildId)>) -> Vec<u8> {
        let owns_frame = matches!(self.placement.mode, ChildMode::Active | ChildMode::Live)
            && !frame_revoked.contains(&(self.placer, self.view, self.child))
            && self.hosted().is_some();
        if !owns_frame {
            return Vec::new();
        }
        let owner = chrome_owner(&self.editor);
        let Some((_, editor)) = owner.hosted() else {
            return Vec::new();
        };
        let items = POOL.with_borrow(|pool| pool.menus.get(&(editor, menu_view(&owner))).cloned());
        match items {
            Some(items) if !items.is_empty() => be_wasm_ops::encode(&items),
            _ => Vec::new(),
        }
    }

    /// Whether it takes the input where it is placed: placed to, and not taken back (`revoked`).
    fn takes_input(&self, revoked: &HashSet<(EditorInstanceId, ViewId, ChildId)>) -> bool {
        takes_input(self.placement.mode) && !revoked.contains(&(self.placer, self.view, self.child))
    }
}

/// What names the editor a place keeps (`Child::key`): its place, and what it shows.
type ChildKey = (EditorInstanceId, ViewId, ChildId, Vec<u8>);

/// How the editor of a child placed is (`Children::child_of`).
enum PlacedEditor {
    /// Open in its app.
    Open(Embedded, EditorInstanceId),
    /// Being opened.
    Opening,
    /// Not placed, or it failed: it shows nothing.
    Gone,
}

/// What a `settle` makes of the children shown and those placed now (`reconcile`).
struct Reconciled {
    /// The children placed now, in their order, each keeping the editor it had if it showed
    /// the same before, else with an editor of its own to open.
    kept: Vec<Child>,
    /// The editors to open, of the children of `kept` (by index).
    opened: Vec<usize>,
    /// Those shown before and placed no longer, to let go of.
    gone: Vec<Child>,
}

/// Matches the children `wanted` now with those `shown` before by what names their editor
/// (`Child::key`): one that matches keeps its editor and its own children at its new place. One
/// that does not gets an editor of its own, opened, unless its block is open `above` it, which it
/// shows instead. The rest go. An editor shown as its block's label, placed now to show its main
/// view, is opened again.
fn reconcile(shown: Vec<Child>, wanted: Vec<Child>, above: &[[u8; 16]]) -> Reconciled {
    let mut before: HashMap<ChildKey, Child> = shown
        .into_iter()
        .map(|child| (child.key(), child))
        .collect();
    let mut kept = Vec::with_capacity(wanted.len());
    let mut opened = Vec::new();
    for mut child in wanted {
        match before.remove(&child.key()) {
            Some(shown) => {
                child.editor = shown.editor;
                let relabelled =
                    matches!(&*child.editor.showing.borrow(), Showing::Label) && !child.previewed();
                if relabelled {
                    *child.editor.showing.borrow_mut() = Showing::Opening;
                    opened.push(kept.len());
                }
            }
            None if child.block.is_some_and(|block| above.contains(&block)) => {
                child.editor = Editor::new(
                    child.shown.clone(),
                    Showing::Failed(ALREADY_OPEN_ABOVE.to_owned()),
                );
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

/// The editor drawing the chrome of the frame `editor` is shown in, as be-host's `chrome_owner`
/// says: the child taking `editor`'s frame over, the child taking that one's in turn, and so
/// on, else `editor` itself.
fn chrome_owner(editor: &Rc<Editor>) -> Rc<Editor> {
    let mut owner = Rc::clone(editor);
    // Each level is another editor's: no deeper than the children shown.
    for _ in 0..64 {
        let next = owner
            .nested
            .borrow()
            .shown
            .iter()
            .find(|child| child.framing && child.hosted().is_some())
            .map(|child| Rc::clone(&child.editor));
        match next {
            Some(next) if !Rc::ptr_eq(&next, &owner) => owner = next,
            _ => break,
        }
    }
    owner
}

/// The view of `editor` whose menu its frame shows and a pick from it names, as be-host-runtime's
/// `content_view` says: its main view while that holds its content, else the view of its dock's
/// tab its content moved to, else its main view.
fn menu_view(editor: &Editor) -> ViewId {
    let Some((app, opened)) = editor.hosted() else {
        return ViewId::MAIN;
    };
    if app.content(main_screen(opened)).is_some() {
        return ViewId::MAIN;
    }
    editor
        .tabs
        .borrow()
        .iter()
        .find(|tab| app.content(tab.screen).is_some())
        .map_or(ViewId::MAIN, |tab| ViewId(tab.tab))
}

impl Children {
    /// Follows what `parent`'s editors placed now, at the frame clock's `now`: opens an editor
    /// for each child newly placed to run one, lets go of those no longer placed, asks each
    /// editor for its screens at its place, and tells the host how each view's children are once that changed. `above` are
    /// the blocks open in `parent` and the editors that place it, which none of its children may
    /// show again; its last is `parent`'s own. The children of the children shown are followed
    /// the same way. Then the docks of every editor shown hear what each app's said, and the
    /// root's host is handed what they have for each editor.
    pub fn settle(
        &mut self,
        parent: &Embedded,
        above: &[[u8; 16]],
        embedding: &mut Embedding,
        spawning: &Spawning,
        now: Duration,
    ) {
        let mut docks = std::mem::take(&mut self.docks);
        POOL.with_borrow_mut(|pool| pool.settled.clear());
        let mut settling = Settling {
            embedding,
            spawning,
            now,
            docks: &mut docks,
        };
        let block = above.last().map(|block| Uuid::from_bytes(*block));
        self.settle_in(parent, None, above, block, &mut settling, Vec::new(), true);
        // Each editor is asked for its screens once every level was followed. Its own children
        // were followed against the screens it was asked for before: one that changed them lays
        // out anew for them, and the placements of that layout bring the settle that follows its
        // children against them, so they lag no layout behind.
        let settled = POOL.with_borrow_mut(|pool| std::mem::take(&mut pool.settled));
        for editor in &settled {
            editor.show(settling.docks);
        }
        self.hear_told(parent, &mut docks);
        self.tell_all();
        self.hear_docks(parent, &mut docks);
        self.docks = docks;
        sweep_apps(embedding);
    }

    /// `settle` for one level, the children `parent`'s editor `only` places (every editor's of
    /// it with none): `asks` are the bodies of joined docks an editor above placed whose paths
    /// lead through these children, `block` the block `parent`'s editor edits (none for a
    /// dialog's, which has no dock, as be-host registers none for it), and `on_screen` whether
    /// `parent` shows any view.
    #[allow(clippy::too_many_arguments)]
    fn settle_in(
        &mut self,
        parent: &Embedded,
        only: Option<EditorInstanceId>,
        above: &[[u8; 16]],
        block: Option<Uuid>,
        settling: &mut Settling<'_>,
        asks: Vec<TabAsk>,
        on_screen: bool,
    ) {
        let ours = |editor: EditorInstanceId| only.is_none_or(|only| only == editor);
        let mut screens = parent.screens();
        screens.retain(|screen| ours(screen.instance));
        self.instance = parent.instance();
        self.placers = screens
            .iter()
            .map(|screen| (parent.layer_id(screen.screen), screen.instance, screen.view))
            .collect();
        let app = app_key(self.instance);
        let mut editors: Vec<EditorInstanceId> = Vec::new();
        // An editor shown only as a preview has no dock, as under be-host, which keeps previews
        // out of its docks: another editor of its block may be docked.
        for screen in screens
            .iter()
            .filter(|screen| screen.view != ViewId::PREVIEW)
        {
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
        // The views its editors anchored are placers too: children are placed in them.
        self.requests = screens
            .into_iter()
            .chain(
                parent
                    .anchored()
                    .into_iter()
                    .filter(|view| ours(view.editor))
                    .map(|view| view.request),
            )
            .collect();
        let mut placed = parent.placed();
        placed.retain(|placements| ours(placements.instance));
        let sheets: Vec<(EditorInstanceId, ViewId)> = parent
            .anchored()
            .into_iter()
            .filter(|view| matches!(view.placement.anchor, ViewAnchor::Edge(_)))
            .map(|view| (view.editor, view.view))
            .collect();
        let mut wanted = Vec::new();
        let mut asking: HashMap<EditorInstanceId, (u64, ChildId)> = HashMap::default();
        let mut docked = docked_children(&placed);
        self.bodies.clear();
        self.unshown.clear();
        for placements in &placed {
            let placer = self.requests.iter().find(|screen| {
                (screen.instance, screen.view) == (placements.instance, placements.view)
            });
            let raster = placer.map_or(1.0, |screen| screen.metrics.raster);
            let space = placer.and_then(|screen| screen.anchor_space);
            for (index, placement) in placements.children.iter().enumerate() {
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
                            space,
                        });
                    }
                    continue;
                }
                // A web view is its editor's own, which says in its place that it fails, as
                // under be-host (be-host-runtime's `drive_web_views`, through the host's
                // `Forwarded`): the root shows nothing of it and says nothing of it.
                if matches!(placement.content, ChildContent::WebView(_)) {
                    continue;
                }
                let occluders = occluders_of(placements, index);
                match Child::placed(
                    placements.instance,
                    placements.view,
                    raster,
                    in_view(placement, placements.view),
                    occluders,
                ) {
                    // A dialog placed in a view anchored to an edge of its owner's opens as a
                    // sheet, as under be-host.
                    Some(child) if sheets.contains(&(placements.instance, placements.view)) => {
                        wanted.push(child.in_sheet())
                    }
                    Some(child) => wanted.push(child),
                    None => self.unshown.push((
                        (placements.instance, placements.view),
                        ShownChild {
                            child: child.0,
                            shows: None,
                            shown_in: None,
                            active: false,
                            error: Some(NOT_SHOWN_YET.to_owned()),
                            preview: ShownPreview::None,
                            intrinsic: None,
                            menu: Vec::new(),
                        },
                    )),
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
            release(&gone, settling.embedding, Some(&mut *settling.docks));
        }
        self.shown = kept;
        self.forget_revoked();
        let mut asks: Vec<TabAsk> = asks.into_iter().chain(self.bodies.clone()).collect();
        let mut nested = above.to_vec();
        for child in &mut self.shown {
            child.framing = child.placed
                && taking.iter().any(|(editor, view, id, _)| {
                    (*editor, *view, *id) == (child.placer, child.view, child.child)
                });
            let editor = Rc::clone(&child.editor);
            editor.tab_asks.borrow_mut().clear();
            POOL.with_borrow_mut(|pool| pool.settled.push(Rc::clone(&editor)));
            let failed = match &*editor.showing.borrow() {
                Showing::Shown { hosted, .. } => hosted.app.failure(),
                Showing::Opening | Showing::Label | Showing::Failed(_) => None,
            };
            if let Some(why) = failed {
                close(&editor, settling.embedding, Some(&mut *settling.docks));
                *editor.showing.borrow_mut() = Showing::Failed(why);
            }
            if let Showing::Shown { hosted, embedded } = &mut *editor.showing.borrow_mut()
                && !*embedded
            {
                embed_app(&hosted.app, settling.embedding);
                *embedded = true;
            }
            *editor.asks.borrow_mut() = child.placed.then(|| {
                let placer = self
                    .requests
                    .iter()
                    .find(|screen| (screen.instance, screen.view) == (child.placer, child.view));
                PlaceAsk {
                    placement: child.placement.clone(),
                    raster: child.raster,
                    framing: placer.filter(|_| child.framing).cloned(),
                    space: placer.and_then(|screen| screen.anchor_space),
                }
            });
            // The bodies of its dock's views a joined dock places, and those of its own
            // children's, whose paths lead on through it.
            let (mine, rest): (Vec<TabAsk>, Vec<TabAsk>) =
                std::mem::take(&mut asks).into_iter().partition(|ask| {
                    ask.editor == child.placer && ask.via.first() == Some(&child.child)
                });
            asks = rest;
            let hosted = child.hosted();
            let mut deeper = Vec::new();
            if let Some((_, inner)) = &hosted {
                for ask in mine {
                    match ask.via.len() {
                        1 => editor.tab_asks.borrow_mut().push(ask),
                        _ => deeper.push(TabAsk {
                            editor: *inner,
                            via: ask.via[1..].to_vec(),
                            ..ask
                        }),
                    }
                }
            }
            if let Some((app, inner)) = hosted {
                nested.extend(child.block);
                let shows = child.placed
                    || !editor.tabs.borrow().is_empty()
                    || editor.main.borrow().is_some();
                editor.nested.borrow_mut().settle_in(
                    &app,
                    Some(inner),
                    &nested,
                    child.block.map(Uuid::from_bytes),
                    settling,
                    deeper,
                    shows,
                );
                if child.block.is_some() {
                    nested.pop();
                }
            }
        }
    }

    /// Tells the host how each view's children are, once every editor shown was asked for its
    /// screens (`Editor::show`), which says which place shows each, and the same of the children
    /// of each editor shown.
    fn tell_all(&mut self) {
        for child in &mut self.shown {
            child.layer = child.shown_layer();
            child.up = child.hosted().is_some_and(|(app, _)| app.started());
        }
        let mut views: HashMap<(EditorInstanceId, ViewId), Vec<ShownChild>> = HashMap::default();
        for (view, shown) in &self.unshown {
            views.entry(*view).or_default().push(shown.clone());
        }
        for child in &self.shown {
            if !child.placed {
                continue;
            }
            let hosted = child.hosted();
            views
                .entry((child.placer, child.view))
                .or_default()
                .push(ShownChild {
                    child: child.child.0,
                    shows: Some(child.shown.clone()),
                    shown_in: hosted.as_ref().map(|(app, editor)| OpenedIn {
                        instance: app.instance(),
                        editor: editor.0,
                    }),
                    active: child.takes_input(&self.revoked),
                    error: match &*child.editor.showing.borrow() {
                        Showing::Failed(why) => Some(why.clone()),
                        Showing::Opening | Showing::Label | Showing::Shown { .. } => None,
                    },
                    preview: child.shown_preview(),
                    intrinsic: child.intrinsic(),
                    menu: child.frame_menu(&self.frame_revoked),
                });
        }
        // The bodies of joined docks its own editors place: available once the editor owning
        // each shows it.
        for ask in &self.bodies {
            let owner = self.owner(ask.editor, &ask.via);
            let shown_in = owner.and_then(|owner| {
                let (app, editor) = owner.hosted()?;
                owner
                    .tabs
                    .borrow()
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
                    shows: None,
                    shown_in,
                    active: true,
                    error: None,
                    preview: ShownPreview::None,
                    intrinsic: None,
                    menu: Vec::new(),
                });
        }
        self.tell(views);
        for child in &self.shown {
            if child.hosted().is_some() {
                child.editor.nested.borrow_mut().tell_all();
            }
        }
    }

    /// Hears what every app shown said of its docks, once what each placed is in (`settle`),
    /// and hands the root's host what the docks have for each editor (`host.dock`), with the
    /// picks from a joined dock's tab menus and the asks for docked children's details, each
    /// for the editor it names (`host.tell-editor`).
    fn hear_docks(&self, parent: &Embedded, docks: &mut Docks) {
        let apps = self.every_app(parent);
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
            // A pick from a joined dock's tab menu goes to the editor whose menu it is, and an
            // ask for a docked child's details to its placer, as be-host-runtime sends them.
            for (owner, view, id, anchor) in docks.take_picks(&key) {
                let pick = MenuChoice { view, id, anchor };
                tell_editor(app.instance(), owner, &EditorEvent::MenuPick(pick));
            }
            for (placer, child) in docks.take_details(&key) {
                tell_editor(app.instance(), placer, &EditorEvent::ChildDetails(child));
            }
        }
        // be-host-runtime marks the apps `take_changed` names for a frame. Here every app's
        // updates were just taken and handed to the host, whose relayed answer to an editor's
        // dock watch wakes its app, and what the app lays out then reaches the root as any
        // layout does (`Embedded::wants`): there is nothing left to mark.
        let _ = docks.take_changed();
    }

    /// `parent` and the apps of every child shown, any number of levels deep, each once.
    fn every_app(&self, parent: &Embedded) -> Vec<Embedded> {
        let mut apps: Vec<Embedded> = vec![parent.clone()];
        for app in self.apps() {
            if !apps.iter().any(|held| held.instance() == app.instance()) {
                apps.push(app);
            }
        }
        apps
    }

    /// Acts on what the editors of `parent` and of every app shown told the shell around them
    /// (`Embedded::take_told`), as be-host does: a menu goes to the docks, which show it on a
    /// joined dock's tab, and to the statuses of the children whose frame its editor draws; an
    /// ask for its details goes to the docks hosting its dock, else to its placer as its child's
    /// (`child-details`); a pick from a child's menu goes to the editor drawing that child's
    /// frame (`menu-pick`); and an ask to leave the frame it takes gives the frame back to its
    /// placer, as Escape there does. The root draws no details of its own: an editor that asks
    /// with no placer hears nothing.
    fn hear_told(&mut self, parent: &Embedded, docks: &mut Docks) {
        // The apps no place visited (one being opened, or whose child went) are heard too: what
        // they told is taken, so none owes the root a frame for it.
        let mut apps = self.every_app(parent);
        let pooled: Vec<Embedded> = POOL.with_borrow(|pool| {
            pool.apps
                .values()
                .map(|pooled| pooled.app.clone())
                .collect()
        });
        for app in pooled {
            if !apps.iter().any(|held| held.instance() == app.instance()) {
                apps.push(app);
            }
        }
        for app in apps {
            for (editor, message) in app.take_told() {
                match message {
                    ShellMessage::Menu(MenuSet { view, items }) => {
                        docks.set_menu(editor, view, items.clone());
                        POOL.with_borrow_mut(|pool| {
                            match items.is_empty() {
                                true => pool.menus.remove(&(editor, view)),
                                false => pool.menus.insert((editor, view), items),
                            };
                        });
                    }
                    ShellMessage::Details => {
                        if !docks.details(editor)
                            && let Some((instance, placer, child)) = self.placer_of(editor)
                        {
                            tell_editor(instance, placer, &EditorEvent::ChildDetails(child));
                        }
                    }
                    ShellMessage::ChildMenuPick(choice) => {
                        self.pick_child_menu(editor, &choice);
                    }
                    ShellMessage::LeaveFrame => self.leave_frame(editor),
                    ShellMessage::CommitChild(creation) => self.commits.push((editor, creation)),
                    _ => {}
                }
            }
        }
        self.relay_commits();
    }

    /// Hands each creation a placer committed (`commit-child`) to the editor its child runs,
    /// through the host (`host.tell-shell`), which makes the block as be-host does, once that
    /// editor opened: one asked for before keeps until then, and goes with its child.
    fn relay_commits(&mut self) {
        let commits = std::mem::take(&mut self.commits);
        for (placer, creation) in commits {
            match self.child_of(placer, creation.child) {
                PlacedEditor::Open(app, editor) => {
                    app.tell_shell(editor, vec![ShellMessage::CommitChild(creation)]);
                }
                PlacedEditor::Opening => self.commits.push((placer, creation)),
                PlacedEditor::Gone => {}
            }
        }
    }

    /// How the editor of the child `child` that `placer` places is, any number of levels deep.
    fn child_of(&self, placer: EditorInstanceId, child: ChildId) -> PlacedEditor {
        if let Some(found) = self
            .shown
            .iter()
            .find(|shown| shown.placed && (shown.placer, shown.child) == (placer, child))
        {
            return match (found.hosted(), &*found.editor.showing.borrow()) {
                (Some((app, editor)), _) => PlacedEditor::Open(app, editor),
                (None, Showing::Opening) => PlacedEditor::Opening,
                (None, _) => PlacedEditor::Gone,
            };
        }
        self.shown
            .iter()
            .map(|shown| shown.editor.nested.borrow().child_of(placer, child))
            .find(|found| !matches!(found, PlacedEditor::Gone))
            .unwrap_or(PlacedEditor::Gone)
    }

    /// The place that shows the editor `editor`, any number of levels deep, as the handle of
    /// its placer's app, its placer and its id there.
    fn placer_of(&self, editor: EditorInstanceId) -> Option<(u64, EditorInstanceId, ChildId)> {
        let shows = |child: &&Child| {
            child.placed && child.hosted().is_some_and(|(_, shown)| shown == editor)
        };
        let place = self.shown.iter().find(shows);
        if let Some(child) = place {
            return Some((self.instance, child.placer, child.child));
        }
        self.shown
            .iter()
            .find_map(|child| child.editor.nested.borrow().placer_of(editor))
    }

    /// Hands a pick from the menu of `placer`'s child `choice.child` to the editor drawing that
    /// child's frame (`chrome_owner`), as be-host's `pick_frame_menu` does: the anchor only
    /// when that is the child's own editor. Whether some level held the child.
    fn pick_child_menu(&self, placer: EditorInstanceId, choice: &ChildMenuChoice) -> bool {
        let place = self
            .shown
            .iter()
            .find(|child| child.placer == placer && child.child == choice.child);
        if let Some(place) = place {
            let owner = chrome_owner(&place.editor);
            if let Some((app, editor)) = owner.hosted() {
                let anchor = choice.anchor.filter(|_| Rc::ptr_eq(&owner, &place.editor));
                let pick = MenuChoice {
                    view: menu_view(&owner),
                    id: choice.id.clone(),
                    anchor,
                };
                tell_editor(app.instance(), editor, &EditorEvent::MenuPick(pick));
            }
            return true;
        }
        self.shown
            .iter()
            .any(|child| child.editor.nested.borrow().pick_child_menu(placer, choice))
    }

    /// Gives back the frame the editor `editor` takes over wherever it does, as Escape in its
    /// placer does (`revoke`): from every child of that view that could take it, so it goes to
    /// none of its siblings.
    fn leave_frame(&mut self, editor: EditorInstanceId) {
        let left: Vec<(EditorInstanceId, ViewId)> = self
            .shown
            .iter()
            .filter(|child| {
                child.framing && child.hosted().is_some_and(|(_, shown)| shown == editor)
            })
            .map(|child| (child.placer, child.view))
            .collect();
        for (placer, view) in left {
            for child in &self.shown {
                if (child.placer, child.view) == (placer, view) && could_frame(&child.placement) {
                    self.frame_revoked
                        .insert((child.placer, child.view, child.child));
                }
            }
        }
        for child in &self.shown {
            child.editor.nested.borrow_mut().leave_frame(editor);
        }
    }

    /// The editor of the child the path `via` leads to from `editor`'s children, through each
    /// one's own: the editor owning a body of a joined dock.
    fn owner(&self, editor: EditorInstanceId, via: &[ChildId]) -> Option<Rc<Editor>> {
        let (first, rest) = via.split_first()?;
        let child = self
            .shown
            .iter()
            .find(|child| child.placer == editor && child.child == *first)?;
        if rest.is_empty() {
            return Some(Rc::clone(&child.editor));
        }
        let (_, inner) = child.hosted()?;
        child.editor.nested.borrow().owner(inner, rest)
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
                    (child.placer, child.view, child.child) == (*editor, *view, *id)
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

    /// Takes back active children: a pointer's press anywhere but in the child (in its placer's
    /// view or outside it, on the root's own surfaces too). An Escape the child leaves to its
    /// placer takes nothing back: the keyboard stays where it is. Each leaves the input to its
    /// placer, which hears it is no longer active with the next `settle`. A child taking its
    /// placer's frame over gives it back on such a press, or Escape in its placer's view, and
    /// so does every child of that view that could take it, as be-host's
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
                .find(|(_, editor, view)| (*editor, *view) == (child.placer, child.view))
                .map(|(layer, ..)| *layer);
            let outside = pressed
                .iter()
                .any(|press| !press.escape && !child.holds(press.layer));
            let escaped = pressed
                .iter()
                .any(|press| press.escape && Some(press.layer) == placer);
            if outside && active {
                self.revoked.insert((child.placer, child.view, child.child));
            }
            if (outside || escaped) && child.framing {
                frames_back.push((child.placer, child.view));
            }
        }
        for (editor, view) in frames_back {
            for child in &self.shown {
                if (child.placer, child.view) == (editor, view) && could_frame(&child.placement) {
                    self.frame_revoked
                        .insert((child.placer, child.view, child.child));
                }
            }
        }
        for child in &self.shown {
            child.editor.nested.borrow_mut().revoke(pressed);
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
            (child.placer, child.view, child.child) == (editor, view, id) && child.shows_frame()
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
                .find(|child| child.placer == ask.editor && child.child == ask.child)
            else {
                continue;
            };
            if matches!(&*child.editor.showing.borrow(), Showing::Failed(_)) {
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
        for child in &self.shown {
            let inner = child.editor.nested.borrow_mut().keyboard(focused, drawn);
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
                .any(|child| child.placer == editor && child.holds(layer))
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
                false => child.editor.nested.borrow().focused_into(layer),
            };
            if child.shows(layer) || !into.is_empty() {
                into.push((self.instance, child.placer, child.child));
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
                &*child.editor.showing.borrow(),
                Showing::Opening
                    | Showing::Shown {
                        embedded: false,
                        ..
                    }
            ) || child.editor.nested.borrow().opening()
        })
    }

    /// Lets go of every child, as when their placer goes: each one's editor closes, and each app
    /// once it runs no editor.
    pub fn release(&mut self, embedding: &mut Embedding) {
        self.release_in(embedding, None);
        self.docks = Docks::default();
        sweep_apps(embedding);
    }

    /// `release`, forgetting the docks of every editor let go of, when `docks` holds them.
    fn release_in(&mut self, embedding: &mut Embedding, mut docks: Option<&mut Docks>) {
        for child in std::mem::take(&mut self.shown) {
            release(&child, embedding, docks.as_deref_mut());
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
            let Some((app, editor)) = child.hosted() else {
                continue;
            };
            let screen = child.screen(editor);
            if let (Some(_), Some(layer)) = (app.layer(screen, pos2(0.0, 0.0)), child.layer) {
                screens.push((app.clone(), screen, layer));
            }
            for tab in child.editor.tabs.borrow().iter() {
                if let Some(layer) = app.layer(tab.screen, pos2(0.0, 0.0)) {
                    screens.push((app.clone(), tab.screen, layer.id));
                }
            }
            screens.extend(child.editor.nested.borrow().screens());
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

    /// The apps of every child shown, any number of levels deep, each once.
    pub fn apps(&self) -> Vec<Embedded> {
        let mut apps: Vec<Embedded> = Vec::new();
        for child in self.shown.iter() {
            let held = child
                .hosted()
                .map(|(app, _)| app)
                .into_iter()
                .chain(child.editor.nested.borrow().apps());
            for app in held {
                if !apps.iter().any(|known| known.instance() == app.instance()) {
                    apps.push(app);
                }
            }
        }
        apps
    }

    /// The layers showing the views `parent`'s editor `only` (every editor's of it with none)
    /// anchored outside their groups (menus, popovers, sheets), each with the children placed in
    /// it, and those the editors of its children anchored, any number of levels deep: over
    /// everything placed, in the order they opened across every app, as be-host stacks them.
    pub fn anchored_layers(
        &self,
        parent: &Embedded,
        only: Option<EditorInstanceId>,
        layers: &mut Vec<Layer>,
    ) {
        let mut hung = Vec::new();
        self.hung_layers(parent, only, &mut hung);
        hung.sort_by_key(|(opened, _)| *opened);
        layers.extend(hung.into_iter().flat_map(|(_, layers)| layers));
    }

    /// `anchored_layers`, each view's layers with when it opened.
    fn hung_layers(
        &self,
        parent: &Embedded,
        only: Option<EditorInstanceId>,
        hung: &mut Vec<(u64, Vec<Layer>)>,
    ) {
        for view in parent.anchored() {
            if only.is_none_or(|only| only == view.editor) {
                let mut layers = Vec::new();
                self.layers(&view.request, view.layer, &mut layers);
                hung.push((view.opened, layers));
            }
        }
        for child in self.shown.iter() {
            if let Some((app, editor)) = child.hosted() {
                child
                    .editor
                    .nested
                    .borrow()
                    .hung_layers(&app, Some(editor), hung);
            }
        }
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
        // Where children that take input are, in the placer's pixels, each with the occluders
        // that hide it, where the placer is hit still.
        let mut holes: Vec<(Rect, Vec<Rect>)> = Vec::new();
        let placed_in = |editor: EditorInstanceId, view: ViewId| {
            (editor, view) == (screen.instance, screen.view)
        };
        for child in &self.shown {
            if !child.placed || !placed_in(child.placer, child.view) {
                continue;
            }
            let Some((app, editor)) = child.hosted() else {
                continue;
            };
            let Some(request) = child.request() else {
                continue;
            };
            let placement = &child.placement;
            let Some(mut shown_child) = app.layer(child.screen(editor), pos2(0.0, 0.0)) else {
                continue;
            };
            let framing = child.framing;
            // From its view's points to its placer's, and what of its placer's it may show: the
            // whole of its placer's view while it takes the frame, else its place, laid out at
            // its zoom and turned by its rotation about its middle, within its clip.
            let (to_placer, clip, opacity) = match framing {
                true => (
                    Affine::IDENTITY,
                    Rect::from_min_max(
                        pos2(0.0, 0.0),
                        pos2(screen.metrics.logical.x, screen.metrics.logical.y),
                    ),
                    1.0,
                ),
                false => {
                    let zoom = child.zoom().map_or(1.0, |(_, zoom)| zoom);
                    let scale = be_lib_geometry::vec2(zoom, zoom);
                    let to_placer = placed_transform(placement, scale);
                    let swept = to_placer.bounds(Rect::from_min_size(
                        pos2(0.0, 0.0),
                        be_lib_geometry::vec2(
                            placement.rect.width() / scale.x.max(f32::EPSILON),
                            placement.rect.height() / scale.y.max(f32::EPSILON),
                        ),
                    ));
                    (
                        to_placer,
                        placement.clip.intersect(swept),
                        placement.opacity,
                    )
                }
            };
            shown_child.transform = shown_child.transform.then(&to_placer).then(&to_window);
            let Some(from_window) = shown_child.transform.inverse() else {
                continue;
            };
            let clip = to_window.bounds(clip).intersect(shown);
            shown_child.clip = shown_child.clip.intersect(from_window.bounds(clip));
            shown_child.opacity = layer.opacity * opacity.clamp(0.0, 1.0);
            shown_child.input.placer = Some(layer.id);
            let active = child.takes_input(&self.revoked);
            // The occluders placed after it, in its pixels: presses there reach its placer.
            let occluded: Vec<Rect> = child
                .occluders
                .iter()
                .map(|occluder| from_window.bounds(to_window.bounds(*occluder)))
                .collect();
            shown_child.input.hit = match (framing, active) {
                (true, _) => framing_hit(
                    shown_child.size,
                    pixels,
                    app.content(main_screen(editor)),
                    placement.rect.intersect(placement.clip),
                ),
                (false, true) => without(shown_child.size, &occluded),
                (false, false) => Area::Nowhere,
            };
            if active && !framing && placement.layer == ChildLayer::Below {
                // A child placed to fit takes the presses only where it took room; a turned
                // one, the upright bounds of its place.
                let room = child.fitted().map_or(placement.rect.size(), |room| {
                    be_lib_geometry::vec2(room.width, room.height)
                });
                let swept = placed_transform(placement, be_lib_geometry::vec2(1.0, 1.0))
                    .bounds(Rect::from_min_size(pos2(0.0, 0.0), room));
                let hole = hole(
                    &ChildPlacement {
                        rect: swept,
                        ..placement.clone()
                    },
                    pixels,
                    layer.clip,
                );
                let occluders = child
                    .occluders
                    .iter()
                    .map(|occluder| scaled(*occluder, pixels))
                    .collect();
                holes.push((hole, occluders));
            }
            let into = match (framing, placement.layer) {
                (false, ChildLayer::Below) => &mut below,
                (true, _) | (false, ChildLayer::Above) => &mut over,
            };
            child
                .editor
                .nested
                .borrow()
                .layers(&request, shown_child, into);
        }
        for ask in &self.bodies {
            if !placed_in(ask.editor, ask.view) {
                continue;
            }
            let Some(owner) = self.owner(ask.editor, &ask.via) else {
                continue;
            };
            let Some((app, editor)) = owner.hosted() else {
                continue;
            };
            let Some(screen) = owner
                .tabs
                .borrow()
                .iter()
                .find(|tab| tab.tab == ask.tab)
                .map(|tab| tab.screen)
            else {
                continue;
            };
            let placement = &ask.placement;
            let at = to_window.bounds(placement.rect);
            let clip = to_window
                .bounds(placement.clip.intersect(placement.rect))
                .intersect(shown);
            let Some(mut body) = app.layer(screen, at.min) else {
                continue;
            };
            let Some(from_window) = body.transform.inverse() else {
                continue;
            };
            body.clip = body.clip.intersect(from_window.bounds(clip));
            body.input.placer = Some(layer.id);
            if placement.layer == ChildLayer::Below {
                holes.push((hole(placement, pixels, layer.clip), Vec::new()));
            }
            let request = tab_screen(screen, editor, ask, false);
            let into = match placement.layer {
                ChildLayer::Below => &mut below,
                ChildLayer::Above => &mut over,
            };
            owner.nested.borrow().layers(&request, body, into);
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
/// that take input are, each with the occluders placed after its child, which hide it: as
/// be-host-runtime's holes say, everywhere but where some hole is and none of its own occluders
/// are.
fn placer_hit(size: (u32, u32), holes: Vec<(Rect, Vec<Rect>)>) -> Area {
    if holes.is_empty() {
        return Area::Whole;
    }
    let whole = Rect::from_min_max(pos2(0.0, 0.0), pos2(size.0 as f32, size.1 as f32));
    let cut: Vec<Rect> = holes
        .into_iter()
        .flat_map(|(hole, occluders)| hole.subtract(&occluders))
        .collect();
    Area::Region(whole.subtract(&cut))
}

/// The occluders of `placements` that hide its child `index`: those placed after it, as
/// be-host-runtime's `Instances` says, in the view's points.
fn occluders_of(placements: &ChildPlacements, index: usize) -> Vec<Rect> {
    placements
        .occluders
        .iter()
        .filter(|occluder| occluder.after as usize > index)
        .map(|occluder| occluder.rect)
        .collect()
}

/// Where a child's layer `size` pixels big is hit with `occluded` (in its pixels) hidden by its
/// placer's occluders: everywhere else.
fn without(size: (u32, u32), occluded: &[Rect]) -> Area {
    if occluded.is_empty() {
        return Area::Whole;
    }
    let whole = Rect::from_min_max(pos2(0.0, 0.0), pos2(size.0 as f32, size.1 as f32));
    Area::Region(whole.subtract(occluded))
}

/// From the points of a child's view to its placer's: at its place as `placement` says, scaled
/// by `scale` (its zoom) and turned by its rotation (radians, clockwise on screen) about
/// the middle of its place, as be-host draws a placed child.
fn placed_transform(placement: &ChildPlacement, scale: be_lib_geometry::Vec2) -> Affine {
    let (sin, cos) = match placement.rotation.is_finite() {
        true => placement.rotation.sin_cos(),
        false => (0.0, 1.0),
    };
    let middle = placement.rect.size() / 2.0;
    let turned = be_lib_geometry::vec2(
        cos * middle.x - sin * middle.y,
        sin * middle.x + cos * middle.y,
    );
    Affine::from_axes(
        be_lib_geometry::vec2(cos * scale.x, sin * scale.x),
        be_lib_geometry::vec2(-sin * scale.y, cos * scale.y),
        placement.rect.min + middle - turned,
    )
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
/// Drawn at `zoom` (`Child::zoom`), it is laid out at its place's size over that zoom, painted at
/// its placer's pixels all the same, as be-host lays out a child placed with an intrinsic size.
/// Placed to fit its content (`fit`), it is asked for at most that room, and takes what it needs.
pub fn child_screen(
    placement: &ChildPlacement,
    editor: EditorInstanceId,
    raster: f32,
    zoom: f32,
) -> ScreenRequest {
    let fit = placement
        .fit
        .map(|fit| be_lib_geometry::vec2(fit.x, fit.y))
        .filter(|fit| fit.x > 0.0 && fit.y > 0.0);
    let size = fit.unwrap_or(placement.rect.size()) / zoom;
    let raster = raster * zoom;
    let origin = placement.rect.min.to_vec2();
    ScreenRequest {
        screen: main_screen(editor),
        instance: editor,
        view: ViewId::MAIN,
        placement: ViewPlacement::Parent(ParentView::Main),
        frame: placement.own_frame.then(|| FrameSpec {
            chrome: FrameChrome::Drawn,
            top_bar: placement.top_bar,
            dialog: placement.dialog.clone(),
            anchor: placement
                .anchor
                .map(|anchor| scaled(anchor.translate(-origin), 1.0 / zoom)),
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
        fit: fit.is_some(),
        origin: None,
    }
}

/// The size a child placed as `placement` is laid out at and the zoom it is drawn at
/// (`Child::zoom`), placed with an intrinsic size and an editor that does not pan and zoom
/// (`pans`): its place's size over the zoom that fits its intrinsic size in it.
fn placed_zoom(placement: &ChildPlacement, pans: bool) -> Option<(be_lib_geometry::Vec2, f32)> {
    let size = placement.intrinsic.filter(|size| {
        !pans && size.x > 0.0 && size.y > 0.0 && size.x.is_finite() && size.y.is_finite()
    })?;
    let rect = placement.rect;
    let zoom = (rect.width() / size.x).min(rect.height() / size.y);
    (zoom.is_finite() && zoom > 0.0).then_some((rect.size() / zoom, zoom))
}

/// Where a child placed as `placement` in a view that may anchor views in `space`, laid out at
/// `zoom`, may anchor views of its own: its placer's, moved to its place and scaled by its zoom,
/// within its placer's bounds, as be-host's views hand theirs on. None where its placer may
/// anchor none (a preview).
fn placed_space(
    space: Option<AnchorSpace>,
    placement: &ChildPlacement,
    zoom: f32,
) -> Option<AnchorSpace> {
    let space = space?;
    let to_host = space.to_host;
    Some(AnchorSpace {
        to_host: HostTransform {
            origin: ViewPos2 {
                x: to_host.origin.x + placement.rect.min.x * to_host.scale.x,
                y: to_host.origin.y + placement.rect.min.y * to_host.scale.y,
            },
            scale: ViewVec2 {
                x: to_host.scale.x * zoom,
                y: to_host.scale.y * zoom,
            },
        },
        bounds: space.bounds,
    })
}

/// `rect` with its corners scaled by `by` about the origin.
fn scaled(rect: Rect, by: f32) -> Rect {
    Rect::from_min_max(
        pos2(rect.min.x * by, rect.min.y * by),
        pos2(rect.max.x * by, rect.max.y * by),
    )
}

/// Where a child placed as `placement` shows its content in its main view's points: its placed
/// rect, what shows of it, while it takes its placer's frame (`framing`), whose screen is its
/// placer's, else the whole of its view, the size of its place.
fn placed_content(placement: &ChildPlacement, framing: bool) -> Rect {
    match framing {
        true => placement.rect.intersect(placement.clip),
        false => Rect::from_min_max(
            pos2(0.0, 0.0),
            pos2(placement.rect.width(), placement.rect.height()),
        ),
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
    // A preview is drawn the size of its place, never sized to its content.
    let placement = &ChildPlacement {
        fit: None,
        ..placement.clone()
    };
    let size = placement.rect.size();
    let longest = size.x.max(size.y) * raster;
    let raster = match longest > PREVIEW_PIXELS {
        true => raster * PREVIEW_PIXELS / longest,
        false => raster,
    };
    ScreenRequest {
        screen: preview_screen_of(editor),
        view: ViewId::PREVIEW,
        placement: ViewPlacement::Parent(ParentView::Preview),
        frame: None,
        ..child_screen(placement, editor, raster, 1.0)
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
        screen: main_screen(editor),
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
        // It is its placer's size where its placer is: it anchors views where its placer would.
        anchor_space: placer.anchor_space,
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
    let mut request = child_screen(&ask.placement, editor, ask.raster, 1.0);
    request.anchor_space = placed_space(ask.space, &ask.placement, 1.0);
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

/// Opens the editor `child` shows, at the frame clock's `now`, in the app running the staged
/// app that edits its block, spawned if none does yet (`reserve_app`): it shows once the host
/// opened it and its app runs.
fn spawn(child: &Child, spawning: &Spawning, now: Duration) {
    let editor = Rc::downgrade(&child.editor);
    let shown = child.editor.shown.clone();
    let preview = child.previewed();
    let with = SpawnWith {
        surface: spawning.surface.clone(),
        fonts: Rc::clone(&spawning.fonts),
        now,
    };
    let wake = Rc::clone(&spawning.wake);
    be_wasm_ops::guest::spawn(async move {
        let opened = open(shown, preview, &with).await;
        let Some(editor) = editor.upgrade() else {
            // No place shows it any more: it closes as it opened.
            if let Ok(hosted) = opened {
                hosted.app.close_editor(hosted.editor);
                release_app(&hosted.app);
                // The app it was opened in may hold no editor now: the next settle lets it go.
                wake();
            }
            return;
        };
        *editor.showing.borrow_mut() = match opened {
            Ok(hosted) => Showing::Shown {
                hosted,
                embedded: false,
            },
            Err(Unopened::Label) => Showing::Label,
            Err(Unopened::Failed(why)) => Showing::Failed(why),
        };
        wake();
    });
}

/// Why `open` opened no editor.
enum Unopened {
    /// Asked for a preview, its editor draws none: nothing failed, and its placer draws the
    /// block's label in its place.
    Label,
    Failed(String),
}

/// Opens `shown`'s editor, for a preview only when `preview`, in the app the root's host names
/// for it (`host.editor-of`): the one the root runs already, or one spawned for it.
async fn open(shown: Shown, preview: bool, with: &SpawnWith) -> Result<Hosted, Unopened> {
    let editor = host::editor_of(shown.clone())
        .await
        .map_err(|error| format!("the host did not answer which app edits the block: {error:?}"))
        .and_then(|editor| editor)
        .map_err(Unopened::Failed)?;
    if preview && !editor.preview {
        return Err(Unopened::Label);
    }
    let app = reserve_app(editor.app, editor.module, with);
    match app.open_editor(shown).await {
        Ok(opened) => Ok(Hosted {
            app,
            editor: opened,
            name: editor.name,
            preview: editor.preview,
            pan_and_zoom: editor.pan_and_zoom,
            shows: editor.editor,
        }),
        Err(why) => {
            release_app(&app);
            Err(Unopened::Failed(format!(
                "{} could not start: {why}",
                editor.name
            )))
        }
    }
}

/// Lets go of the place `child`: its editor closes (`close`).
fn release(child: &Child, embedding: &mut Embedding, docks: Option<&mut Docks>) {
    close(&child.editor, embedding, docks);
}

/// Closes `editor` in its app, which stops once it runs no editor, lets go of its own children
/// as embedded, and forgets its docks in `docks`.
fn close(editor: &Editor, embedding: &mut Embedding, mut docks: Option<&mut Docks>) {
    if let Some((app, opened)) = editor.hosted() {
        if let Some(docks) = docks.as_deref_mut() {
            docks.forget(opened);
        }
        app.close_editor(opened);
        release_app(&app);
        POOL.with_borrow_mut(|pool| pool.menus.retain(|(held, _), _| *held != opened));
    }
    editor.tabs.borrow_mut().clear();
    editor.nested.borrow_mut().release_in(embedding, docks);
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
