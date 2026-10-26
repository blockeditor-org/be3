mod input;
mod platform;
mod surface;
mod views;

use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::task::{Context as TaskContext, Poll, Waker as TaskWaker};

use beui_core::app::{App, SafeArea, Setup, Waker};
use beui_core::context::{Context, FrameOutput};
use beui_core::geometry::{Rect, Vec2, vec2};
use beui_core::renderer::Loaded;
use beui_core::runner::{Incoming, Launch, Platform, RunOptions, Runner, Running};
use beui_core::window::{
    HostPlacement, WindowEdge, WindowHosting, WindowId, WindowPlacement, WindowReport,
    WindowRequest, WindowRequests, WindowState,
};
use block_editor_plugin::accesskit::{ActionRequest, Affine, TreeUpdate};
use block_editor_plugin::{
    Claim, DescribedAction, Description, EditorHost, EditorRegion, Frame, Input, Region, RegionId,
    Regions, TimedInput,
};
#[cfg(target_arch = "wasm32")]
use block_editor_plugin::{PaintTarget, SurfaceRect};

use platform::HostPlatform;
use surface::{ModuleRenderer, Surface};
pub use views::{Report, View, Views};

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
            runner.start(vec![loaded], setup)?;
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
            };
            self.host.handle_regions(Box::new(handler));
            Ok(())
        })
    }
}

pub fn launch(
    host: &EditorHost,
    options: RunOptions,
    context: Context,
    app: impl App + 'static,
) -> Result<(), Box<dyn Error>> {
    let launch = Launch {
        options,
        context,
        app: Box::new(app),
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

pub struct ModuleRunner {
    runner: Runner,
    services: HostPlatform,
    views: Views,
    surfaces: HashMap<RegionId, Surface>,
    windows: HashMap<RegionId, WindowId>,
    sized: HashMap<WindowId, (Vec2, f32)>,
    described: Vec<(RegionId, u64)>,
    focused: HashSet<RegionId>,
    active: Option<RegionId>,
    toggles: HashMap<RegionId, u32>,
    overflows: Vec<(WindowId, String)>,
    floated: HashMap<WindowId, Floated>,
    asks: Vec<(RegionId, block_plugin_api::WindowRequest)>,
}

struct Floated {
    region: RegionId,
    shown: bool,
    placement: Option<String>,
}

impl ModuleRunner {
    pub fn of(handler: &mut dyn Regions) -> Option<&mut Self> {
        (handler as &mut dyn Any).downcast_mut()
    }

    pub fn context(&self) -> &Context {
        self.runner.context()
    }

    pub fn output(&self) -> Option<&FrameOutput> {
        self.runner.output()
    }

    pub fn take_output(&mut self) -> Option<FrameOutput> {
        self.runner.take_output()
    }

    pub fn window(&self, region: RegionId) -> Option<WindowId> {
        self.windows.get(&region).copied()
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
                .filter(|region| !matches!(region.region, EditorRegion::Window(_)))
                .map(|region| View {
                    id: region.id(),
                    block: region.block,
                    size: region.rect.size() * ratio,
                    scale_factor: region.scale_factor,
                    ratio,
                    spec: region.spec,
                })
                .collect(),
        );
        let shown = |id: &RegionId| regions.iter().any(|region| region.id() == *id);
        self.float(regions);
        self.windows.retain(|id, _| shown(id));
        self.surfaces.retain(|id, _| shown(id));
        self.focused.retain(|id| shown(id));
        self.toggles.retain(|id, _| shown(id));
        let windows: HashSet<WindowId> = self.windows.values().copied().collect();
        self.sized.retain(|window, _| windows.contains(window));
        for region in regions {
            let id = region.id();
            if let Some(window) = self.windows.get(&id).copied() {
                let sized = (region.rect.size() * ratio, region.scale_factor / ratio);
                if self.sized.insert(window, sized) != Some(sized) {
                    self.runner.push(Incoming::Window(WindowReport::Resized {
                        window,
                        size: sized.0,
                    }));
                    self.runner.push(Incoming::Window(WindowReport::Scaled {
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
                    if region.placement != floated.placement {
                        floated.placement.clone_from(&region.placement);
                        if let Some(placement) = &region.placement {
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

    fn region_of(&self, window: WindowId) -> Option<RegionId> {
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
                window, placement, size,
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
                        Some(region) => block_plugin_api::WindowPlacement::Over(block_plugin_api::WindowOver {
                            region: reference(region),
                            at: beui_core::geometry::Pos2::new(at.x / ratio, at.y / ratio),
                        }),
                        None => block_plugin_api::WindowPlacement::AtPointer,
                    },
                    WindowPlacement::Host(HostPlacement(placement)) => {
                        block_plugin_api::WindowPlacement::Host(placement)
                    }
                };
                let region = RegionId {
                    instance: opener.instance,
                    region: EditorRegion::Window(window.raw()),
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
                        window: window.raw(),
                        opener: reference(opener),
                        placement,
                        size: to_region(size),
                    }),
                ));
            }
            _ if !self.floated.contains_key(&window) => {}
            WindowRequest::BeginMove(window) => self.push_ask(window, Ask::BeginMove(window.raw())),
            WindowRequest::BeginResize(window, edge) => self.push_ask(
                window,
                Ask::BeginResize(block_plugin_api::WindowResizeBegin {
                    window: window.raw(),
                    edge: edge_of(edge),
                }),
            ),
            WindowRequest::Resize { window, size } => self.push_ask(
                window,
                Ask::Resize(block_plugin_api::WindowResize {
                    window: window.raw(),
                    size: to_region(size),
                }),
            ),
            WindowRequest::Close(window) => {
                self.push_ask(window, Ask::Close(window.raw()));
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
        let described: Vec<(RegionId, u64)> = regions
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

    fn keyboard_region(&self, regions: &[Region]) -> Option<RegionId> {
        let keyboard = self.runner.context().keyboard_window();
        self.windows
            .iter()
            .find(|(_, window)| **window == keyboard)
            .map(|(id, _)| *id)
            .or(self.active)
            .filter(|id| regions.iter().any(|region| region.id() == *id))
            .or_else(|| regions.first().map(Region::id))
    }

    fn requests(&mut self, id: RegionId, keyboard: Option<RegionId>, ratio: f32) -> WindowRequests {
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

    fn describe(&self, id: RegionId, region: &Region, ratio: f32) -> Description {
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

fn reference(id: RegionId) -> block_plugin_api::RegionRef {
    block_plugin_api::RegionRef {
        instance: id.instance,
        region: id.region,
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
    fn input(&mut self, region: &Region, event: &TimedInput, over: Option<RegionId>) {
        let id = region.id();
        self.active = Some(id);
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
        let mut asks = std::mem::take(&mut self.asks);
        let fallback = regions.first().map(Region::id);
        regions
            .iter()
            .map(|region| {
                let id = region.id();
                let mut mine = Vec::new();
                asks.retain(|(from, ask)| {
                    let here = *from == id
                        || (Some(id) == fallback
                            && !regions.iter().any(|region| region.id() == *from));
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
                let report = self.views.take_report(id);
                let to_region = |rect: Rect| rect.scaled(ratio.recip());
                Frame {
                    changed,
                    repaint_after: frame.wait().filter(|_| keyboard == Some(id)),
                    animating: frame.animating,
                    content: report.content.map(to_region),
                    painted: report.painted.into_iter().map(to_region).collect(),
                    floating: report.floating.into_iter().map(to_region).collect(),
                    drawing: report.drawing.into_iter().map(to_region).collect(),
                    every_sample: report.every_sample,
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
                }
            })
            .collect()
    }

    fn painting(&mut self, region: RegionId) -> Option<beui_core::painting::Painting> {
        let surface = self.surfaces.get(&region)?;
        let output = surface.output()?;
        let (width, height) = surface.size;
        let pixels_per_point = output.pixels_per_point;
        beui_core::painting::painting(
            output,
            vec2(
                width as f32 / pixels_per_point,
                height as f32 / pixels_per_point,
            ),
            pixels_per_point,
            output.window.appearance.clear_color,
            beui_core::painting::Punches::Erase,
        )
        .ok()
    }

    fn overflows(&mut self, region: RegionId) -> Vec<String> {
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
    fn paint(&mut self, region: RegionId, target: &PaintTarget<'_>) -> Vec<SurfaceRect> {
        self.surfaces
            .get_mut(&region)
            .map(|surface| surface.paint(target))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests;
