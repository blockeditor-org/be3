//! The host panels the root's apps place (`ChildContent::Host`, be-sdk-shell's host panel
//! tabs and a menu button's popover): be-host draws its own, and the root draws them itself, as
//! surfaces of its own the size of each place (plans/host_as_platform_adapter.md, 4g7b3m).

use be_lib_layers::Layer;
use be_sdk_api::{ChildId, EditorInstanceId, HostPanel, ViewId};
use be_sdk_root::PanelPlace;

use super::*;
use crate::{PANEL_LAYERS, PANEL_NOT_DRAWN, PANEL_SURFACES};

/// What names a place among every app's: its placer, its view and its id there.
type PanelKey = (EditorInstanceId, ViewId, ChildId);

/// A host panel the root draws, in one place.
pub(super) struct PanelSurface {
    key: PanelKey,
    panel: HostPanel,
    /// The size of its place, in points, and the pixels per point it is drawn at.
    size: (f32, f32),
    raster: f32,
    surface: BeuiSurface,
    /// The statistics a performance panel shows, which the host's answers set, once its view
    /// was made.
    statistics: Rc<Cell<Option<(Signal<HostStatistics>, Signal<MemoryMinutes>)>>>,
}

impl Root {
    /// Draws each host panel placed now (`places`) on a surface of its own the size of its
    /// place, made as it is first placed and let go of once it is no longer, and answers the
    /// layers showing them, each by its place, with its top left at its surface's origin.
    pub(super) fn settle_panels(&self, places: Vec<PanelPlace>) -> Vec<(PanelKey, Layer)> {
        let mut panels = self.panels.borrow_mut();
        panels.retain(|held| {
            places
                .iter()
                .any(|place| (place.key(), place.panel) == (held.key, held.panel))
        });
        for place in &places {
            let size = (place.size.x.max(1.0), place.size.y.max(1.0));
            let pixels = (
                (size.0 * place.raster).ceil().max(1.0) as u32,
                (size.1 * place.raster).ceil().max(1.0) as u32,
            );
            if let Some(held) = panels.iter_mut().find(|held| held.key == place.key()) {
                if (held.size, held.raster) != (size, place.raster) {
                    held.size = size;
                    held.raster = place.raster;
                    held.surface.resize(pixels, place.raster);
                }
                continue;
            }
            if let Some(panel) = self.open_panel(place, pixels) {
                panels.push(PanelSurface {
                    size,
                    raster: place.raster,
                    ..panel
                });
            }
        }
        let mut layers = Vec::new();
        for held in panels.iter_mut() {
            // A panel that cannot be drawn shows nothing in its place.
            if held.surface.frame().is_err() {
                continue;
            }
            let id = held.surface.id();
            layers.push((
                held.key,
                held.surface
                    .layer(LayerId(PANEL_LAYERS + u64::from(id)), pos2(0.0, 0.0)),
            ));
        }
        layers
    }

    /// A surface drawing `place`'s panel at `pixels`, numbered after the last one made.
    fn open_panel(&self, place: &PanelPlace, pixels: (u32, u32)) -> Option<PanelSurface> {
        let beui = self.beui.borrow().clone()?;
        let id = PANEL_SURFACES + self.panels_made.get();
        self.panels_made.set(self.panels_made.get() + 1);
        let waker = self.waker.clone();
        let statistics: Rc<Cell<Option<(Signal<HostStatistics>, Signal<MemoryMinutes>)>>> =
            Rc::default();
        let shown = Rc::clone(&statistics);
        let panel = place.panel;
        let menu = self.menu_parts();
        let surface = BeuiSurface::new(
            &beui,
            OwnSurface::new(id, pixels),
            place.raster,
            move || waker.wake(),
            move || match (panel, menu) {
                (HostPanel::Performance, _) => {
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
                (HostPanel::Menu, (core, asks)) => {
                    let (want, statistics, panel) = asks.callbacks();
                    let core = create_signal(core);
                    let saving = create_signal(SavingState {
                        generation: 0,
                        unsaved: false,
                        saves: 0,
                        unsent: 0,
                        sync: SyncState::Saved,
                    });
                    view! {
                        <Root>
                            <HostMenu
                                core={core}
                                saving={saving}
                                want={want}
                                statistics={statistics}
                                panel={panel}
                                rows=true
                            />
                        </Root>
                    }
                }
                _ => view! {
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
            be_wasm_ops::guest::spawn(self.clone().hear_panel_statistics(id));
        }
        Some(PanelSurface {
            key: place.key(),
            panel,
            size: (0.0, 0.0),
            raster: place.raster,
            surface,
            statistics,
        })
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

    /// What a menu panel shows and does, as the root's own menu: the host's view as it is
    /// now, and where its asks go (`MenuAsks::callbacks`, made in the panel's own scope).
    fn menu_parts(&self) -> (CoreView, MenuAsks) {
        let asks = MenuAsks {
            wanted: Rc::clone(&self.wanted),
            statistics_asked: Rc::clone(&self.statistics_asked),
            panel_asked: Rc::clone(&self.panel_asked),
            waker: self.waker.clone(),
        };
        (self.core.borrow().clone(), asks)
    }

    /// Watches the host's statistics for the performance panel on surface `id` while it is
    /// placed, showing each change of them in it.
    async fn hear_panel_statistics(self, id: u32) {
        if host::watch_statistics(true).await.is_err() {
            return;
        }
        let mut seen = 0;
        while let Ok(statistics) = host::statistics(seen).await {
            seen = statistics.generation;
            let mut panels = self.panels.borrow_mut();
            let Some(held) = panels.iter_mut().find(|held| held.surface.id() == id) else {
                return;
            };
            if let Some((shown, _)) = held.statistics.get() {
                held.surface.enter(|| shown.set(statistics));
            }
        }
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

/// Where a menu panel's asks go: the root's, as its own menu's.
struct MenuAsks {
    wanted: Rc<RefCell<Option<Asking>>>,
    statistics_asked: Rc<Cell<bool>>,
    panel_asked: Rc<Cell<Option<HostPanel>>>,
    waker: Waker,
}

impl MenuAsks {
    /// Its asks as the menu's callbacks, made in the scope of the view drawing it.
    fn callbacks(self) -> (Callback<Asking>, Callback<()>, Callback<HostPanel>) {
        let Self {
            wanted,
            statistics_asked,
            panel_asked,
            waker,
        } = self;
        let wakes = waker.clone();
        let wakes_too = waker.clone();
        let want = Callback::new(move |asking: Asking| {
            *wanted.borrow_mut() = Some(asking);
            wakes.wake();
        });
        let statistics = Callback::new(move |()| {
            statistics_asked.set(true);
            wakes_too.wake();
        });
        let panel = Callback::new(move |shown: HostPanel| {
            panel_asked.set(Some(shown));
            waker.wake();
        });
        (want, statistics, panel)
    }
}
