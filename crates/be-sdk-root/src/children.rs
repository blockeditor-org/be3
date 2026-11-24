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
//! Left for later (4g7b3's later parts): giving a child the keyboard (`focus-child`) and the
//! gifts of keys held for it, promoting and taking back an active child, a child that takes its
//! placer's frame, docks that join across apps (a child is asked for its own dock), drags, a block
//! shown in several places sharing one editor, occluders, a child sized to its content (`fit`), one placed in place at
//! its zoom, previews, and every other kind of child (web views, host panels, program windows,
//! pickers, creations, artifact settings, rename forms and dock tab bodies), which the placer
//! hears are unavailable.

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
use be_lib_stable_hash::HashMap;
use be_sdk_api::{
    ChildContent, ChildId, ChildLayer, ChildMode, ChildPlacement, EditorInstanceId, Fonts,
    FrameChrome, FrameSpec, ParentView, Pos2 as ViewPos2, ScreenId, ScreenRequest, SurfaceSpec,
    Vec2 as ViewVec2, ViewId, ViewPlacement, ViewportMetrics,
};
use be_wasm_root_api::{OpenedIn, ShownBlock, ShownChild, guest::host};

use crate::{BlockSpawn, Embedded, Embedding, Hosted, SpawnFailure};

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
    shown: Vec<Child>,
    /// What the host was told last of each view's children (`host.show-children`).
    told: HashMap<(EditorInstanceId, ViewId), Vec<ShownChild>>,
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
}

/// How far a child's editor got.
enum Showing {
    /// The host has yet to name and open its editor, or its app to say hello.
    Opening,
    Shown {
        hosted: Hosted,
        /// The children its own editor places.
        children: Children,
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
/// (`Child::key`): one that matches keeps its editor at its new place, one that does not is
/// opened, unless its block is open `above` it, which it shows instead, and the rest go.
fn reconcile(shown: Vec<Child>, wanted: Vec<Child>, above: &[[u8; 16]]) -> Reconciled {
    let mut before: HashMap<ChildKey, Child> = shown
        .into_iter()
        .map(|child| (child.key(), child))
        .collect();
    let mut kept = Vec::with_capacity(wanted.len());
    let mut opened = Vec::new();
    for mut child in wanted {
        match before.remove(&child.key()) {
            Some(shown) => child.showing = shown.showing,
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
        let mut wanted = Vec::new();
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
        let Reconciled { kept, opened, gone } =
            reconcile(std::mem::take(&mut self.shown), wanted, above);
        for child in &opened {
            spawn(&kept[*child], spawning, now);
        }
        for gone in gone {
            release(&gone.showing.borrow(), embedding);
        }
        self.shown = kept;
        let mut nested = above.to_vec();
        for child in &self.shown {
            let mut showing = child.showing.borrow_mut();
            if let Showing::Shown { hosted, .. } = &*showing
                && let Some(why) = hosted.app.failure()
            {
                release(&showing, embedding);
                *showing = Showing::Failed(why);
            }
            let told = views.entry((child.editor, child.view)).or_default();
            told.push(ShownChild {
                child: child.child.0,
                shown_in: match &*showing {
                    Showing::Shown { hosted, .. } => Some(OpenedIn {
                        instance: hosted.app.instance(),
                        editor: hosted.editor.0,
                    }),
                    Showing::Opening | Showing::Failed(_) => None,
                },
                active: takes_input(child.placement.mode),
                error: match &*showing {
                    Showing::Failed(why) => Some(why.clone()),
                    Showing::Opening | Showing::Shown { .. } => None,
                },
            });
            if let Showing::Shown {
                hosted,
                children,
                embedded,
            } = &mut *showing
            {
                if !*embedded {
                    embedding.embed(hosted.app.clone());
                    *embedded = true;
                }
                hosted.app.show(vec![child_screen(
                    &child.placement,
                    hosted.editor,
                    child.raster,
                )]);
                nested.push(child.block);
                children.settle(&hosted.app, &nested, embedding, spawning, now);
                nested.pop();
            }
        }
        self.tell(parent.instance(), views);
    }

    /// Tells the host how each view's children are where that changed since it was told last,
    /// and that a view it was told of placed none once it places none.
    fn tell(
        &mut self,
        instance: u64,
        mut views: HashMap<(EditorInstanceId, ViewId), Vec<ShownChild>>,
    ) {
        for view in self.told.keys() {
            views.entry(*view).or_default();
        }
        for ((editor, view), children) in views {
            if self.told.get(&(editor, view)) == Some(&children) {
                continue;
            }
            let told = host::show_children(instance, editor.0, view.0, children.clone());
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

    /// Whether a child's editor is still being opened, any number of levels deep: its host has
    /// yet to open it, or the root to embed its app, so the root owes its host the frame that shows it (a host waiting for its
    /// root to settle waits for it). Once its app runs, the app's own pacing says what it owes.
    pub fn opening(&self) -> bool {
        self.shown
            .iter()
            .any(|child| match &*child.showing.borrow() {
                Showing::Opening
                | Showing::Shown {
                    embedded: false, ..
                } => true,
                Showing::Shown { children, .. } => children.opening(),
                Showing::Failed(_) => false,
            })
    }

    /// Lets go of every child, as when their placer goes: each child's app stops once nothing
    /// else holds it.
    pub fn release(&mut self, embedding: &mut Embedding) {
        for child in std::mem::take(&mut self.shown) {
            release(&child.showing.borrow(), embedding);
        }
        self.told.clear();
    }

    /// The apps of every child shown, any number of levels deep, each with the id of the layer
    /// showing its screen once it painted it: what the root's paintings carry.
    pub fn screens(&self) -> Vec<(Embedded, LayerId)> {
        let mut screens = Vec::new();
        for child in &self.shown {
            if let Showing::Shown {
                hosted, children, ..
            } = &*child.showing.borrow()
            {
                if let Some(layer) = hosted.app.layer(CHILD_SCREEN, pos2(0.0, 0.0)) {
                    screens.push((hosted.app.clone(), layer.id));
                }
                screens.extend(children.screens());
            }
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
            if let Showing::Shown {
                hosted, children, ..
            } = &*child.showing.borrow()
            {
                apps.push(hosted.app.clone());
                apps.extend(children.apps());
            }
        }
        apps
    }

    /// The layers showing `screen` of the app these are the children of, with its children:
    /// those placed below it, then `layer`, the placer's own, hit everywhere but where a child
    /// that takes input is, then those placed above it. Each child is clipped to what its
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
            let Showing::Shown {
                hosted, children, ..
            } = &*child.showing.borrow()
            else {
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
            let active = takes_input(placement.mode);
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
            children.layers(&child_screen, shown_child, into);
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
                children: Children::default(),
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

/// Lets go of a child's app and of those of its own children, as embedded.
fn release(showing: &Showing, embedding: &mut Embedding) {
    if let Showing::Shown {
        hosted, children, ..
    } = showing
    {
        embedding.remove(hosted.app.instance());
        for child in &children.shown {
            release(&child.showing.borrow(), embedding);
        }
    }
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
