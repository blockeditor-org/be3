mod escaped;
mod fonts;
mod input;
mod platform;
mod surface;

use be_lib_stable_hash::{HashMap, HashSet};
use std::error::Error;
use std::task::{Context as TaskContext, Poll, Waker as TaskWaker};

use be_sdk::accesskit::{ActionRequest, Affine, TreeUpdate};
#[cfg(target_arch = "wasm32")]
use be_sdk::driver::Damage;
use be_sdk::driver::{Driver, HostServices, Ran, Regions, Surface, Surfaces};
use be_sdk::{
    Claim, DescribedAction, Description, EditorHost, Frame, HostView, Input, ShownView, Stopped,
    TimedInput, ViewHandler, ViewId, ViewPlacement, ViewRef,
};
#[cfg(target_arch = "wasm32")]
use be_sdk::{PaintTarget, SurfaceGpu, SurfaceRect};
use beui_core::context::{Context, FrameOutput};
use beui_core::document::Document;
use beui_core::program::{Program, SafeArea, Setup, Waker};
use beui_core::window::{WindowHosting, WindowState};
use beui_runner::renderer::Loaded;
use beui_runner::{Incoming, Launch, Platform, RunOptions, Runner, Running};
use beui_types::window::{
    HostPlacement, WindowEdge, WindowId, WindowPlacement, WindowReport, WindowRequest,
    WindowRequests,
};
use beui_types::{ActionGroup, Pos2, Rect, Vec2, vec2};

use be_sdk_beui_platform_api::{
    AppRuntime, View, ViewRunner, Views, anchored_layer, anchored_view, window_view,
};
use beui_host::styled::Root;
use platform::HostPlatform;
use surface::{ModuleRenderer, Painter, note};

pub struct AppPlatform {
    host: EditorHost,
}

impl AppPlatform {
    pub fn new(host: EditorHost) -> Self {
        Self { host }
    }
}

impl Platform for AppPlatform {
    fn name(&self) -> &'static str {
        "app"
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
                windows: HashMap::default(),
                sized: HashMap::default(),
                origins: HashMap::default(),
                described: Vec::new(),
                actions: Vec::new(),
                actions_version: 0,
                sent_actions: HashMap::default(),
                focused: HashSet::default(),
                toggles: HashMap::default(),
                toggled: 0,
                overflows: Vec::new(),
                floated: HashMap::default(),
                asks: Vec::new(),
                escaped: HashMap::default(),
                spaces: HashMap::default(),
                placements: HashMap::default(),
                ratio: 1.0,
            };
            self.host
                .handle_views(Box::new(Driver::new(self.host.clone(), handler)));
            Ok(())
        })
    }
}

struct Runtime;

impl AppRuntime for Runtime {
    fn launch(
        &self,
        host: &EditorHost,
        keeps_accessibility: bool,
        program: Box<dyn Program>,
    ) -> Result<(), Box<dyn Error>> {
        let options = RunOptions {
            accessibility_tree: keeps_accessibility,
            ..RunOptions::default()
        };
        launch(host, options, fonts::context(), program)
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
fn __be3_beui_app_runtime() -> &'static dyn AppRuntime {
    &Runtime
}

fn launch(
    host: &EditorHost,
    options: RunOptions,
    context: Context,
    program: Box<dyn Program>,
) -> Result<(), Box<dyn Error>> {
    let launch = Launch {
        options,
        context,
        program,
        layers: Vec::new(),
    };
    let mut running = Box::new(AppPlatform::new(host.clone())).run(launch);
    match running
        .as_mut()
        .poll(&mut TaskContext::from_waker(TaskWaker::noop()))
    {
        Poll::Ready(result) => result,
        Poll::Pending => Err("the app platform starts without waiting".into()),
    }
}

struct ModuleRunner {
    runner: Runner,
    views: Views,
    windows: HashMap<ViewRef, WindowId>,
    sized: HashMap<WindowId, (Vec2, f32)>,
    origins: HashMap<WindowId, Pos2>,
    described: Vec<(ViewRef, u64)>,
    /// Every window's actions as the document last published them, at which of the context's
    /// `actions_version`.
    actions: Vec<ActionGroup>,
    actions_version: u64,
    /// What each view last sent of them, at which change.
    sent_actions: HashMap<ViewRef, (SentActions, Vec<DescribedAction>)>,
    focused: HashSet<ViewRef>,
    toggles: HashMap<ViewRef, u32>,
    toggled: u32,
    overflows: Vec<(WindowId, String)>,
    floated: HashMap<WindowId, Floated>,
    asks: Vec<(ViewRef, be_sdk_api::WindowRequest)>,
    escaped: HashMap<ViewRef, Vec<escaped::Shown>>,
    spaces: HashMap<ViewRef, be_sdk_api::AnchorSpace>,
    placements: HashMap<ViewRef, ViewPlacement>,
    ratio: f32,
}

/// What a view's actions were last worked out from: the context's `actions_version` and the
/// view's window.
type SentActions = (u64, Option<WindowId>);

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
                    fit: region.fit,
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
        self.origins.retain(|window, _| windows.contains(window));
        self.escaped.retain(|id, _| shown(id));
        self.spaces = regions
            .iter()
            .filter_map(|region| Some((region.id(), region.anchor_space?)))
            .collect();
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
                if let Some(origin) = region.origin {
                    let origin = Pos2::ZERO + origin.to_vec2() * ratio;
                    if self.origins.insert(window, origin) != Some(origin) {
                        self.runner
                            .push(Incoming::Window(WindowReport::Moved { window, origin }));
                    }
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

    /// The window `region` shows. A window the app asked the host to float has its region
    /// before the next frame matches the two (`float`), and input can reach that region first.
    fn window_of(&self, region: ViewRef) -> Option<WindowId> {
        self.windows.get(&region).copied().or_else(|| {
            self.floated
                .iter()
                .find(|(_, floated)| floated.region == region)
                .map(|(window, _)| *window)
        })
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
        use be_sdk_api::WindowRequest as Ask;
        let to_region = |size: Vec2| be_sdk_api::Size {
            width: size.x / ratio,
            height: size.y / ratio,
        };
        let window = request.window();
        match request {
            WindowRequest::Open {
                window,
                placement,
                size,
                level,
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
                let placement = self.placement_of(placement, ratio);
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
                    Ask::Open(be_sdk_api::WindowOpen {
                        view: window_view(window),
                        screen: be_sdk_api::ScreenId(0),
                        opener,
                        placement,
                        size: to_region(size),
                        level,
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
                Ask::BeginResize(be_sdk_api::WindowResizeBegin {
                    view: window_view(window),
                    edge: edge_of(edge),
                }),
            ),
            WindowRequest::Resize { window, size } => self.push_ask(
                window,
                Ask::Resize(be_sdk_api::WindowResize {
                    view: window_view(window),
                    size: to_region(size),
                }),
            ),
            WindowRequest::Close(window) => {
                self.push_ask(window, Ask::Close(window_view(window)));
                self.floated.remove(&window);
            }
            WindowRequest::Place { window, placement } => {
                let placement = self.placement_of(placement, ratio);
                self.push_ask(
                    window,
                    Ask::Place(be_sdk_api::WindowPlace {
                        view: window_view(window),
                        placement,
                    }),
                );
            }
        }
    }

    fn placement_of(&self, placement: WindowPlacement, ratio: f32) -> be_sdk_api::WindowPlacement {
        match placement {
            WindowPlacement::AtPointer => be_sdk_api::WindowPlacement::AtPointer,
            WindowPlacement::Over { window: over, at } => match self.region_of(over) {
                Some(region) => be_sdk_api::WindowPlacement::Over(be_sdk_api::WindowOver {
                    view: region,
                    at: Pos2::new(at.x / ratio, at.y / ratio),
                }),
                None => be_sdk_api::WindowPlacement::AtPointer,
            },
            WindowPlacement::Host(HostPlacement(placement)) => {
                be_sdk_api::WindowPlacement::Host(placement)
            }
        }
    }

    fn push_ask(&mut self, window: WindowId, ask: be_sdk_api::WindowRequest) {
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
        self.runner.context().want_overflows(!described.is_empty());
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

    fn count_toggles(&mut self, regions: &[ShownView], active: Option<ViewRef>) {
        let Some(output) = self.runner.output() else {
            return;
        };
        let toggled = output.main().window.requests.keyboard_toggles;
        let asked = toggled.wrapping_sub(std::mem::replace(&mut self.toggled, toggled));
        if asked == 0 {
            return;
        }
        let shown = |id: &ViewRef| regions.iter().any(|region| region.id() == *id);
        let Some(view) = active
            .filter(shown)
            .or_else(|| self.keyboard_view())
            .or_else(|| regions.first().map(ShownView::id))
        else {
            return;
        };
        let count = self.toggles.entry(view).or_default();
        *count = count.wrapping_add(asked);
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
        }
        state.requests.keyboard_toggles = self.toggles.get(&id).copied().unwrap_or(0);
        state.scaled(ratio.recip()).requests
    }

    fn describe(&self, id: ViewRef, region: &ShownView, ratio: f32) -> Description {
        let window = self.windows.get(&id).copied();
        let output = self.runner.output();
        let tree = window.zip(output).map(|(window, output)| {
            let tree = output.window_accessibility_tree(window, "", region.rect.size() * ratio);
            placed(tree, ratio)
        });
        Description { tree }
    }

    fn follow_actions(&mut self, regions: &[ShownView]) {
        // A view the host stops showing forgets its actions, so it sends them again when it
        // is shown again.
        self.sent_actions
            .retain(|view, _| regions.iter().any(|region| region.id() == *view));
        let context = self.runner.context();
        let version = context.actions_version();
        if version != self.actions_version {
            self.actions = context.every_document_action();
            self.actions_version = version;
        }
    }

    /// The view's actions when they changed since it last sent them: those registered in its
    /// window, and for the main view those registered outside any window too.
    fn changed_actions(&mut self, id: ViewRef) -> Option<Vec<DescribedAction>> {
        let window = self.windows.get(&id).copied();
        let changed = (self.actions_version, window);
        if self
            .sent_actions
            .get(&id)
            .is_some_and(|(seen, _)| *seen == changed)
        {
            return None;
        }
        let main = id.view.is_main();
        let mut listed = HashSet::default();
        let mut actions: Vec<DescribedAction> = Vec::new();
        for group in self.actions.iter().filter(|group| {
            (window.is_some() && group.window == window) || (main && group.window.is_none())
        }) {
            for action in &group.actions {
                if !listed.insert(action.id.as_str()) {
                    continue;
                }
                actions.push(DescribedAction {
                    id: action.id.clone(),
                    label: action.label.clone(),
                    glyph: action.glyph.clone(),
                    shortcut: action.shortcut,
                    enabled: action.enabled,
                    checked: action.checked,
                    live: action.live,
                });
            }
        }
        let sent = self
            .sent_actions
            .insert(id, (changed, actions.clone()))
            .map(|(_, sent)| sent);
        (sent.as_ref() != Some(&actions)).then_some(actions)
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
        let mut backdrop = None;
        if let Some(output) = window.and_then(|window| output?.window(window)) {
            let surface = surfaces.of(id);
            let by = surface.visible * ratio;
            note(surface, output, by);
            backdrop = opaque(output.window.appearance.clear_color);
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
        let backdrop = backdrop.filter(|_| !report.shows_through);
        let to_region = |rect: Rect| rect.scaled(ratio.recip());
        let fitted = report
            .fitted
            .filter(|_| region.fit)
            .map(|fitted| fitted / ratio);
        if let Some(fitted) = fitted {
            surfaces.of(id).size = be_sdk::fitted_pixels(fitted, region.raster);
        }
        Frame {
            content: report.content.map(to_region),
            painted: report.painted.into_iter().map(to_region).collect(),
            floating: report.floating.into_iter().map(to_region).collect(),
            floating_painted: report.floating_painted.into_iter().map(to_region).collect(),
            marks: report.marks.into_iter().map(to_region).collect(),
            marks_painted: report.marks_painted.into_iter().map(to_region).collect(),
            scrims: report
                .scrims
                .into_iter()
                .map(|shown| {
                    escaped::scrim(beui_types::layer::Scrim {
                        rect: to_region(shown.rect),
                        radius: shown.radius / ratio,
                        ..shown
                    })
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
            work_area: report.work_area.map(to_region),
            window: self.requests(id, keyboard, ratio),
            description: region.describe.map(|_| self.describe(id, region, ratio)),
            actions: self.changed_actions(id),
            anchored,
            fitted,
            backdrop,
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

fn edge_of(edge: WindowEdge) -> be_sdk_api::WindowEdge {
    use be_sdk_api::WindowEdge as Edge;
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
        let window = self.window_of(id);
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
        } else {
            let target = over.unwrap_or(id);
            match anchored_layer(target.view) {
                Some(layer) => {
                    let space = match target == id {
                        true => region.anchor_space,
                        false => self.spaces.get(&target).copied(),
                    };
                    if let Some(space) = space {
                        let input = escaped::to_host(event.input.clone(), space.to_host);
                        self.runner.push(Incoming::layer(layer, input));
                    }
                }
                None => {
                    let incoming = input::translate(event);
                    let window = self.window_of(target).or(window);
                    self.runner.push(match window {
                        Some(window) => incoming.in_window(window),
                        None => incoming,
                    });
                }
            }
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
            .system_preferences(be_sdk::host_preferences().into());
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
        if let Some(now) = be_sdk::frame_time() {
            self.runner.context().set_clock(now);
            beui_core::timer::pin_wall_clock(Some((now, be_sdk::wall_clock())));
        }
        self.describe_views(regions);
        let Some(frame) = self
            .runner
            .frame(&mut HostPlatform(services), SafeArea::default())
        else {
            return Err(self.stopped());
        };
        self.match_windows(ratio, regions, active);
        self.follow_actions(regions);
        self.count_toggles(regions, active);
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
                beui_types::Color32::TRANSPARENT,
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

/// A view cleared to an opaque color is backed by it on the host (`FrameReport::backdrop`).
fn opaque(clear: beui_types::Color32) -> Option<be_sdk_api::accessibility::Color> {
    let [red, green, blue, alpha] = clear.to_array();
    (alpha == u8::MAX).then_some(be_sdk_api::accessibility::Color {
        red,
        green,
        blue,
        alpha,
    })
}
