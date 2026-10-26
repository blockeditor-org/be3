mod input;
mod platform;
mod surface;
mod views;

use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::rc::Rc;
use std::task::{Context as TaskContext, Poll, Waker as TaskWaker};

use beui_core::app::{App, SafeArea, Setup, Waker};
use beui_core::context::{Context, FrameOutput};
use beui_core::geometry::{Rect, Vec2, vec2};
use beui_core::renderer::Loaded;
use beui_core::runner::{Incoming, Launch, Platform, RunOptions, Runner, Running};
use beui_core::screens::Screen;
use beui_core::window::{
    WindowHosting, WindowId, WindowPlacement, WindowReport, WindowRequest, WindowRequests,
    WindowState,
};
use block_editor_plugin::accesskit::{ActionRequest, Affine, TreeUpdate};
use block_editor_plugin::{
    Claim, DescribedAction, Description, EditorHost, Frame, Input, Region, RegionId, Regions,
    TimedInput,
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
            let shared = Rc::new(RefCell::new(surface::Shared::default()));
            let views = Views::default();
            let mut runner = Runner::new(launch);
            runner.context().set_window_hosting(WindowHosting::Placed);
            runner.context().set_shows_main(false);
            let waker = self
                .host
                .module()
                .map_or_else(|| self.host.waker(), |module| module.waker());
            let loaded = Loaded {
                renderer: Box::new(ModuleRenderer::new(Rc::clone(&shared))),
                fonts: None,
            };
            let mut setup = Setup::new(Waker::new(move || waker.wake()));
            setup.provide(views.clone());
            runner.start(vec![loaded], setup)?;
            let handler = ModuleRunner {
                runner,
                services: HostPlatform::new(self.host.clone()),
                shared,
                views,
                surfaces: HashMap::new(),
                windows: HashMap::new(),
                sized: HashMap::new(),
                described: Vec::new(),
                focused: HashSet::new(),
                active: None,
                toggles: HashMap::new(),
                overflows: Vec::new(),
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
        host: None,
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
    shared: Rc<RefCell<surface::Shared>>,
    views: Views,
    surfaces: HashMap<RegionId, Surface>,
    windows: HashMap<RegionId, WindowId>,
    sized: HashMap<WindowId, (Vec2, f32)>,
    described: Vec<(RegionId, u64)>,
    focused: HashSet<RegionId>,
    active: Option<RegionId>,
    toggles: HashMap<RegionId, u32>,
    overflows: Vec<(WindowId, String)>,
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
        let scale = regions
            .first()
            .map_or(1.0, |region| region.scale_factor);
        let context = self.runner.context();
        context.set_pixels_per_point(scale);
        self.runner.reports().scaled(scale);
        scale / context.pixels_per_point()
    }

    fn show(&mut self, regions: &[Region], ratio: f32) {
        let monitors = regions
            .iter()
            .find(|region| !region.monitors.is_empty())
            .map(|region| {
                region
                    .monitors
                    .iter()
                    .map(|monitor| {
                        Screen::new(&monitor.id, &monitor.name, monitor.rect)
                            .scaled(region.scale_factor)
                    })
                    .collect()
            });
        let window = || {
            regions
                .iter()
                .map(|region| Screen::window(region.rect).scaled(region.scale_factor))
                .take(1)
                .collect()
        };
        self.shared.borrow_mut().screens = monitors.unwrap_or_else(window);
        self.views.set(
            regions
                .iter()
                .map(|region| View {
                    id: region.id(),
                    size: region.rect.size() * ratio,
                    scale_factor: region.scale_factor,
                    ratio,
                    spec: region.spec,
                })
                .collect(),
        );
        let shown = |id: &RegionId| regions.iter().any(|region| region.id() == *id);
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

    fn match_windows(&mut self) {
        let Some(output) = self.runner.output() else {
            return;
        };
        let requests = output.window_requests.clone();
        for request in requests {
            match request {
                WindowRequest::Open {
                    window,
                    placement: WindowPlacement::Host(placement),
                    ..
                } => {
                    if let Some(id) = self.views.placed(&placement) {
                        self.windows.insert(id, window);
                    }
                }
                WindowRequest::Close(window) => {
                    self.windows.retain(|_, held| *held != window);
                    self.sized.remove(&window);
                }
                _ => {}
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
            state.requests.intercepted_keys.clone_from(&main.intercepted_keys);
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
    fn input(&mut self, region: &Region, event: &TimedInput) {
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
        self.match_windows();
        self.overflows = self
            .runner
            .output()
            .map(|output| output.window_overflows.clone())
            .unwrap_or_default();
        let keyboard = self.keyboard_region(regions);
        regions
            .iter()
            .map(|region| {
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
                    repaint_after: frame.wait(),
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
                    description: region
                        .describe
                        .map(|_| self.describe(id, region, ratio)),
                    stopped: None,
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
