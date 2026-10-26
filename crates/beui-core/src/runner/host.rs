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

    fn opens_windows(&self) -> bool {
        true
    }

    fn passes_main_through(&self) -> bool {
        false
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

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Hand {
    Left,
    Right,
}

impl Hand {
    pub const BOTH: [Self; 2] = [Self::Left, Self::Right];

    pub fn named(name: &str) -> Option<Self> {
        match name {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct HandPose {
    pub position: [f32; 3],
    pub orientation: [f32; 4],
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Controller {
    Pose {
        hand: Hand,
        aim: HandPose,
        grip: HandPose,
    },
    Toward {
        hand: Hand,
        window: crate::window::WindowId,
        at: crate::geometry::Pos2,
    },
    Trigger {
        hand: Hand,
        value: f32,
    },
    Squeeze {
        hand: Hand,
        value: f32,
    },
    Lost {
        hand: Hand,
    },
}

impl Controller {
    pub fn hand(&self) -> Hand {
        match self {
            Self::Pose { hand, .. }
            | Self::Toward { hand, .. }
            | Self::Trigger { hand, .. }
            | Self::Squeeze { hand, .. }
            | Self::Lost { hand } => *hand,
        }
    }
}
