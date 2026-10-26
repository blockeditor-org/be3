use std::time::{Duration, Instant};

use crate::app::Setup;
use crate::context::{FrameOutput, WindowOutput};
use crate::geometry::Vec2;

use super::{Incoming, Launch, Platform, Running};

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

    fn input(&mut self, incoming: &mut Vec<Incoming>, view: &HostView);

    fn frame(&mut self, output: &FrameOutput, view: &HostView) -> Hosted;
}

pub struct HostedPlatform {
    pub platform: Box<dyn Platform>,
    pub host: Box<dyn WindowHost>,
}

impl Platform for HostedPlatform {
    fn name(&self) -> &'static str {
        self.platform.name()
    }

    fn run(self: Box<Self>, mut launch: Launch) -> Running {
        launch.host = Some(self.host);
        self.platform.run(launch)
    }
}
