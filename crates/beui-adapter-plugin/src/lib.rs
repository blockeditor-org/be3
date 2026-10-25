mod input;
mod platform;
mod surface;

use std::cell::RefCell;
use std::rc::Rc;
use std::task::{Context as TaskContext, Poll, Waker as TaskWaker};

use beui_core::app::{App, SafeArea, Setup, Waker};
use beui_core::context::{Context, FrameOutput};
use beui_core::geometry::Rect;
use beui_core::renderer::Loaded;
use beui_core::runner::{Adapter, Incoming, Launch, RunOptions, Runner, Running};
use beui_core::screens::Screen;
use beui_core::window::WindowState;
use block_editor_plugin::accesskit::{ActionRequest, Affine, TreeUpdate};
use block_editor_plugin::{DescribedAction, Description, EditorHost, Frame, Region, TimedInput};
#[cfg(target_arch = "wasm32")]
use block_editor_plugin::{PaintTarget, SurfaceRect};

use platform::HostPlatform;
use surface::{Surface, SurfaceRenderer};

pub struct PluginAdapter {
    host: EditorHost,
    surface: Rc<RefCell<Surface>>,
    launched: Rc<RefCell<Option<Runner>>>,
}

impl Adapter for PluginAdapter {
    fn name(&self) -> &'static str {
        "plugin"
    }

    fn run(self: Box<Self>, launch: Launch) -> Running {
        Box::pin(async move {
            let mut runner = Runner::new(launch);
            let waker = self.host.waker();
            let loaded = Loaded {
                renderer: Box::new(SurfaceRenderer::new(Rc::clone(&self.surface))),
                fonts: None,
            };
            runner.start(vec![loaded], Setup::new(Waker::new(move || waker.wake())))?;
            *self.launched.borrow_mut() = Some(runner);
            Ok(())
        })
    }
}

pub struct PluginSurface {
    runner: Runner,
    platform: HostPlatform,
    surface: Rc<RefCell<Surface>>,
    described: Option<u64>,
}

impl PluginSurface {
    pub fn launch(host: EditorHost, context: Context, app: impl App + 'static) -> Self {
        let surface = Rc::new(RefCell::new(Surface::default()));
        let launched = Rc::new(RefCell::new(None));
        let adapter = Box::new(PluginAdapter {
            host: host.clone(),
            surface: Rc::clone(&surface),
            launched: Rc::clone(&launched),
        });
        let launch = Launch {
            options: RunOptions::default(),
            context,
            app: Box::new(app),
            host: None,
        };
        let mut running = adapter.run(launch);
        match running
            .as_mut()
            .poll(&mut TaskContext::from_waker(TaskWaker::noop()))
        {
            Poll::Ready(Ok(())) => {}
            Poll::Ready(Err(error)) => panic!("a plugin surface could not start: {error}"),
            Poll::Pending => panic!("a plugin surface starts without waiting"),
        }
        let runner = launched
            .take()
            .expect("the plugin adapter hands its runner back once it has started");
        Self {
            runner,
            platform: HostPlatform::new(host),
            surface,
            described: None,
        }
    }

    pub fn context(&self) -> &Context {
        self.runner.context()
    }

    pub fn ratio(&self, region: &Region) -> f32 {
        let context = self.runner.context();
        context.set_pixels_per_point(region.scale_factor);
        region.scale_factor / context.pixels_per_point()
    }

    pub fn node_action(&mut self, request: ActionRequest) {
        self.runner.accessibility_action(request);
    }

    pub fn run_action(&mut self, id: &str) {
        self.runner.context().request_action(id.to_owned());
    }

    pub fn input(&mut self, region: &Region, event: &TimedInput) {
        self.runner.push(input::translate(region, event));
        if let Some(at) = input::time(event) {
            self.runner.push(Incoming::Time(at));
        }
    }

    pub fn update(&mut self, region: &Region) -> Frame {
        self.runner
            .reports()
            .system_preferences(block_editor_plugin::host_preferences().into());
        let [width, height] = region.pixels;
        let resized = {
            let mut surface = self.surface.borrow_mut();
            surface.age = region.age;
            surface.laid = region.rect;
            surface.screens = region
                .monitors
                .iter()
                .map(|monitor| {
                    Screen::new(&monitor.id, &monitor.name, monitor.rect)
                        .scaled(region.scale_factor)
                })
                .collect();
            let resized = surface.size != Some((width, height));
            surface.size = Some((width, height));
            resized
        };
        if resized {
            self.runner.reports().resized(width, height);
        }
        self.runner.reports().scaled(region.scale_factor);
        self.runner.set_occluded(!region.seen);
        if let Some(now) = block_editor_plugin::frame_time() {
            self.runner.context().set_clock(now);
            beui_core::timer::pin_wall_clock(Some((now, block_editor_plugin::wall_clock())));
        }
        let scale = region.scale_factor;
        let rect = region.rect;
        let area = SafeArea {
            left: rect.min.x * scale,
            top: rect.min.y * scale,
            right: width as f32 - rect.max.x * scale,
            bottom: height as f32 - rect.max.y * scale,
        };
        if self.described != region.describe {
            self.described = region.describe;
            match region.describe {
                Some(_) => self.runner.accessibility_requested(),
                None => self.runner.accessibility_deactivated(),
            }
            self.runner
                .context()
                .set_automated(region.describe.is_some());
        }
        let Some(frame) = self.runner.frame(&mut self.platform, area) else {
            return Frame {
                stopped: self.runner.exited().then(|| match self.runner.failure() {
                    Some(failure) => block_plugin_api::Stopped::Failed(failure.to_owned()),
                    None => block_plugin_api::Stopped::Closed,
                }),
                ..Frame::default()
            };
        };
        let description = region.describe.map(|_| self.describe(region));
        let mut frame = frame_for(self.runner.window(), &frame, description);
        frame.changed |= self.surface.borrow().unpainted();
        frame
    }

    fn describe(&mut self, region: &Region) -> Description {
        let tree = self
            .platform
            .take_tree()
            .map(|tree| self.placed(region, tree));
        let actions = self
            .runner
            .output()
            .map(|output| {
                output
                    .actions
                    .iter()
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

    fn placed(&self, region: &Region, mut tree: TreeUpdate) -> TreeUpdate {
        let scale = self.runner.context().pixels_per_point() / region.scale_factor;
        let origin = region.rect.min;
        let placed = Affine::translate((-f64::from(origin.x), -f64::from(origin.y)))
            * Affine::scale(scale.into());
        let root = tree
            .tree
            .as_ref()
            .map_or(beui_core::accessibility::WINDOW_NODE, |tree| tree.root);
        for (id, node) in &mut tree.nodes {
            if *id == root {
                node.set_transform(placed);
            }
        }
        tree
    }

    pub fn to_region(&self, region: &Region, rect: Rect) -> Rect {
        let context = self.runner.context();
        rect.scaled(context.pixels_per_point() / region.scale_factor)
    }

    pub fn painting(&self) -> Option<beui_core::painting::Painting> {
        let output = self.runner.output()?;
        let (width, height) = self.surface.borrow().size?;
        let pixels_per_point = output.pixels_per_point();
        beui_core::painting::painting(
            output.main(),
            beui_core::geometry::vec2(
                width as f32 / pixels_per_point,
                height as f32 / pixels_per_point,
            ),
            pixels_per_point,
            output.main().window.appearance.clear_color,
            beui_core::painting::Punches::Erase,
        )
        .ok()
    }

    pub fn overflows(&self) -> Vec<String> {
        self.runner
            .output()
            .map(|output| output.overflows.clone())
            .unwrap_or_default()
    }

    pub fn take_output(&mut self) -> Option<FrameOutput> {
        self.runner.take_output()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn paint(&mut self, target: &PaintTarget<'_>) -> Vec<SurfaceRect> {
        self.surface.borrow_mut().aim(target);
        self.runner.present();
        self.surface.borrow_mut().take_drawn()
    }
}

fn frame_for(
    window: &WindowState,
    frame: &beui_core::runner::Frame,
    description: Option<Description>,
) -> Frame {
    Frame {
        changed: frame.pending,
        repaint_after: frame.wait(),
        animating: frame.animating,
        content: None,
        painted: Vec::new(),
        floating: Vec::new(),
        drawing: Vec::new(),
        every_sample: false,
        claims: Vec::new(),
        window: window.requests.clone(),
        description,
        stopped: None,
    }
}

#[cfg(test)]
mod tests;
