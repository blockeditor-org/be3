mod fonts;
mod input;
mod layers;
mod platform;
mod surface;

use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::task::{Context as TaskContext, Poll, Waker as TaskWaker};

use beui_core::app::{App, SafeArea, Setup, Waker};
use beui_core::context::{Context, FrameOutput};
use beui_core::document::Document;
use beui_core::geometry::{Rect, Vec2, vec2};
use beui_core::window::{
    HostPlacement, WindowEdge, WindowHosting, WindowId, WindowPlacement, WindowReport,
    WindowRequest, WindowRequests, WindowState,
};
use beui_runner::renderer::Loaded;
use beui_runner::{Incoming, Launch, Platform, RunOptions, Runner, Running};
use block_editor_plugin::accesskit::{ActionRequest, Affine, TreeUpdate};
use block_editor_plugin::{
    Claim, DescribedAction, Description, EditorHost, Frame, HostView, Input, Region, Regions,
    TimedInput, ViewId, ViewPlacement, ViewRef,
};
#[cfg(target_arch = "wasm32")]
use block_editor_plugin::{PaintTarget, SurfaceRect};

use beui_host::styled::Root;
use beui_platform_plugin_api::{
    PluginRuntime, RegionsRunner, View, Views, anchored_layer, anchored_view, window_view,
};
use platform::HostPlatform;
use surface::{ModuleRenderer, Surface};

pub struct PluginPlatform {
    host: EditorHost,
}

impl PluginPlatform {
    pub fn new(host: EditorHost) -> Self {
        Self { host }
    }
}

impl Platform for PluginPlatform {
    fn name(&self) -> &'static str {
        "plugin"
    }

    fn run(self: Box<Self>, launch: Launch) -> Running {
        Box::pin(async move {
            let views = Views::default();
            let mut runner = Runner::new(launch);
            runner.context().set_window_hosting(WindowHosting::Placed);
            runner.context().set_shows_main(false);
            runner.context().set_hosts_scrims(true);
            let waker = self
                .host
                .module()
                .map_or_else(|| self.host.waker(), |module| module.waker());
            let loaded = Loaded {
                renderer: Box::new(ModuleRenderer),
                fonts: None,
            };
            let mut setup = Setup::new(Waker::new(move || waker.wake()));
            setup.provide(views.clone());
            runner.start(loaded, setup)?;
            let handler = ModuleRunner {
                runner,
                services: HostPlatform::new(self.host.clone()),
                views,
                surfaces: HashMap::new(),
                windows: HashMap::new(),
                sized: HashMap::new(),
                described: Vec::new(),
                focused: HashSet::new(),
                active: None,
                toggles: HashMap::new(),
                overflows: Vec::new(),
                floated: HashMap::new(),
                asks: Vec::new(),
                layers: HashMap::new(),
                placements: HashMap::new(),
                ratio: 1.0,
            };
            self.host.handle_regions(Box::new(handler));
            Ok(())
        })
    }
}

struct Runtime;

impl PluginRuntime for Runtime {
    fn launch(
        &self,
        host: &EditorHost,
        keeps_accessibility: bool,
        app: Box<dyn App>,
    ) -> Result<(), Box<dyn Error>> {
        let options = RunOptions {
            accessibility_tree: keeps_accessibility,
            ..RunOptions::default()
        };
        launch(host, options, fonts::context(), app)
    }

    fn build(&self, root: Box<dyn FnOnce() -> Root>) -> Document {
        beui_host::native::build(root)
    }

    fn context(&self) -> Context {
        fonts::context()
    }

    fn use_bundled_fonts(&self) {
        fonts::use_bundled();
    }

    fn runner<'a>(&self, handler: &'a mut dyn Regions) -> Option<&'a mut dyn RegionsRunner> {
        Some(ModuleRunner::of(handler)?)
    }
}

// unsafe(ffi)
#[unsafe(no_mangle)]
fn __be3_beui_plugin_runtime() -> &'static dyn PluginRuntime {
    &Runtime
}

fn launch(
    host: &EditorHost,
    options: RunOptions,
    context: Context,
    app: Box<dyn App>,
) -> Result<(), Box<dyn Error>> {
    let launch = Launch {
        options,
        context,
        app,
        layers: Vec::new(),
    };
    let mut running = Box::new(PluginPlatform::new(host.clone())).run(launch);
    match running
        .as_mut()
        .poll(&mut TaskContext::from_waker(TaskWaker::noop()))
    {
        Poll::Ready(result) => result,
        Poll::Pending => Err("the plugin platform starts without waiting".into()),
    }
}

struct ModuleRunner {
    runner: Runner,
    services: HostPlatform,
    views: Views,
    surfaces: HashMap<ViewRef, Surface>,
    windows: HashMap<ViewRef, WindowId>,
    sized: HashMap<WindowId, (Vec2, f32)>,
    described: Vec<(ViewRef, u64)>,
    focused: HashSet<ViewRef>,
    active: Option<ViewRef>,
    toggles: HashMap<ViewRef, u32>,
    overflows: Vec<(WindowId, String)>,
    floated: HashMap<WindowId, Floated>,
    asks: Vec<(ViewRef, block_plugin_api::WindowRequest)>,
    layers: HashMap<ViewRef, Vec<layers::Shown>>,
    placements: HashMap<ViewRef, ViewPlacement>,
    ratio: f32,
}

struct Floated {
    region: ViewRef,
    shown: bool,
    placement: Option<String>,
}

impl RegionsRunner for ModuleRunner {
    fn context(&self) -> &Context {
        self.runner.context()
    }

    fn window(&self, region: ViewRef) -> Option<WindowId> {
        self.windows.get(&region).copied()
    }

    fn take_output(&mut self) -> Option<FrameOutput> {
        self.runner.take_output()
    }
}

impl ModuleRunner {
    fn of(handler: &mut dyn Regions) -> Option<&mut Self> {
        (handler as &mut dyn Any).downcast_mut()
    }

    fn ratio(&self, regions: &[Region]) -> f32 {
        let scale = regions.first().map_or(1.0, |region| region.scale_factor);
        let context = self.runner.context();
        context.set_pixels_per_point(scale);
        self.runner.reports().scaled(scale);
        scale / context.pixels_per_point()
    }

    fn show(&mut self, regions: &[Region], ratio: f32) {
        self.views.set(
            regions
                .iter()
                .filter(|region| !region.view.is_window())
                .map(|region| View {
                    id: region.id(),
                    placement: region.view.placement,
                    block: region.block,
                    size: region.rect.size() * ratio,
                    scale_factor: region.scale_factor,
                    ratio,
                    spec: region.spec,
                    docked: region.docked,
                })
                .collect(),
        );
        let shown = |id: &ViewRef| regions.iter().any(|region| region.id() == *id);
        self.float(regions);
        self.windows.retain(|id, _| shown(id));
        self.surfaces.retain(|id, _| shown(id));
        self.focused.retain(|id| shown(id));
        self.toggles.retain(|id, _| shown(id));
        let windows: HashSet<WindowId> = self.windows.values().copied().collect();
        self.sized.retain(|window, _| windows.contains(window));
        self.layers.retain(|id, _| shown(id));
        self.placements = regions
            .iter()
            .map(|region| (region.id(), region.view.placement))
            .collect();
        for region in regions {
            let id = region.id();
            if let Some(window) = self.windows.get(&id).copied() {
                layers::set_space(self.runner.context(), window, region.layers, ratio);
                let raster = match region.raster.is_finite() && region.raster > 0.0 {
                    true => region.raster,
                    false => region.scale_factor,
                };
                let sized = (region.rect.size() * ratio, raster / ratio);
                if self.sized.insert(window, sized) != Some(sized) {
                    self.runner.push(Incoming::Window(WindowReport::Resized {
                        window,
                        size: sized.0,
                    }));
                    self.runner.push(Incoming::Window(WindowReport::Rastered {
                        window,
                        pixels_per_point: sized.1,
                    }));
                }
            }
            let surface = self.surfaces.entry(id).or_default();
            surface.size = (region.pixels[0], region.pixels[1]);
            surface.age = region.age;
            surface.visible = region.visible * ratio;
        }
    }

    fn float(&mut self, regions: &[Region]) {
        let mut gone = Vec::new();
        for (window, floated) in &mut self.floated {
            match regions.iter().find(|region| region.id() == floated.region) {
                Some(region) => {
                    floated.shown = true;
                    self.windows.insert(floated.region, *window);
                    if region.host_place != floated.placement {
                        floated.placement.clone_from(&region.host_place);
                        if let Some(placement) = &region.host_place {
                            self.runner.push(Incoming::Window(WindowReport::Placed {
                                window: *window,
                                placement: HostPlacement(placement.clone()),
                            }));
                        }
                    }
                }
                None if floated.shown => gone.push(*window),
                None => {}
            }
        }
        for window in gone {
            self.floated.remove(&window);
            self.windows.retain(|_, held| *held != window);
            self.sized.remove(&window);
            self.runner
                .push(Incoming::Window(WindowReport::Closed(window)));
        }
        self.views.set_floated(
            self.floated
                .iter()
                .filter(|(_, floated)| floated.shown)
                .map(|(window, floated)| (floated.region, *window))
                .collect(),
        );
    }

    fn region_of(&self, window: WindowId) -> Option<ViewRef> {
        self.windows
            .iter()
            .find(|(_, held)| **held == window)
            .map(|(id, _)| *id)
    }

    fn ask(&mut self, request: WindowRequest, ratio: f32, regions: &[Region]) {
        use block_plugin_api::WindowRequest as Ask;
        let to_region = |size: Vec2| block_plugin_api::Size {
            width: size.x / ratio,
            height: size.y / ratio,
        };
        let window = request.window();
        match request {
            WindowRequest::Open {
                window,
                placement,
                size,
                role,
            } => {
                let opener = match &placement {
                    WindowPlacement::Over { window, .. } => self.region_of(*window),
                    _ => None,
                }
                .or(self.active)
                .filter(|id| regions.iter().any(|region| region.id() == *id))
                .or_else(|| regions.iter().find(|region| region.windows).map(Region::id));
                let Some(opener) = opener else {
                    return;
                };
                let placement = match placement {
                    WindowPlacement::AtPointer => block_plugin_api::WindowPlacement::AtPointer,
                    WindowPlacement::Over { window: over, at } => match self.region_of(over) {
                        Some(region) => {
                            block_plugin_api::WindowPlacement::Over(block_plugin_api::WindowOver {
                                view: region,
                                at: beui_core::geometry::Pos2::new(at.x / ratio, at.y / ratio),
                            })
                        }
                        None => block_plugin_api::WindowPlacement::AtPointer,
                    },
                    WindowPlacement::Host(HostPlacement(placement)) => {
                        block_plugin_api::WindowPlacement::Host(placement)
                    }
                };
                let region = ViewRef {
                    instance: opener.instance,
                    view: window_view(window),
                };
                self.floated.insert(
                    window,
                    Floated {
                        region,
                        shown: false,
                        placement: None,
                    },
                );
                self.asks.push((
                    opener,
                    Ask::Open(block_plugin_api::WindowOpen {
                        view: window_view(window),
                        screen: block_plugin_api::ScreenId(0),
                        opener,
                        placement,
                        size: to_region(size),
                        role,
                    }),
                ));
            }
            _ if !self.floated.contains_key(&window) => {}
            WindowRequest::BeginMove(window) => {
                self.push_ask(window, Ask::BeginMove(window_view(window)))
            }
            WindowRequest::Raise(window) => self.push_ask(window, Ask::Raise(window_view(window))),
            WindowRequest::BeginResize { window, edge } => self.push_ask(
                window,
                Ask::BeginResize(block_plugin_api::WindowResizeBegin {
                    view: window_view(window),
                    edge: edge_of(edge),
                }),
            ),
            WindowRequest::Resize { window, size } => self.push_ask(
                window,
                Ask::Resize(block_plugin_api::WindowResize {
                    view: window_view(window),
                    size: to_region(size),
                }),
            ),
            WindowRequest::Close(window) => {
                self.push_ask(window, Ask::Close(window_view(window)));
                self.floated.remove(&window);
            }
        }
    }

    fn push_ask(&mut self, window: WindowId, ask: block_plugin_api::WindowRequest) {
        if let Some(floated) = self.floated.get(&window) {
            self.asks.push((floated.region, ask));
        }
    }

    fn describe_regions(&mut self, regions: &[Region]) {
        let described: Vec<(ViewRef, u64)> = regions
            .iter()
            .filter_map(|region| Some((region.id(), region.describe?)))
            .collect();
        if self.described == described {
            return;
        }
        match described.is_empty() {
            true => self.runner.accessibility_deactivated(),
            false => self.runner.accessibility_requested(),
        }
        self.runner.context().set_automated(!described.is_empty());
        self.described = described;
    }

    fn match_windows(&mut self, ratio: f32, regions: &[Region]) {
        let Some(output) = self.runner.output() else {
            return;
        };
        let requests = output.window_requests.clone();
        let hosted = self.runner.context().opens_windows();
        for request in requests {
            match &request {
                WindowRequest::Open {
                    window,
                    placement: WindowPlacement::Host(placement),
                    ..
                } if self.views.placed(placement).is_some() => {
                    if let Some(id) = self.views.placed(placement) {
                        self.windows.insert(id, *window);
                    }
                    continue;
                }
                WindowRequest::Close(window) if !self.floated.contains_key(window) => {
                    self.windows.retain(|_, held| held != window);
                    self.sized.remove(window);
                    continue;
                }
                _ => {}
            }
            if hosted {
                self.ask(request, ratio, regions);
            }
        }
    }

    fn keyboard_region(&self, regions: &[Region]) -> Option<ViewRef> {
        let keyboard = self.runner.context().keyboard_window();
        self.windows
            .iter()
            .find(|(_, window)| **window == keyboard)
            .map(|(id, _)| *id)
            .or(self.active)
            .filter(|id| regions.iter().any(|region| region.id() == *id))
            .or_else(|| regions.first().map(Region::id))
    }

    fn requests(&mut self, id: ViewRef, keyboard: Option<ViewRef>, ratio: f32) -> WindowRequests {
        let Some(output) = self.runner.output() else {
            return WindowRequests::default();
        };
        let window = self.windows.get(&id).copied();
        let mut state = window
            .and_then(|window| output.window(window))
            .map_or_else(WindowState::default, |output| output.window.clone());
        let main = &output.main().window.requests;
        let toggles = self.toggles.entry(id).or_insert(main.keyboard_toggles);
        if keyboard == Some(id) {
            if state.requests.ime.is_none() {
                state.requests.ime.clone_from(&main.ime);
            }
            state.requests.pointer_locked |= main.pointer_locked;
            state.requests.handles_back = main.handles_back;
            state.requests.wants_keyboard = main.wants_keyboard;
            state
                .requests
                .intercepted_keys
                .clone_from(&main.intercepted_keys);
            state.requests.pastes.clone_from(&main.pastes);
            state.requests.clipboard_keys = main.clipboard_keys;
            *toggles = main.keyboard_toggles;
        }
        state.requests.keyboard_toggles = *toggles;
        state.scaled(ratio.recip()).requests
    }

    fn describe(&self, id: ViewRef, region: &Region, ratio: f32) -> Description {
        let window = self.windows.get(&id).copied();
        let output = self.runner.output();
        let tree = window.zip(output).map(|(window, output)| {
            let tree = output.window_accessibility_tree(window, "", region.rect.size() * ratio);
            placed(tree, ratio)
        });
        let actions = window
            .zip(output)
            .map(|(window, output)| {
                output
                    .actions
                    .iter()
                    .filter(|group| group.window == Some(window))
                    .flat_map(|group| &group.actions)
                    .map(|action| DescribedAction {
                        id: action.id.clone(),
                        label: action.label.clone(),
                        shortcut: action.shortcut.clone(),
                        enabled: action.enabled,
                        checked: action.checked,
                        live: action.live,
                    })
                    .collect()
            })
            .unwrap_or_default();
        Description { tree, actions }
    }

    #[allow(clippy::too_many_arguments)]
    fn framed(
        &mut self,
        region: &Region,
        asks: Vec<block_plugin_api::WindowRequest>,
        keyboard: Option<ViewRef>,
        escaped: &[beui_core::context::EscapedOutput],
        repaint_after: Option<std::time::Duration>,
        animating: bool,
        ratio: f32,
    ) -> Frame {
        let id = region.id();
        let window = self.windows.get(&id).copied();
        let output = window
            .and_then(|window| self.runner.output()?.window(window))
            .cloned();
        let surface = self.surfaces.entry(id).or_default();
        if let Some(output) = &output {
            surface.note(output);
        }
        let changed = surface.unpainted();
        surface.settle();
        let held = self.layers.entry(id).or_default();
        match window {
            Some(window) => layers::follow(held, escaped, window),
            None => held.clear(),
        }
        let anchored = layers::report(held, region.view.id, ratio);
        let report = self.views.take_report(id);
        let to_region = |rect: Rect| rect.scaled(ratio.recip());
        Frame {
            changed,
            repaint_after,
            animating,
            content: report.content.map(to_region),
            painted: report.painted.into_iter().map(to_region).collect(),
            floating: report.floating.into_iter().map(to_region).collect(),
            floating_painted: report.floating_painted.into_iter().map(to_region).collect(),
            scrims: report
                .scrims
                .into_iter()
                .map(|(rect, color)| {
                    let [red, green, blue, alpha] = color.to_array();
                    block_plugin_api::Scrim {
                        rect: to_region(rect),
                        color: block_plugin_api::accessibility::Color {
                            red,
                            green,
                            blue,
                            alpha,
                        },
                    }
                })
                .collect(),
            drawing: report.drawing.into_iter().map(to_region).collect(),
            claims: report
                .claims
                .into_iter()
                .map(|claim| Claim {
                    modifiers: claim.modifiers,
                    rect: to_region(claim.rect),
                })
                .collect(),
            window: self.requests(id, keyboard, ratio),
            description: region.describe.map(|_| self.describe(id, region, ratio)),
            stopped: None,
            asks,
            anchored,
        }
    }

    fn owner_of(&self, view: ViewRef) -> Option<ViewRef> {
        self.layers
            .iter()
            .find(|(owner, held)| {
                owner.instance == view.instance
                    && held
                        .iter()
                        .any(|shown| anchored_view(shown.escaped.layer.id) == view.view)
            })
            .map(|(owner, _)| *owner)
    }

    fn anchored(&mut self, view: ViewRef) -> Option<&mut layers::Shown> {
        self.layers
            .iter_mut()
            .filter(|(owner, _)| owner.instance == view.instance)
            .flat_map(|(_, held)| held.iter_mut())
            .find(|shown| anchored_view(shown.escaped.layer.id) == view.view)
    }

    fn anchored_frame(&mut self, region: &Region) -> Frame {
        let describe = region.describe.is_some();
        let output = self.runner.output();
        let context = self.runner.context();
        let mut description = None;
        if describe
            && let Some(owner) = self.owner_of(region.id())
            && let Some(window) = self.windows.get(&owner).copied()
            && let Some(space) = context.layer_space(window)
            && let Some(output) = output
            && let Some(shown) = self.layers.get(&owner).and_then(|held| {
                held.iter()
                    .find(|shown| anchored_view(shown.escaped.layer.id) == region.view.id)
            })
        {
            let layer = &shown.escaped.layer;
            description = Some(Description {
                tree: output.escaped_accessibility_tree(
                    layer.id,
                    layer.rect.size(),
                    layer.rect.min,
                    space.to_host,
                ),
                actions: Vec::new(),
            });
        }
        match self.anchored(region.id()) {
            Some(shown) => layers::framed(shown, description),
            None => Frame::default(),
        }
    }

    fn stopped(&self, regions: &[Region]) -> Vec<Frame> {
        let stopped = self.runner.exited().then(|| match self.runner.failure() {
            Some(failure) => block_plugin_api::Stopped::Failed(failure.to_owned()),
            None => block_plugin_api::Stopped::Closed,
        });
        regions
            .iter()
            .map(|_| Frame {
                stopped: stopped.clone(),
                ..Frame::default()
            })
            .collect()
    }
}

fn edge_of(edge: WindowEdge) -> block_plugin_api::WindowEdge {
    use block_plugin_api::WindowEdge as Edge;
    match edge {
        WindowEdge::Top => Edge::Top,
        WindowEdge::Bottom => Edge::Bottom,
        WindowEdge::Left => Edge::Left,
        WindowEdge::Right => Edge::Right,
        WindowEdge::TopLeft => Edge::TopLeft,
        WindowEdge::TopRight => Edge::TopRight,
        WindowEdge::BottomLeft => Edge::BottomLeft,
        WindowEdge::BottomRight => Edge::BottomRight,
    }
}

fn placed(mut tree: TreeUpdate, ratio: f32) -> TreeUpdate {
    let root = tree
        .tree
        .as_ref()
        .map_or(beui_core::accessibility::WINDOW_NODE, |tree| tree.root);
    for (id, node) in &mut tree.nodes {
        if *id == root {
            node.set_transform(Affine::scale(f64::from(ratio.recip())));
        }
    }
    tree
}

impl Regions for ModuleRunner {
    fn input(&mut self, region: &Region, event: &TimedInput, over: Option<ViewRef>) {
        let id = region.id();
        self.active = Some(self.owner_of(id).unwrap_or(id));
        self.services.serve(region.instance);
        let window = self.windows.get(&id).copied();
        if let Input::Focus(focused) = event.input {
            if let Some(window) = window {
                self.runner
                    .push(Incoming::Window(WindowReport::Focused { window, focused }));
            }
            let was = !self.focused.is_empty();
            match focused {
                true => self.focused.insert(id),
                false => self.focused.remove(&id),
            };
            let now = !self.focused.is_empty();
            if was != now {
                self.runner.push(Incoming::logical(Input::Focus(now)));
            }
        } else if region.view.is_anchored() {
            if let (Some(space), Some(layer)) = (region.layers, anchored_layer(region.view.id)) {
                let input = layers::to_host(event.input.clone(), space.to_host);
                self.runner.push(Incoming::layer(layer, input));
            }
        } else {
            let incoming = input::translate(event);
            let window = over
                .and_then(|over| self.windows.get(&over).copied())
                .or(window);
            self.runner.push(match window {
                Some(window) => incoming.in_window(window),
                None => incoming,
            });
        }
        if let Some(at) = input::time(event) {
            self.runner.push(Incoming::Time(at));
        }
    }

    fn dismiss_anchored(&mut self, region: &Region, views: &[ViewId]) {
        self.services.serve(region.instance);
        for layer in views.iter().copied().filter_map(anchored_layer) {
            self.runner.context().dismiss_layer(layer);
        }
    }

    fn run_action(&mut self, region: &Region, id: &str) {
        let context = self.runner.context();
        match self.windows.get(&region.id()) {
            Some(window) => context.request_action_in(*window, id.to_owned()),
            None => context.request_action(id.to_owned()),
        }
    }

    fn node_action(&mut self, region: &Region, request: ActionRequest) {
        self.services.serve(region.instance);
        self.runner.accessibility_action(request);
    }

    fn frame(&mut self, regions: &[Region]) -> Vec<Frame> {
        self.runner
            .reports()
            .system_preferences(block_editor_plugin::host_preferences().into());
        let ratio = self.ratio(regions);
        self.ratio = ratio;
        let hosting = match regions.iter().any(|region| region.windows) {
            true => WindowHosting::Any,
            false => WindowHosting::Placed,
        };
        if self.runner.context().window_hosting() != hosting {
            self.runner.context().set_window_hosting(hosting);
        }
        self.show(regions, ratio);
        self.runner
            .set_occluded(!regions.iter().any(|region| region.seen));
        if let Some(now) = block_editor_plugin::frame_time() {
            self.runner.context().set_clock(now);
            beui_core::timer::pin_wall_clock(Some((now, block_editor_plugin::wall_clock())));
        }
        self.describe_regions(regions);
        let Some(frame) = self.runner.frame(&mut self.services, SafeArea::default()) else {
            return self.stopped(regions);
        };
        self.match_windows(ratio, regions);
        self.overflows = self
            .runner
            .output()
            .map(|output| output.window_overflows.clone())
            .unwrap_or_default();
        let keyboard = self.keyboard_region(regions);
        let escaped = self
            .runner
            .output()
            .map(|output| output.escaped.clone())
            .unwrap_or_default();
        let mut asks = std::mem::take(&mut self.asks);
        let fallback = regions.first().map(Region::id);
        regions
            .iter()
            .map(|region| {
                let id = region.id();
                let mut mine = Vec::new();
                asks.retain(|(from, ask)| {
                    let owner = regions
                        .iter()
                        .map(Region::id)
                        .find(|region| region.instance == from.instance)
                        .or(fallback);
                    let here = owner == Some(id);
                    if here {
                        mine.push(ask.clone());
                    }
                    !here
                });
                (region, mine)
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|(region, asks)| {
                let wait = frame.wait().filter(|_| keyboard == Some(region.id()));
                self.framed(
                    region,
                    asks,
                    keyboard,
                    &escaped,
                    wait,
                    frame.animating,
                    ratio,
                )
            })
            .collect()
    }

    fn frame_opened(&mut self, regions: &[Region]) -> Vec<Frame> {
        let ratio = self.ratio;
        for region in regions.iter().filter(|region| !region.view.is_anchored()) {
            let id = region.id();
            let Some(window) = self
                .floated
                .iter()
                .find(|(_, floated)| floated.region == id)
                .map(|(window, _)| *window)
            else {
                continue;
            };
            if let Some(floated) = self.floated.get_mut(&window) {
                floated.shown = true;
            }
            self.windows.insert(id, window);
            self.placements.insert(id, region.view.placement);
            let surface = self.surfaces.entry(id).or_default();
            surface.size = (region.pixels[0], region.pixels[1]);
            surface.age = region.age;
            surface.visible = region.visible * ratio;
        }
        self.views.set_floated(
            self.floated
                .iter()
                .filter(|(_, floated)| floated.shown)
                .map(|(window, floated)| (floated.region, *window))
                .collect(),
        );
        let escaped = self
            .runner
            .output()
            .map(|output| output.escaped.clone())
            .unwrap_or_default();
        regions
            .iter()
            .map(|region| match region.view.is_anchored() {
                true => self.anchored_frame(region),
                false => self.framed(region, Vec::new(), None, &escaped, None, false, ratio),
            })
            .collect()
    }

    fn painting(&mut self, region: ViewRef) -> Option<beui_core::painting::Painting> {
        if let Some(shown) = self.anchored(region) {
            let output = &shown.escaped.output;
            return beui_core::painting::painting(
                output,
                output.size,
                output.pixels_per_point,
                beui_core::color::Color32::TRANSPARENT,
                beui_core::painting::Punches::Erase,
            )
            .ok();
        }
        let surface = self.surfaces.get(&region)?;
        let output = surface.output()?;
        let (width, height) = surface.size;
        let pixels_per_point = output.pixels_per_point;
        let size = match self.placements.get(&region) {
            Some(ViewPlacement::Host(HostView::Window)) => output.size,
            _ => vec2(
                width as f32 / pixels_per_point,
                height as f32 / pixels_per_point,
            ),
        };
        beui_core::painting::painting(
            output,
            size,
            pixels_per_point,
            output.window.appearance.clear_color,
            beui_core::painting::Punches::Erase,
        )
        .ok()
    }

    fn overflows(&mut self, region: ViewRef) -> Vec<String> {
        let Some(window) = self.windows.get(&region) else {
            return Vec::new();
        };
        self.overflows
            .iter()
            .filter(|(held, _)| held == window)
            .map(|(_, found)| found.clone())
            .collect()
    }

    #[cfg(target_arch = "wasm32")]
    fn paint(&mut self, region: ViewRef, target: &PaintTarget<'_>) -> Vec<SurfaceRect> {
        if let Some(shown) = self.anchored(region) {
            shown.surface.size = (target.width, target.height);
            return shown.surface.paint(target);
        }
        self.surfaces
            .get_mut(&region)
            .map(|surface| surface.paint(target))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests;
