use std::time::{Duration, Instant};

use crate::app::Setup;
use crate::context::{FrameOutput, WindowOutput};
use crate::geometry::Vec2;

use super::{Adapter, Incoming, Launch, Running};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct HostView {
    pub platform: Vec2,
    pub pixels_per_point: f32,
    pub now: Instant,
}

pub struct Hosted {
    pub window: WindowOutput,
    pub repaint_after: Duration,
}

pub trait WindowHost {
    fn setup(&mut self, _setup: &Setup) {}

    fn pace(&mut self) -> Option<Instant> {
        None
    }

    fn input(&mut self, incoming: &mut Vec<Incoming>, view: &HostView);

    fn frame(&mut self, output: &FrameOutput, view: &HostView) -> Hosted;
}

pub struct HostedAdapter {
    pub adapter: Box<dyn Adapter>,
    pub host: Box<dyn WindowHost>,
}

impl Adapter for HostedAdapter {
    fn name(&self) -> &'static str {
        self.adapter.name()
    }

    fn run(self: Box<Self>, mut launch: Launch) -> Running {
        launch.host = Some(self.host);
        self.adapter.run(launch)
    }
}
