use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use beui::WindowId;
use beui::native::Document;
use beui::reactive::*;
use beui::styled::*;
use beui_platform_plugin::{Report, View, Views};
use block_editor_plugin::{EditorHost, EditorInstanceId, EditorRegion, RegionId};

pub(crate) trait Shown {
    fn host(&self) -> EditorHost;

    fn generation(&self) -> u64;

    fn content(&self, region: EditorRegion) -> RenderFn<WindowHandle>;

    fn before(&self, view: &View, context: &beui::Context, document: &mut Document);

    fn after(
        &self,
        view: &View,
        context: &beui::Context,
        document: &mut Document,
        window: WindowId,
    ) -> Report;

    fn floated(&self, document: &mut Document, window: WindowId, size: beui::Vec2) -> Report;
}

struct Registry {
    shown: RefCell<HashMap<EditorInstanceId, Rc<dyn Shown>>>,
    windows: RefCell<HashMap<RegionId, WindowId>>,
    revision: OwnedSignal<u64>,
}

impl Registry {
    fn shown(&self, instance: EditorInstanceId) -> Option<Rc<dyn Shown>> {
        self.shown.borrow().get(&instance).cloned()
    }
}

pub(crate) struct Module {
    registry: Rc<Registry>,
    pub(crate) document: RefCell<Option<Document>>,
    launched: std::cell::Cell<bool>,
}

impl Default for Module {
    fn default() -> Self {
        Self {
            registry: Rc::new(Registry {
                shown: RefCell::default(),
                windows: RefCell::default(),
                revision: OwnedSignal::new(0),
            }),
            document: RefCell::new(None),
            launched: std::cell::Cell::new(false),
        }
    }
}

impl Module {
    pub(crate) fn of(host: &EditorHost) -> Option<Rc<Self>> {
        Some(host.module()?.local(Self::default))
    }

    pub(crate) fn show(
        self: &Rc<Self>,
        instance: EditorInstanceId,
        shown: Rc<dyn Shown>,
        host: &EditorHost,
        keeps_accessibility: bool,
    ) {
        self.registry.shown.borrow_mut().insert(instance, shown);
        self.touch();
        if self.launched.replace(true) {
            return;
        }
        let options = beui::RunOptions {
            accessibility_tree: keeps_accessibility,
            ..beui::RunOptions::default()
        };
        let app = ModuleApp {
            module: Rc::clone(self),
            views: None,
            requested: OwnedSignal::new(Vec::new()),
        };
        if let Err(error) =
            beui_platform_plugin::launch(host, options, crate::fonts::context(), app)
        {
            panic!("the plugin's beui app could not start: {error}");
        }
    }

    pub(crate) fn hide(&self, instance: EditorInstanceId) {
        let hidden = self.registry.shown.borrow_mut().remove(&instance);
        drop(hidden);
        self.touch();
    }

    fn touch(&self) {
        self.registry
            .revision
            .handle()
            .update(|revision| *revision += 1);
    }

    fn shown(&self, instance: EditorInstanceId) -> Option<Rc<dyn Shown>> {
        self.registry.shown(instance)
    }

    pub(crate) fn window(&self, region: RegionId) -> Option<WindowId> {
        self.registry.windows.borrow().get(&region).copied()
    }
}

struct ModuleApp {
    module: Rc<Module>,
    views: Option<Views>,
    requested: OwnedSignal<Vec<View>>,
}

impl beui::App for ModuleApp {
    fn setup(&mut self, setup: &beui::Setup) {
        let Some(views) = setup.get::<Views>().cloned() else {
            return;
        };
        let requested = self.requested.handle();
        let watched = views.clone();
        views.watch(move || requested.set(watched.views()));
        self.requested.set(views.views());
        self.views = Some(views);
    }

    fn update(&mut self, context: &beui::Context, rect: beui::Rect) {
        let Some(views) = self.views.clone() else {
            return;
        };
        let module = Rc::clone(&self.module);
        let shown_views = views.views();
        let mut held = module.document.borrow_mut();
        let document = held.get_or_insert_with(|| {
            let requested = self.requested.read_only();
            let registry = Rc::clone(&module.registry);
            let views = views.clone();
            beui::reactive::build(move || {
                view! {
                    <beui::styled::Root clear_color=ColorRole::None>
                        <ModuleWindows registry requested views />
                    </beui::styled::Root>
                }
            })
        });
        beui::native::with_reactive_scope(document, || {});
        for view in &shown_views {
            if let Some(shown) = module.shown(view.id.instance) {
                shown
                    .host()
                    .in_region(view.id.region, || shown.before(view, context, document));
            }
        }
        document.show(context, rect);
        for view in &shown_views {
            let (Some(shown), Some(window)) =
                (module.shown(view.id.instance), module.window(view.id))
            else {
                continue;
            };
            let report = shown.host().in_region(view.id.region, || {
                shown.after(view, context, document, window)
            });
            views.report(view.id, report);
        }
        for (id, window) in views.floated() {
            let Some(shown) = module.shown(id.instance) else {
                continue;
            };
            let size = context.window_size(window).unwrap_or_default();
            let report = shown
                .host()
                .in_region(id.region, || shown.floated(document, window, size));
            views.report(id, report);
        }
    }
}

#[component]
fn ModuleWindows(
    registry: Rc<Registry>,
    requested: ReadSignal<Vec<View>>,
    views: Views,
) -> beui::NodeId {
    let revision = registry.revision.read_only();
    let generations = Rc::clone(&registry);
    let keys = create_memo(move || {
        revision.get();
        requested.with(|requested| {
            requested
                .iter()
                .map(|view| {
                    let generation = generations
                        .shown(view.id.instance)
                        .map_or(0, |shown| shown.generation());
                    (view.id, generation)
                })
                .collect::<Vec<_>>()
        })
    });
    view! {
        <List spacing=Space::None>
            <ForEach keys>|(id, _generation): (RegionId, u64)|
                <RegionWindow registry={Rc::clone(&registry)} id requested views={views.clone()} />
            </ForEach>
        </List>
    }
}

#[component]
fn RegionWindow(
    registry: Rc<Registry>,
    id: RegionId,
    requested: ReadSignal<Vec<View>>,
    views: Views,
) -> beui::NodeId {
    let window = WindowId::next();
    registry.windows.borrow_mut().insert(id, window);
    let held = Rc::clone(&registry);
    on_cleanup(move || {
        let mut windows = held.windows.borrow_mut();
        if windows.get(&id) == Some(&window) {
            windows.remove(&id);
        }
    });
    let initial = requested.with_untracked(|requested| {
        requested
            .iter()
            .find(|view| view.id == id)
            .map_or(beui::Vec2::ZERO, |view| view.size)
    });
    let size = create_memo(move || {
        requested.with(|requested| {
            requested
                .iter()
                .find(|view| view.id == id)
                .map_or(initial, |view| view.size)
        })
    });
    let content = match registry.shown(id.instance) {
        Some(shown) => shown.content(id.region),
        None => RenderFn::new(|_: WindowHandle| {
            view! {
                <Frame />
            }
        }),
    };
    view! {
        <Window id={Some(window)} size={size} placement={views.placement(id)} content />
    }
}
