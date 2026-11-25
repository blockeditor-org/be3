//! The host panels the root's apps place (`ChildContent::Host`, be-sdk-shell's host panel
//! tabs and a menu button's popover): be-host draws its own, and the root draws them itself, as
//! surfaces of its own the size of each place (plans/host_as_platform_adapter.md, 4g7b3m). Its
//! menu opens every one of them the way be-host's does, asking the session's editor to show it
//! (`host.show-panel`).

use be_lib_layers::Layer;
use be_sdk_api::{ChildId, EditorInstanceId, HostPanel, ViewId};
use be_sdk_root::PanelPlace;

use super::*;
use crate::{FIRST_PANEL_LAYER, MENU_SURFACE, PANEL_NOT_DRAWN, PANEL_SURFACES};

/// What names a place among every app's: its placer, its view and its id there.
type PanelKey = (EditorInstanceId, ViewId, ChildId);

/// The statistics a performance panel shows and the memory over time it graphs, which the
/// host's answers set.
type Shown = (Signal<HostStatistics>, Signal<MemoryMinutes>);

/// A host panel the root draws, in one place.
pub(super) struct PanelSurface {
    key: PanelKey,
    panel: HostPanel,
    /// The size of its place, in points, and the pixels per point it is drawn at.
    size: (f32, f32),
    raster: f32,
    surface: BeuiSurface,
    /// What a performance panel shows, once its view was made.
    statistics: Rc<Cell<Option<Shown>>>,
}

/// Where the root's menu and a menu panel send what is picked in them, and what they show
/// first: the one way both are built (`MenuParts::view`), so both follow the host's view of the
/// block stack and its saving as they change (`Root::menus_show`).
#[derive(Clone)]
pub(super) struct MenuParts {
    /// The surface drawing it, by which its signals are found again.
    surface: u32,
    core: CoreView,
    saving: SavingState,
    wanted: Rc<RefCell<Option<Asking>>>,
    panel_asked: Rc<Cell<Option<HostPanel>>>,
    menu_shows: MenuShows,
    waker: Waker,
}

impl MenuParts {
    /// The menu, made in the scope of the view drawing it: behind its button, or its rows laid
    /// out in place for a menu panel.
    pub(super) fn view(self, rows: bool) -> NodeId {
        let Self {
            surface,
            core,
            saving,
            wanted,
            panel_asked,
            menu_shows,
            waker,
        } = self;
        let wakes = waker.clone();
        let want = Callback::new(move |asking: Asking| {
            *wanted.borrow_mut() = Some(asking);
            wakes.wake();
        });
        let panel = Callback::new(move |shown: HostPanel| {
            panel_asked.set(Some(shown));
            waker.wake();
        });
        let core = create_signal(core);
        let saving = create_signal(saving);
        menu_shows.borrow_mut().push((surface, core, saving));
        view! {
            <HostMenu core={core} saving={saving} want={want} panel={panel} rows={rows} />
        }
    }
}

impl Root {
    /// What its menu drawn on `surface` is made of: the host's views as they are now, and where
    /// its picks go.
    pub(super) fn menu_parts(&self, surface: u32) -> MenuParts {
        MenuParts {
            surface,
            core: self.core.borrow().clone(),
            saving: self.saving_now.borrow().clone(),
            wanted: Rc::clone(&self.wanted),
            panel_asked: Rc::clone(&self.panel_asked),
            menu_shows: Rc::clone(&self.menu_shows),
            waker: self.waker.clone(),
        }
    }

    /// Shows `core` and `saving`, the host's views as they changed, in every menu drawn: its
    /// own and each menu panel.
    pub(super) fn menus_show(&self, core: Option<&CoreView>, saving: Option<&SavingState>) {
        let shows = self.menu_shows.borrow().clone();
        for (surface, shown_core, shown_saving) in shows {
            let set = || {
                if let Some(core) = core {
                    shown_core.set(core.clone());
                }
                if let Some(saving) = saving {
                    shown_saving.set(saving.clone());
                }
            };
            match surface {
                MENU_SURFACE => {
                    if let Some(menu) = &mut *self.menu.borrow_mut() {
                        menu.enter(set);
                    }
                }
                _ => {
                    let mut panels = self.panels.borrow_mut();
                    if let Some(held) = panels.iter_mut().find(|held| held.surface.id() == surface)
                    {
                        held.surface.enter(set);
                    }
                }
            }
        }
    }

    /// Draws each host panel placed now (`places`) on a surface of its own the size of its
    /// place, made as it is first placed and let go of once it is no longer, and answers the
    /// layers showing them, each by its place, with its top left at its surface's origin.
    pub(super) fn settle_panels(&self, places: Vec<PanelPlace>) -> Vec<(PanelKey, Layer)> {
        let gone: Vec<PanelSurface> = {
            let mut panels = self.panels.borrow_mut();
            let (kept, gone) = std::mem::take(&mut *panels).into_iter().partition(|held| {
                places
                    .iter()
                    .any(|place| (place.key(), place.panel) == (held.key, held.panel))
            });
            *panels = kept;
            gone
        };
        for held in gone {
            self.let_go_panel(held);
        }
        for place in &places {
            let size = (place.size.x.max(1.0), place.size.y.max(1.0));
            let pixels = (
                (size.0 * place.raster).ceil().max(1.0) as u32,
                (size.1 * place.raster).ceil().max(1.0) as u32,
            );
            {
                let mut panels = self.panels.borrow_mut();
                if let Some(held) = panels.iter_mut().find(|held| held.key == place.key()) {
                    if (held.size, held.raster) != (size, place.raster) {
                        held.size = size;
                        held.raster = place.raster;
                        held.surface.resize(pixels, place.raster);
                    }
                    continue;
                }
            }
            if let Some(panel) = self.open_panel(place, size, pixels) {
                self.panels.borrow_mut().push(panel);
            }
        }
        let mut layers = Vec::new();
        for held in self.panels.borrow_mut().iter_mut() {
            // A panel that cannot be drawn shows nothing in its place.
            if held.surface.frame().is_err() {
                continue;
            }
            let id = held.surface.id();
            layers.push((
                held.key,
                held.surface
                    .layer(LayerId(FIRST_PANEL_LAYER + u64::from(id)), pos2(0.0, 0.0)),
            ));
        }
        layers
    }

    /// Lets go of every panel, as when the session's editor that placed them goes.
    pub(super) fn let_go_panels(&self) {
        let gone = std::mem::take(&mut *self.panels.borrow_mut());
        for held in gone {
            self.let_go_panel(held);
        }
    }

    /// Lets go of `held`: its menu's signals, and its watch of the host's statistics, which the
    /// host stops once no panel watches them.
    fn let_go_panel(&self, held: PanelSurface) {
        let id = held.surface.id();
        self.menu_shows
            .borrow_mut()
            .retain(|(surface, ..)| *surface != id);
        if held.panel == HostPanel::Performance {
            let watchers = self.statistics_watchers.get().saturating_sub(1);
            self.statistics_watchers.set(watchers);
            if watchers == 0 {
                be_wasm_ops::guest::spawn(async {
                    // A host that cannot hear it has stopped, which the root hears from its
                    // frames.
                    let _ = host::watch_statistics(false).await;
                });
            }
        }
    }

    /// A surface drawing `place`'s panel at `pixels`, numbered after the last one made.
    fn open_panel(
        &self,
        place: &PanelPlace,
        size: (f32, f32),
        pixels: (u32, u32),
    ) -> Option<PanelSurface> {
        let beui = self.beui.borrow().clone()?;
        let id = PANEL_SURFACES + self.panels_made.get();
        self.panels_made.set(self.panels_made.get() + 1);
        let waker = self.waker.clone();
        let statistics: Rc<Cell<Option<Shown>>> = Rc::default();
        let shown = Rc::clone(&statistics);
        let panel = place.panel;
        let menu = self.menu_parts(id);
        let surface = BeuiSurface::new(
            &beui,
            OwnSurface::new(id, pixels),
            place.raster,
            move || waker.wake(),
            move || match panel {
                HostPanel::Performance => {
                    let statistics = create_signal(HostStatistics {
                        generation: 0,
                        performance: Vec::new(),
                        memory: Vec::new(),
                        memory_file: None,
                    });
                    let history = create_signal(MemoryMinutes::default());
                    shown.set(Some((statistics, history)));
                    view! {
                        <Root>
                            <StatisticsPanel statistics={statistics} history={history} />
                        </Root>
                    }
                }
                HostPanel::Menu => view! {
                    <Root>{menu.view(true)}</Root>
                },
                HostPanel::BlockStack | HostPanel::Apps => view! {
                    <Root>
                        <Frame padding=Space::Md>
                            <Text color=TextColor::Muted>{PANEL_NOT_DRAWN}</Text>
                        </Frame>
                    </Root>
                },
            },
        )
        .ok()?;
        if panel == HostPanel::Performance {
            self.watch_statistics();
        }
        Some(PanelSurface {
            key: place.key(),
            panel,
            size,
            raster: place.raster,
            surface,
            statistics,
        })
    }

    /// Counts another watcher of the host's statistics: the first asks the host to keep them,
    /// and the feeds that hand them to every performance panel run while any watches.
    fn watch_statistics(&self) {
        let watchers = self.statistics_watchers.get() + 1;
        self.statistics_watchers.set(watchers);
        if watchers == 1 {
            be_wasm_ops::guest::spawn(async {
                // A host that cannot hear it has stopped, which the root hears from its frames.
                let _ = host::watch_statistics(true).await;
            });
        }
        if !self.statistics_fed.replace(true) {
            be_wasm_ops::guest::spawn(feed_statistics(
                Rc::clone(&self.panels),
                Rc::clone(&self.statistics_watchers),
                Rc::clone(&self.statistics_fed),
            ));
        }
        if !self.memory_fed.replace(true) {
            be_wasm_ops::guest::spawn(feed_memory(
                Rc::clone(&self.panels),
                Rc::clone(&self.statistics_watchers),
                Rc::clone(&self.memory_fed),
            ));
        }
    }

    /// Asks the session's editor to show the host's panel `panel`, as be-host asks its shell
    /// (`host.show-panel`): it places it, and the root draws it there.
    pub(super) fn show_panel(&self, panel: HostPanel) {
        let Some((instance, editor)) = self.session_editor() else {
            return;
        };
        let shown = host::show_panel(instance, editor, be_wasm_ops::encode(&panel));
        be_wasm_ops::guest::spawn(async move {
            // A host that stopped answering stops the root too.
            let _ = shown.await;
        });
    }

    /// Hands each panel what the root's router delivered to it.
    pub(super) fn panels_heard(&self, own: &[Delivery]) {
        for held in self.panels.borrow_mut().iter_mut() {
            held.surface.input(own);
        }
    }

    /// When the panels want their next frame.
    pub(super) fn panels_next(&self) -> Option<Duration> {
        self.panels
            .borrow()
            .iter()
            .filter_map(|held| held.surface.next())
            .min()
    }

    /// What each panel painted last, by its surface's id.
    pub(super) fn panels_painted(&self) -> Vec<(u64, Painting)> {
        self.panels
            .borrow()
            .iter()
            .filter_map(|held| Some((u64::from(held.surface.id()), held.surface.painting()?)))
            .collect()
    }
}

/// The signals of each menu drawn, by the surface drawing it (`MENU_SURFACE` for the root's
/// own): what `Root::menus_show` sets as the host's views change.
pub(super) type MenuShows = Rc<RefCell<Vec<(u32, Signal<CoreView>, Signal<SavingState>)>>>;

/// Hands each change of the host's statistics to every performance panel in `panels` while
/// `watchers` counts any, then says it stopped (`fed`). It holds only what it hands them to.
async fn feed_statistics(
    panels: Rc<RefCell<Vec<PanelSurface>>>,
    watchers: Rc<Cell<u32>>,
    fed: Rc<Cell<bool>>,
) {
    let mut seen = 0;
    while let Ok(statistics) = host::statistics(seen).await {
        if watchers.get() == 0 {
            break;
        }
        seen = statistics.generation;
        for held in panels.borrow_mut().iter_mut() {
            if let Some((shown, _)) = held.statistics.get() {
                let statistics = statistics.clone();
                held.surface.enter(|| shown.set(statistics));
            }
        }
    }
    fed.set(false);
}

/// Hands each change of the host's memory over time to every performance panel in `panels`
/// while `watchers` counts any, as `feed_statistics` does: they change once a minute, the
/// statistics every frame, so it is a feed of its own.
async fn feed_memory(
    panels: Rc<RefCell<Vec<PanelSurface>>>,
    watchers: Rc<Cell<u32>>,
    fed: Rc<Cell<bool>>,
) {
    let mut seen = 0;
    while let Ok(history) = host::memory_history(seen).await {
        if watchers.get() == 0 {
            break;
        }
        seen = history.generation;
        for held in panels.borrow_mut().iter_mut() {
            if let Some((_, shown)) = held.statistics.get() {
                let history = history.clone();
                held.surface.enter(|| shown.set(history));
            }
        }
    }
    fed.set(false);
}
