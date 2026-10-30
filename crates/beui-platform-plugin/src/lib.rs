mod escaped;
mod fonts;
mod input;
mod platform;
mod surface;

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
#[cfg(target_arch = "wasm32")]
use block_editor_plugin::driver::Damage;
use block_editor_plugin::driver::{Driver, HostServices, Ran, Regions, Surface, Surfaces};
use block_editor_plugin::{
    Claim, DescribedAction, Description, EditorHost, Frame, HostView, Input, ShownView, Stopped,
    TimedInput, ViewHandler, ViewId, ViewPlacement, ViewRef,
};
#[cfg(target_arch = "wasm32")]
use block_editor_plugin::{PaintTarget, SurfaceGpu, SurfaceRect};

use beui_host::styled::Root;
use beui_platform_plugin_api::{
    PluginRuntime, View, ViewRunner, Views, anchored_layer, anchored_view, window_view,
};
use platform::HostPlatform;
use surface::{ModuleRenderer, Painter, note};

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
                views,
                windows: HashMap::new(),
                sized: HashMap::new(),
                described: Vec::new(),
                focused: HashSet::new(),
                toggles: HashMap::new(),
                overflows: Vec::new(),
                floated: HashMap::new(),
                asks: Vec::new(),
                escaped: HashMap::new(),
                placements: HashMap::new(),
                ratio: 1.0,
            };
            self.host
                .handle_views(Box::new(Driver::new(self.host.clone(), handler)));
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

    fn runner<'a>(&self, handler: &'a mut dyn ViewHandler) -> Option<&'a mut dyn ViewRunner> {
        Some(Driver::<ModuleRunner>::of(handler)?.regions_mut())
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
    views: Views,
    windows: HashMap<ViewRef, WindowId>,
    sized: HashMap<WindowId, (Vec2, f32)>,
    described: Vec<(ViewRef, u64)>,
    focused: HashSet<ViewRef>,
    toggles: HashMap<ViewRef, u32>,
    overflows: Vec<(WindowId, String)>,
    floated: HashMap<WindowId, Floated>,
    asks: Vec<(ViewRef, block_plugin_api::WindowRequest)>,
    escaped: HashMap<ViewRef, Vec<escaped::Shown>>,
    placements: HashMap<ViewRef, ViewPlacement>,
    ratio: f32,
}

struct Floated {
    region: ViewRef,
    shown: bool,
    placement: Option<String>,
}

impl ViewRunner for ModuleRunner {
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
    fn ratio(&self, regions: &[ShownView]) -> f32 {
        let scale = regions.first().map_or(1.0, |region| region.scale_factor);
        let context = self.runner.context();
        context.set_pixels_per_point(scale);
        self.runner.reports().scaled(scale);
        scale / context.pixels_per_point()
    }

    fn show(&mut self, regions: &[ShownView], ratio: f32) {
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
        self.focused.retain(|id| shown(id));
        self.toggles.retain(|id, _| shown(id));
        let windows: HashSet<WindowId> = self.windows.values().copied().collect();
        self.sized.retain(|window, _| windows.contains(window));
        self.escaped.retain(|id, _| shown(id));
        self.placements = regions
            .iter()
            .map(|region| (region.id(), region.view.placement))
            .collect();
        for region in regions {
            let id = region.id();
            if let Some(window) = self.windows.get(&id).copied() {
                escaped::set_space(self.runner.context(), window, region.anchor_space, ratio);
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
        }
    }

    fn float(&mut self, regions: &[ShownView]) {
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

    fn ask(
        &mut self,
        request: WindowRequest,
        ratio: f32,
        regions: &[ShownView],
        active: Option<ViewRef>,
    ) {
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
                ..
            } => {
                let opener = match &placement {
                    WindowPlacement::Over { window, .. } => self.region_of(*window),
                    _ => None,
                }
                .or(active)
                .filter(|id| regions.iter().any(|region| region.id() == *id))
                .or_else(|| {
                    regions
                        .iter()
                        .find(|region| region.windows)
                        .map(ShownView::id)
                });
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

    fn describe_views(&mut self, regions: &[ShownView]) {
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

    fn match_windows(&mut self, ratio: f32, regions: &[ShownView], active: Option<ViewRef>) {
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
                self.ask(request, ratio, regions, active);
            }
        }
    }

    fn keyboard_view(&self) -> Option<ViewRef> {
        let keyboard = self.runner.context().keyboard_window();
        self.windows
            .iter()
            .find(|(_, window)| **window == keyboard)
            .map(|(id, _)| *id)
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

    fn describe(&self, id: ViewRef, region: &ShownView, ratio: f32) -> Description {
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

    fn region_frame(
        &mut self,
        surfaces: &mut Surfaces<Painter>,
        region: &ShownView,
        keyboard: Option<ViewRef>,
    ) -> Frame {
        let ratio = self.ratio;
        let id = region.id();
        let window = self.windows.get(&id).copied();
        let output = self.runner.output();
        if let Some(output) = window.and_then(|window| output?.window(window)) {
            let surface = surfaces.of(id);
            let by = surface.visible * ratio;
            note(surface, output, by);
        }
        let held = self.escaped.entry(id).or_default();
        match window {
            Some(window) => escaped::follow(
                held,
                output.map_or(&[][..], |output| output.escaped.as_slice()),
                window,
            ),
            None => held.clear(),
        }
        let anchored = escaped::report(held, region.view.id, ratio);
        let report = self.views.take_report(id);
        let to_region = |rect: Rect| rect.scaled(ratio.recip());
        Frame {
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
            anchored,
            ..Frame::default()
        }
    }

    fn owner_of(&self, view: ViewRef) -> Option<ViewRef> {
        self.escaped
            .iter()
            .find(|(owner, held)| {
                owner.instance == view.instance
                    && held
                        .iter()
                        .any(|shown| anchored_view(shown.escaped.layer.id) == view.view)
            })
            .map(|(owner, _)| *owner)
    }

    fn anchored_shown(&mut self, view: ViewRef) -> Option<&mut escaped::Shown> {
        self.escaped
            .iter_mut()
            .filter(|(owner, _)| owner.instance == view.instance)
            .flat_map(|(_, held)| held.iter_mut())
            .find(|shown| anchored_view(shown.escaped.layer.id) == view.view)
    }

    fn anchored_frame(&mut self, region: &ShownView) -> Frame {
        let describe = region.describe.is_some();
        let output = self.runner.output();
        let context = self.runner.context();
        let mut description = None;
        if describe
            && let Some(owner) = self.owner_of(region.id())
            && let Some(window) = self.windows.get(&owner).copied()
            && let Some(space) = context.layer_space(window)
            && let Some(output) = output
            && let Some(shown) = self.escaped.get(&owner).and_then(|held| {
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
        match self.anchored_shown(region.id()) {
            Some(shown) => escaped::framed(shown, description),
            None => Frame::default(),
        }
    }

    fn stopped(&self) -> Option<Stopped> {
        self.runner.exited().then(|| match self.runner.failure() {
            Some(failure) => Stopped::Failed(failure.to_owned()),
            None => Stopped::Closed,
        })
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
    type Painter = Painter;

    fn input(
        &mut self,
        _services: &mut HostServices,
        region: &ShownView,
        event: &TimedInput,
        over: Option<ViewRef>,
    ) {
        let id = region.id();
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
            if let (Some(space), Some(layer)) =
                (region.anchor_space, anchored_layer(region.view.id))
            {
                let input = escaped::to_host(event.input.clone(), space.to_host);
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

    fn dismiss_anchored(&mut self, _region: &ShownView, views: &[ViewId]) {
        for layer in views.iter().copied().filter_map(anchored_layer) {
            self.runner.context().dismiss_layer(layer);
        }
    }

    fn run_action(&mut self, region: &ShownView, id: &str) {
        let context = self.runner.context();
        match self.windows.get(&region.id()) {
            Some(window) => context.request_action_in(*window, id.to_owned()),
            None => context.request_action(id.to_owned()),
        }
    }

    fn node_action(&mut self, _region: &ShownView, request: ActionRequest) {
        self.runner.accessibility_action(request);
    }

    fn owner(&self, view: ViewRef) -> Option<ViewRef> {
        self.owner_of(view)
    }

    fn frame(
        &mut self,
        services: &mut HostServices,
        active: Option<ViewRef>,
        regions: &[ShownView],
    ) -> Result<Ran, Option<Stopped>> {
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
        self.describe_views(regions);
        let Some(frame) = self
            .runner
            .frame(&mut HostPlatform(services), SafeArea::default())
        else {
            return Err(self.stopped());
        };
        self.match_windows(ratio, regions, active);
        self.overflows = self
            .runner
            .output()
            .map(|output| output.window_overflows.clone())
            .unwrap_or_default();
        Ok(Ran {
            keyboard: self.keyboard_view(),
            asks: std::mem::take(&mut self.asks),
            repaint_after: frame.wait(),
            animating: frame.animating,
        })
    }

    fn opened(&mut self, regions: &[ShownView]) {
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
        }
        self.views.set_floated(
            self.floated
                .iter()
                .filter(|(_, floated)| floated.shown)
                .map(|(window, floated)| (floated.region, *window))
                .collect(),
        );
    }

    fn framed(
        &mut self,
        surfaces: &mut Surfaces<Painter>,
        region: &ShownView,
        keyboard: Option<ViewRef>,
    ) -> Frame {
        self.region_frame(surfaces, region, keyboard)
    }

    fn anchored_frame(&mut self, region: &ShownView) -> Frame {
        ModuleRunner::anchored_frame(self, region)
    }

    fn anchored(&mut self, view: ViewRef) -> Option<&mut Surface<Painter>> {
        self.anchored_shown(view).map(|shown| &mut shown.surface)
    }

    fn painting(
        &mut self,
        surfaces: &Surfaces<Painter>,
        region: ViewRef,
    ) -> Option<beui_core::painting::Painting> {
        if let Some(shown) = self.anchored_shown(region) {
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
        let surface = surfaces.get(region)?;
        let output = surface.painter.output()?;
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
    fn paint(
        painter: &mut Painter,
        gpu: &SurfaceGpu,
        target: &PaintTarget<'_>,
        size: (u32, u32),
        repaint: Damage,
    ) -> Option<Vec<SurfaceRect>> {
        surface::paint(painter, gpu, target, size, repaint)
    }
}

#[cfg(test)]
mod tests;
