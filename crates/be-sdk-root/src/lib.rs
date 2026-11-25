//! What a root guest links to embed apps (plans/host_as_platform_adapter.md, The root guest):
//! the host's half of the app protocol (be-sdk-app-host) run inside the root, over the channel
//! of each app it spawns with the sandbox (`be3:sandbox`), with no GUI framework.
//!
//! - `Embedded` is one app the root spawned: a `Wire` over the instance's channel and a `Run`
//!   with its session, pacing and holds. It opens the editor and its screens once the app says
//!   hello, asks it for a frame when its pacing says one is due and waits for the present, and
//!   names each screen it painted by handle as a layer (`Embedded::layer`), whose pixels stay on
//!   the host's device.
//! - `Windows` routes each window's raw input with be-lib-input-router's `Router` over the
//!   layers the root drew last, as the host's own UI routes today.
//! - `OwnSurface` is a surface of the root's own, which it draws on the host's device as an app
//!   draws its screens and names in its layer list as `Surface::Root`, below, between or above
//!   the screens it embeds.
//! - `Embedded::spawn_block` embeds a block's editor its host opens and serves until step 5
//!   (decisions/a_roots_embedded_editors_are_served_by_its_host_until_step_5.md): `be3:root`'s
//!   `host.editor-of` names the app, `host.open-editor` opens the editor, and everything the app
//!   sends under it the root forwards (`host.forward`) and relays what the host answers
//!   (`host.forwarded`), as be-wasm-ops frames it never reads.
//! - `Children` shows the block children the editors of an embedded app place: each is its
//!   block's editor, opened by the host in the one app running its staged app
//!   (`Embedded::open_editor`) and composited where it was placed, with its own children
//!   likewise; the host tells each placer how they are (`host.show-children`).
//! - `Embedding` holds the `Embedded`s and `Windows`: what is routed to an app's screen reaches
//!   the app as `AppCall::Input`, in the screen's points; what lands on the root's own surfaces
//!   the root gets back. What its apps ask of their windows (a cursor shape, the pointer lock,
//!   every sample) it answers as the window's (`Embedding::requests`). A block an editor drags
//!   out of itself (`drag-block`) the router of its window carries over the views under the
//!   pointer (`Windows::start_drag`), as be-host's drag board does, and each hears it through
//!   its host (`tell_editor`, `editor-event.drag`).
//! - The views an app's editors anchor outside their groups (menus, popovers, sheets), in the
//!   anchor space each screen is asked for with, are `Surface::Anchored` layers
//!   (`Embedded::anchored`) the root draws over what it places (`Children::anchored_layers`).
//!   One asking for the keyboard hands it to the child placed in it, else leaves it with the
//!   screen of the view it hangs from, which takes it back once it closes; a press closes what
//!   be-sdk-app-host's `anchored::dismissed_by_press` says (`input-batch.dismissed`), as be-host
//!   does.
//!
//! The clipboard is not relayed: an app it embeds copies and pastes only as the app protocol
//! lets it (a copy as a host service, which this does not serve yet, and a paste as the input
//! its host delivers), never through the root's own `be3:root` `host.copy` and `host.paste`,
//! which the host trusts the root alone with. A root never hands an app those calls or their
//! answers.
//!
//! The root stays single-threaded and never blocks: each app's frames are heard by a task on
//! the root's executor (`be_wasm_ops::guest::spawn`), and a frame awaits the present as ordinary
//! async code, so the root races it against its deadline (`be3:root`'s `host.deadline`).
//!
//! A root links it as any guest links a be-sdk crate: be-wasm crates may depend only on be-wasm
//! ones (buck/cargo/layers.bzl), but a root is not one. A root that links this is an app's
//! module (be-app, for be-host's own root) or, for tests, a be-sdk crate's
//! (be-sdk-root-testing); both families may link be-sdk. A root draws its own surfaces with
//! beui through be-sdk-beui-root, beside this.

mod children;
mod embedded;
mod own;
mod windows;

pub use children::{
    ALREADY_OPEN_ABOVE, Children, KeyboardMove, NOT_SHOWN_YET, PanelPlace, Spawning, child_screen,
    framing_screen, main_screen, preview_screen, preview_screen_of,
};
pub use embedded::{
    AnchoredView, BlockSpawn, Embedded, Hosted, Spawn, SpawnFailure, Wants, tell_editor,
};
pub use own::OwnSurface;
pub use windows::{Delivery, Windows};

use be_lib_geometry::{Pos2, Rect, pos2};
use be_lib_input_router::Drag;
use be_lib_layers::{Layer, LayerId, LayerList, Surface};
use be_lib_ui_types::{CursorIcon, ImeArea, KeyChord, WindowRequests, input};
use be_sdk_api::{
    DropAction, EditorInstanceId, Pos2 as ViewPos2,
    app::EditorEvent,
    clipboard::{Blocks, ClipBlock, ClipFormat},
    shell_types::{DragOver, DragStart},
};
use be_sdk_app_host::anchored::{Hung, dismissed_by_press};
use uuid::Uuid;

/// The apps a root embeds and the routing of its windows' input to them.
pub struct Embedding {
    apps: Vec<Embedded>,
    windows: Windows,
    /// The block an app's editor drags out of itself, carried over the screens under the
    /// pointer until it is dropped (`Embedding::carry`).
    carried: Option<Carried>,
    /// The anchored views that ask for the keyboard (`anchored-placement.keyboard`), while
    /// their apps keep them open.
    keyed: Vec<Keyed>,
}

/// An anchored view that asks for the keyboard, as be-host gives it: to the child placed in it,
/// once that shows, else to the screen of the view it hangs from, which hands its keys on to
/// the views it anchored; that screen takes it back once the view closes.
struct Keyed {
    instance: u64,
    screen: be_sdk_api::ScreenId,
    window: Option<u64>,
    /// The layer showing the view it hangs from.
    owner: Option<LayerId>,
    /// Whether it was given yet: only once, so one drawn again after a frame without it does
    /// not take it again.
    given: bool,
}

/// A block an editor drags out of itself, as be-host's drag board carries it: the router of
/// the window its press is in carries it (`Windows::start_drag`), and the views under the
/// pointer that take a drag hear it.
struct Carried {
    block: DragStart,
    /// The editor's view it is over, and the layer showing it, which hears it
    /// (`editor-event.drag`).
    over: Option<Over>,
}

/// The view of an editor a carried block is over.
struct Over {
    layer: LayerId,
    app: Embedded,
    editor: EditorInstanceId,
    view: be_sdk_api::ViewId,
}

/// What a window's input did (`Embedding::input`).
#[derive(Debug, Default)]
pub struct Routed {
    /// What the root's own surfaces hear of it.
    pub own: Vec<Delivery>,
    /// The presses and Escapes in it, in order, each with the layer it reached (an app's screen
    /// or the root's own): what takes back active children (`Children::revoke`).
    pub pressed: Vec<Pressed>,
}

/// A press, of a pointer, a finger or a pen, or Escape pressed, and the layer it reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pressed {
    pub layer: LayerId,
    /// Escape pressed, not a pointer's press.
    pub escape: bool,
}

impl Embedding {
    /// Routes the keys the root intercepts and the system's gestures to `compositor`, the
    /// root's own layer.
    pub fn new(compositor: LayerId) -> Self {
        Self {
            apps: Vec::new(),
            windows: Windows::new(compositor),
            carried: None,
            keyed: Vec::new(),
        }
    }

    /// Embeds `app`: input routed to its screens reaches it.
    pub fn embed(&mut self, app: Embedded) {
        self.apps.push(app);
    }

    /// Lets go of the app with the handle `instance`: once no other `Embedded` of it is left,
    /// its instance stops.
    pub fn remove(&mut self, instance: u64) {
        self.apps.retain(|app| app.instance() != instance);
    }

    /// The apps embedded, in the order they were.
    pub fn apps(&self) -> &[Embedded] {
        &self.apps
    }

    /// What the root's apps want of its frames (`Embedded::wants`): the earliest next frame
    /// any of them wants, and whether any owes the root something now, as `be3:root`'s
    /// `frame-done` carries them (`next`, `owed`).
    pub fn wants(&self) -> Wants {
        self.apps
            .iter()
            .map(Embedded::wants)
            .fold(Wants::default(), |all, app| Wants {
                next: match (all.next, app.next) {
                    (Some(left), Some(right)) => Some(left.min(right)),
                    (left, right) => left.or(right),
                },
                owed: all.owed || app.owed,
            })
    }

    /// Routes `window`'s input (`events`, be-lib-ui-types' `input::encode`, as `be3:root`'s
    /// `input.deliver` carries it) over the layers drawn last: what lands on an app's screen goes
    /// to the app, at the time it was sampled; what lands on the root's own surfaces is
    /// answered, for the root to take in.
    pub fn input(&mut self, window: Option<u64>, events: &[u8]) -> Result<Routed, String> {
        let events = input::decode(events)?;
        self.dismiss_outside(window, &events);
        let lifted = self.lift();
        let mut routed = self.windows.route(window, events);
        routed.splice(0..0, lifted);
        self.carry();
        let pressed = routed
            .iter()
            .filter_map(|delivery| {
                Some(Pressed {
                    layer: delivery.target,
                    escape: presses(&delivery.input)?,
                })
            })
            .collect();
        Ok(Routed {
            own: self.forward(routed),
            pressed,
        })
    }

    /// Closes the anchored views a press in `window`'s `events` closes, as be-host does
    /// (be-sdk-app-host's `anchored::dismissed_by_press`, over each chain's views in the order
    /// they opened, where they take presses): the app hears which closed on the screen of the
    /// view the chain hangs from (`input-batch.dismissed`), before the press, which still
    /// reaches what it landed on.
    fn dismiss_outside(&self, window: Option<u64>, events: &[input::Timed]) {
        let presses: Vec<Pos2> = events
            .iter()
            .filter_map(|event| match &event.input {
                input::Input::PointerButton {
                    pos, pressed: true, ..
                } => Some(*pos),
                input::Input::Touch {
                    pos,
                    phase: input::TouchPhase::Start,
                    ..
                } => Some(*pos),
                input::Input::Pen {
                    pos,
                    phase: input::PenPhase::Down(_),
                    ..
                } => Some(*pos),
                _ => None,
            })
            .collect();
        if presses.is_empty() {
            return;
        }
        let drawn = self.windows.layers(window);
        for app in &self.apps {
            for chain in anchored_chains(app.anchored()) {
                let shown =
                    |view: &AnchoredView| drawn.iter().any(|layer| layer.id == view.layer.id);
                if !chain.views.iter().any(shown) {
                    continue;
                }
                let hung: Vec<Hung> = chain
                    .views
                    .iter()
                    .map(|view| Hung {
                        id: view.view,
                        parent: chain
                            .views
                            .iter()
                            .any(|held| held.view == view.placement.parent)
                            .then_some(view.placement.parent),
                        floating: match view.floating.is_empty() {
                            true => vec![view.layer.bounds()],
                            false => view.floating.clone(),
                        },
                        dismiss_on_outside_press: view.placement.dismiss_on_outside_press,
                        trigger: view.placement.trigger,
                        blocks_owner: view.placement.blocks_owner,
                    })
                    .collect();
                let mut dismissed = Vec::new();
                for at in &presses {
                    for view in dismissed_by_press(&hung, *at) {
                        if !dismissed.contains(&view) {
                            dismissed.push(view);
                        }
                    }
                }
                if !dismissed.is_empty()
                    && let Some(screen) = app.screen_of(chain.editor, chain.parent)
                {
                    app.dismiss(screen, dismissed);
                }
            }
        }
    }

    /// Gives the keyboard as the anchored views that ask for it open and close, as be-host does
    /// (be-host-runtime's `gives_keyboard`): to the child placed in one, once it shows, else to
    /// the screen of the view it hangs from, which keeps it; once one closes, that screen takes
    /// it back if nothing has it. Answers what the root's own surfaces hear of it.
    fn key_anchored(&mut self) -> Vec<Delivery> {
        let mut told = Vec::new();
        for keyed in std::mem::take(&mut self.keyed) {
            let open = self
                .apps
                .iter()
                .find(|app| app.instance() == keyed.instance)
                .is_some_and(|app| app.hangs(keyed.screen));
            if open {
                self.keyed.push(keyed);
                continue;
            }
            if self.windows.focused(keyed.window).is_none()
                && let Some(owner) = keyed.owner
            {
                told.extend(self.windows.focus(keyed.window, Some(owner)));
            }
        }
        let mut giving = Vec::new();
        for app in &self.apps {
            let views = app.anchored();
            for view in views.iter().filter(|view| view.placement.keyboard) {
                let Some(window) = self.windows.window_of(view.layer.id) else {
                    continue;
                };
                let at = match self.keyed.iter().position(|keyed| {
                    (keyed.instance, keyed.screen) == (app.instance(), view.screen)
                }) {
                    Some(at) => at,
                    None => {
                        let owner = app
                            .screen_of(view.editor, hung_from(&views, view))
                            .and_then(|screen| app.layer(screen, pos2(0.0, 0.0)))
                            .map(|layer| layer.id);
                        self.keyed.push(Keyed {
                            instance: app.instance(),
                            screen: view.screen,
                            window,
                            owner,
                            given: false,
                        });
                        self.keyed.len() - 1
                    }
                };
                let keyed = &mut self.keyed[at];
                if keyed.given {
                    continue;
                }
                let target = match app.places_in(view.editor, view.view) {
                    // Its child, once it shows.
                    true => self
                        .windows
                        .layers(window)
                        .iter()
                        .find(|layer| layer.input.placer == Some(view.layer.id))
                        .map(|layer| layer.id),
                    false => keyed.owner,
                };
                if let Some(target) = target {
                    keyed.given = true;
                    let focused = self.windows.focused(window).map(|layer| layer.id);
                    if focused != Some(target) {
                        giving.push((window, target));
                    }
                }
            }
        }
        for (window, target) in giving {
            told.extend(self.windows.focus(window, Some(target)));
        }
        told
    }

    /// Lifts a block an app's editor began to drag out of itself, as be-host's drag board lifts
    /// it: the window whose layer of that app holds the mouse's press carries it from then on
    /// (`Windows::start_drag`), and that layer's press ends cancelled. One that arrives once
    /// nothing presses (the app's word came after the release) lifts nothing. Answers what the
    /// layers hear of it.
    fn lift(&mut self) -> Vec<Delivery> {
        let started: Vec<(u64, DragStart)> = self
            .apps
            .iter()
            .filter_map(|app| Some((app.instance(), app.take_drag()?)))
            .collect();
        let mut delivered = Vec::new();
        for (instance, block) in started {
            let cancelled = self.windows.start_drag(instance);
            if !cancelled.is_empty() {
                self.end_carry();
                self.carried = Some(Carried { block, over: None });
                delivered.extend(cancelled);
            }
        }
        delivered
    }

    /// Tells the views the lifted block was carried over what the router saw of it
    /// (`Windows::take_dragged`): the view of an app's editor under the pointer that takes a
    /// drag (`Embedded::drop_site`) hears it there (`editor-event.drag`), with what the keys held
    /// make of a drop, and once the pointer is released over it, that it was dropped there; the
    /// view it left, or that Escape put it down over, hears it left. Its host sends each what
    /// the root tells it (`host.tell-editor`).
    fn carry(&mut self) {
        for dragged in self.windows.take_dragged() {
            let Some(carried) = self.carried.as_ref() else {
                continue;
            };
            let block = carried.block;
            let (at, dropped) = match dragged.drag {
                Drag::Over(at) => (at, false),
                Drag::Dropped { at, .. } => (at, true),
                Drag::Left => {
                    if carried
                        .over
                        .as_ref()
                        .is_some_and(|over| over.layer == dragged.target)
                    {
                        self.end_carry();
                    }
                    continue;
                }
            };
            let site = match dragged.surface {
                Surface::Screen { instance, surface } => self
                    .apps
                    .iter()
                    .find(|app| app.instance() == instance)
                    .and_then(|app| {
                        let (editor, view, at) = app.drop_site(surface, at)?;
                        Some((app.clone(), editor, view, at))
                    }),
                _ => None,
            };
            // Over another view than before, the one it was over hears it left, by view as well
            // as by layer: moving from an editor's main view to a view of its dock is leaving.
            let moved = carried.over.as_ref().is_some_and(|over| {
                site.as_ref().is_none_or(|(app, editor, view, _)| {
                    (over.app.instance(), over.editor, over.view)
                        != (app.instance(), *editor, *view)
                })
            });
            if moved {
                self.end_carry();
            }
            let Some((app, editor, view, at)) = site else {
                continue;
            };
            let blocks = Blocks {
                blocks: vec![ClipBlock {
                    id: Uuid::from_bytes(block.block_id),
                    organization: None,
                    block_type: Uuid::from_bytes(block.block_type),
                    title: String::new(),
                }],
            };
            let over = DragOver {
                view,
                pos: ViewPos2 { x: at.x, y: at.y },
                items: vec![ClipFormat::encode(&blocks)],
                action: DropAction::of(dragged.modifiers),
                dropped,
            };
            tell_editor(app.instance(), editor, &EditorEvent::Drag(Some(over)));
            let Some(carried) = self.carried.as_mut() else {
                continue;
            };
            match dropped {
                true => {
                    app.end_drag(editor);
                    carried.over = None;
                }
                false => {
                    carried.over = Some(Over {
                        layer: dragged.target,
                        app,
                        editor,
                        view,
                    })
                }
            }
        }
        // Dropped, or put down: nothing carries it any more.
        if !self.windows.carrying() {
            self.end_carry();
            self.carried = None;
        }
    }

    /// Tells the view the carried block is over that it left it.
    fn end_carry(&mut self) {
        let Some(over) = self
            .carried
            .as_mut()
            .and_then(|carried| carried.over.take())
        else {
            return;
        };
        tell_editor(over.app.instance(), over.editor, &EditorEvent::Drag(None));
        over.app.end_drag(over.editor);
    }

    /// Hands `chords` to the root's own layer (the compositor `new` names) in every window, ahead
    /// of the layer focused there, in place of those it took before (`Windows::intercept`): the
    /// root's own shortcuts, and Escape while a dialog of its own is open.
    /// The editor whose view has the keyboard in `window`, with the app running it and the
    /// screen showing that view: what the root's command palette lists the actions of first.
    pub fn focused_editor(
        &self,
        window: Option<u64>,
    ) -> Option<(Embedded, EditorInstanceId, be_sdk_api::ScreenId)> {
        let focused = self.focused(window)?.id;
        self.apps.iter().find_map(|app| {
            let (editor, screen) = app.screen_at(focused)?;
            Some((app.clone(), editor, screen))
        })
    }

    /// The app embedded as the sandbox instance `instance`.
    pub fn app_of(&self, instance: u64) -> Option<Embedded> {
        self.apps
            .iter()
            .find(|app| app.instance() == instance)
            .cloned()
    }

    /// The palette an embedded editor asked the root to open (`shell-message.open-palette`),
    /// with the app running it, once.
    pub fn take_palette_ask(
        &self,
    ) -> Option<(Embedded, EditorInstanceId, be_sdk_api::PaletteMode)> {
        self.apps.iter().find_map(|app| {
            let (editor, mode) = app.take_palette_ask()?;
            Some((app.clone(), editor, mode))
        })
    }

    pub fn intercept(&mut self, chords: Vec<KeyChord>) {
        self.windows.intercept(chords);
    }

    /// The layer focused in `window`, as it was drawn last: the one its keys, text and input
    /// method go to.
    pub fn focused(&self, window: Option<u64>) -> Option<&Layer> {
        self.windows.focused(window)
    }

    /// Moves the keyboard in `window` to the layer `target`, or takes it from every layer there
    /// (`Windows::focus`): a dialog of the root's own takes it once drawn, and gives it back to
    /// the layer that had it once it closes. An app whose screen lost or got it hears so; what
    /// the root's own surfaces hear of it is answered.
    pub fn focus(&mut self, window: Option<u64>, target: Option<LayerId>) -> Vec<Delivery> {
        let told = self.windows.focus(window, target);
        self.forward(told)
    }

    /// The input method's area in `window`, in its points: the field the app focused there
    /// edits, as its screen asked, placed where the layers drawn last show that screen. None
    /// while the root's own surface has the focus, or the focused app edits no text.
    pub fn ime(&self, window: Option<u64>) -> Option<ImeArea> {
        let layer = self.windows.focused(window)?;
        let Some((instance, surface)) = app_surface(layer.surface) else {
            return None;
        };
        let app = self.apps.iter().find(|app| app.instance() == instance)?;
        let (area, raster) = app.ime(surface)?;
        Some(ime_in_window(area, raster, layer))
    }

    /// What `window` asks of its host for the apps it shows, as `be3:root`'s `frame-done`
    /// carries it: the cursor shape of the screen under the pointer, the pointer lock while the
    /// focused screen holds it (`drew` grants it), and every sample of the pointer's moves while
    /// a screen it shows asks for them. Over the root's own surfaces the cursor is the default
    /// one: a root that draws its own controls sets its own.
    pub fn requests(&self, window: Option<u64>) -> Requests {
        let mut requests = requests(&self.windows, window, |surface| {
            let Some((instance, surface)) = app_surface(*surface) else {
                return None;
            };
            let app = self.apps.iter().find(|app| app.instance() == instance)?;
            app.window(surface)
        });
        // A block carried over an editor that takes it shows that it would land there, as
        // under be-host.
        let accepted = self.carried.as_ref().is_some_and(|carried| {
            carried.over.as_ref().is_some_and(|over| {
                self.windows
                    .layers(window)
                    .iter()
                    .any(|layer| layer.id == over.layer)
                    && over.app.accepts(over.editor)
            })
        });
        if accepted {
            requests.cursor = CursorIcon::Alias;
        }
        requests
    }

    /// Takes `list` as the layers drawn last, which input is routed by from now on, and answers
    /// it as `be3:root`'s `frame-done` carries it (`LayerList::encode`). The focused screen in
    /// each window gets the pointer lock if it asks for it, and one that stopped asking or lost
    /// the focus loses it. An app focused in a window the list leaves out hears it lost the
    /// focus; the root's own surfaces are answered what they hear of these.
    pub fn drew(&mut self, list: LayerList) -> Drawn {
        let layers = list.encode();
        let mut told = self.windows.drew(list);
        // A drag an app began since the last input is lifted as soon as it is heard, over the
        // layers just drawn.
        told.extend(self.lift());
        self.carry();
        told.extend(self.key_anchored());
        let apps = &self.apps;
        told.extend(self.windows.lock(|layer| {
            let Some((instance, surface)) = app_surface(layer.surface) else {
                return false;
            };
            apps.iter()
                .find(|app| app.instance() == instance)
                .and_then(|app| app.window(surface))
                .is_some_and(|window| window.pointer_locked)
        }));
        Drawn {
            layers,
            own: self.forward(told),
        }
    }

    /// Sends each app what is for its screens, and answers the rest.
    fn forward(&self, deliveries: Vec<Delivery>) -> Vec<Delivery> {
        let mut own = Vec::new();
        for delivery in deliveries {
            let Some((instance, surface)) = app_surface(delivery.surface) else {
                own.push(delivery);
                continue;
            };
            if let Some(app) = self.apps.iter().find(|app| app.instance() == instance) {
                app.input(surface, delivery.input, delivery.time);
            }
        }
        own
    }
}

/// The views one editor anchored that hang, one from another, from one of its other views.
struct Chain {
    editor: EditorInstanceId,
    /// The view the chain hangs from.
    parent: be_sdk_api::ViewId,
    views: Vec<AnchoredView>,
}

/// The view of its editor's that is not anchored that `view` hangs from, through the other
/// `views` it hangs from.
fn hung_from(views: &[AnchoredView], view: &AnchoredView) -> be_sdk_api::ViewId {
    let mut parent = view.placement.parent;
    // Each step is another view of the same editor: no more than it anchored.
    for _ in 0..views.len() {
        match views
            .iter()
            .find(|held| held.editor == view.editor && held.view == parent)
        {
            Some(held) => parent = held.placement.parent,
            None => break,
        }
    }
    parent
}

/// `views` as the chains they hang in: each view with the views it hangs from, back to the
/// editor's view that is not anchored, in the order they opened.
fn anchored_chains(mut views: Vec<AnchoredView>) -> Vec<Chain> {
    views.sort_by_key(|view| view.opened);
    let mut chains: Vec<Chain> = Vec::new();
    for view in &views {
        let parent = hung_from(&views, view);
        match chains
            .iter_mut()
            .find(|chain| (chain.editor, chain.parent) == (view.editor, parent))
        {
            Some(chain) => chain.views.push(view.clone()),
            None => chains.push(Chain {
                editor: view.editor,
                parent,
                views: vec![view.clone()],
            }),
        }
    }
    chains
}

/// The app instance and its surface a layer showing `surface` shows: one of its screens, or a
/// view it anchored, which hears input as a screen does.
fn app_surface(surface: Surface) -> Option<(u64, u32)> {
    match surface {
        Surface::Screen { instance, surface } | Surface::Anchored { instance, surface } => {
            Some((instance, surface))
        }
        Surface::Root(_) | Surface::Host(_) => None,
    }
}

/// Whether `input` is a press, of a pointer, a finger or a pen (`Some(false)`), or Escape
/// pressed (`Some(true)`): what takes an active child back from its placer.
fn presses(input: &input::Input) -> Option<bool> {
    use input::{Input, Key, PenPhase, TouchPhase};
    match input {
        Input::PointerButton { pressed: true, .. } => Some(false),
        Input::Touch {
            phase: TouchPhase::Start,
            ..
        } => Some(false),
        Input::Pen {
            phase: PenPhase::Down(_),
            ..
        } => Some(false),
        Input::Key(stroke)
            if stroke.pressed && !stroke.repeat && stroke.key == Some(Key::Escape) =>
        {
            Some(true)
        }
        _ => None,
    }
}

/// What a window asks of its host for the surfaces it shows (`Embedding::requests`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Requests {
    /// The cursor shape the window shows.
    pub cursor: CursorIcon,
    /// Whether the pointer is locked to the window.
    pub pointer_locked: bool,
    /// Whether the window hands the root every sample of the pointer's moves, rather than
    /// merging a frame's into its last.
    pub every_sample: bool,
}

/// What `window` asks for its layers, given what the surface each shows asks of its window
/// (`asks`; none for one that asks nothing, such as the root's own).
pub(crate) fn requests(
    windows: &Windows,
    window: Option<u64>,
    asks: impl Fn(&Surface) -> Option<WindowRequests>,
) -> Requests {
    let cursor = windows
        .hovered(window)
        .and_then(|layer| asks(&layer.surface))
        .map_or(CursorIcon::Default, |asked| asked.cursor);
    let pointer_locked = windows
        .locked(window)
        .is_some_and(|layer| asks(&layer.surface).is_some_and(|asked| asked.pointer_locked));
    let every_sample = windows
        .layers(window)
        .iter()
        .filter(|layer| layer.shown)
        .any(|layer| asks(&layer.surface).is_some_and(|asked| asked.every_sample));
    Requests {
        cursor,
        pointer_locked,
        every_sample,
    }
}

/// `area`, an input method's area in a surface's points, in the window's points: the surface
/// is painted at `raster` pixels per point and shown by `layer`. An app's screen's area
/// (`Embedding::ime`), or a field in a document of the root's own.
pub fn ime_in_window(area: ImeArea, raster: f32, layer: &Layer) -> ImeArea {
    let pixels = |rect: Rect| {
        layer.to_window(Rect::from_min_max(
            pos2(rect.min.x * raster, rect.min.y * raster),
            pos2(rect.max.x * raster, rect.max.y * raster),
        ))
    };
    ImeArea {
        rect: pixels(area.rect),
        cursor: pixels(area.cursor),
        ..area
    }
}

/// A frame the root drew, as `Embedding::drew` takes it.
pub struct Drawn {
    /// The layer list, as `be3:root`'s `frame-done` carries it.
    pub layers: Vec<u8>,
    /// What the root's own surfaces hear of it: the focus a window that went took with it.
    pub own: Vec<Delivery>,
}

#[cfg(test)]
mod tests;
