mod input;
mod platform;
mod surface;

use std::cell::RefCell;
use std::rc::Rc;
use std::task::{Context as TaskContext, Poll, Waker as TaskWaker};

use beui_core::app::{App, SafeArea, Setup, Waker};
use beui_core::context::{Context, FrameOutput};
use beui_core::geometry::Rect;
use beui_core::input::Keyboard;
use beui_core::renderer::Loaded;
use beui_core::runner::{Adapter, Launch, RunOptions, Runner, Running};
use beui_core::screens::Screen;
use beui_core::window::WindowState;
use block_editor_plugin::{
    DescribedAction, DescribedNode, Description, EditorHost, Frame, InputEvent, NodeValue, Region,
    Toggled,
};
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
            runner.context().set_test_ids_published(true);
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
            options: RunOptions::new("plugin"),
            context,
            app: Box::new(app),
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

    pub fn node_action(&mut self, node: usize, action: &str, value: Option<NodeValue>) {
        let Some(action) = beui_core::app::accessibility_dump::action_named(action) else {
            return;
        };
        let Some(target) = self.runner.accessibility().and_then(|dump| {
            dump.lines()
                .iter()
                .filter(|line| line.role != beui_core::accesskit::Role::Window)
                .nth(node)
                .map(|line| line.id)
        }) else {
            return;
        };
        let data = value.map(|value| match value {
            NodeValue::Text(text) => beui_core::accesskit::ActionData::Value(text.into()),
            NodeValue::Number(number) => beui_core::accesskit::ActionData::NumericValue(number),
        });
        self.runner
            .context()
            .accessibility_action(beui_core::accesskit::ActionRequest {
                action,
                target_tree: beui_core::accesskit::TreeId::ROOT,
                target_node: target,
                data,
            });
    }

    pub fn run_action(&mut self, id: &str) {
        self.runner.context().request_action(id.to_owned());
    }

    pub fn input(&mut self, region: &Region, event: &InputEvent) {
        if let Some(input) = input::translate(region, event) {
            self.runner.push(input);
        }
    }

    pub fn update(&mut self, region: &Region) -> Frame {
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
        if let Some(now) = block_editor_plugin::frame_time() {
            self.runner.context().set_clock(now);
            beui_core::timer::pin_wall_clock(Some((now, block_editor_plugin::wall_clock())));
        }
        self.platform.deliver_picks();
        let scale = region.scale_factor;
        let rect = region.rect;
        let area = SafeArea {
            left: rect.min.x * scale,
            top: rect.min.y * scale,
            right: width as f32 - rect.max.x * scale,
            bottom: height as f32 - rect.max.y * scale,
        };
        if region.describe {
            self.runner.keep_accessibility();
            self.runner.context().set_automated(true);
        }
        let Some(frame) = self.runner.frame(&mut self.platform, area) else {
            return Frame::default();
        };
        let description = region.describe.then(|| self.describe(region));
        frame_for(self.runner.window(), &frame, description)
    }

    fn describe(&self, region: &Region) -> Description {
        let points = region.scale_factor.recip();
        let nodes = self
            .runner
            .accessibility()
            .map(|dump| {
                dump.lines()
                    .iter()
                    .filter(|line| line.role != beui_core::accesskit::Role::Window)
                    .map(|line| DescribedNode {
                        depth: line.depth.saturating_sub(1).min(usize::from(u16::MAX)) as u16,
                        role: format!("{:?}", line.role),
                        label: line.label.clone(),
                        value: line.value.clone(),
                        toggled: line.toggled.map(|toggled| match toggled {
                            beui_core::accesskit::Toggled::False => Toggled::Off,
                            beui_core::accesskit::Toggled::True => Toggled::On,
                            beui_core::accesskit::Toggled::Mixed => Toggled::Mixed,
                        }),
                        disabled: line.disabled,
                        focused: line.focused,
                        rect: line.bounds.map(|bounds| bounds.scaled(points)),
                        actions: line
                            .actions
                            .iter()
                            .map(|action| format!("{action:?}"))
                            .collect(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut test_ids: Vec<(String, Rect)> = self
            .runner
            .output()
            .map(|output| {
                output
                    .test_ids()
                    .iter()
                    .map(|(id, rect)| (id.clone(), self.to_region(region, *rect)))
                    .collect()
            })
            .unwrap_or_default();
        test_ids.sort_by(|a, b| a.0.cmp(&b.0));
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
        Description {
            nodes,
            test_ids,
            actions,
        }
    }

    pub fn to_region(&self, region: &Region, rect: Rect) -> Rect {
        let context = self.runner.context();
        rect.scaled(context.pixels_per_point() / region.scale_factor)
    }

    pub fn pointer_locked(&self) -> bool {
        self.runner.window().pointer_locked
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
        content: None,
        painted: Vec::new(),
        floating: Vec::new(),
        claims: Vec::new(),
        window: block_editor_plugin::WindowRequests {
            cursor: match window.touch_emulation {
                true => block_plugin_api::CursorIcon::Crosshair,
                false => beui_plugin_input::protocol_cursor(window.cursor),
            },
            ime: window.ime.as_ref().map(|area| block_editor_plugin::Ime {
                rect: area.rect,
                cursor: area.cursor,
                text: area.text.as_ref().map(beui_plugin_input::protocol_ime_text),
                keyboard: match area.keyboard {
                    Keyboard::Held => block_plugin_api::Keyboard::Held,
                    Keyboard::Shown { asks } => block_plugin_api::Keyboard::Shown { asks },
                },
            }),
            handles_back: window.handles_back,
            wants_keyboard: window.wants_keyboard,
            intercepted_keys: window
                .intercepted_keys
                .iter()
                .filter_map(|chord| beui_plugin_input::protocol_chord(*chord))
                .collect(),
            pastes: window
                .paste_kinds
                .iter()
                .map(|kind| beui_plugin_input::protocol_paste_kind(*kind))
                .collect(),
        },
        description,
    }
}

#[cfg(test)]
mod tests;
