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
        window: beui_core::window::WindowId,
        at: beui_core::geometry::Pos2,
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
