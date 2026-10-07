#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Motion {
    #[default]
    Animated,
    Instant,
    Still,
}

impl Motion {
    pub fn animates(self) -> bool {
        self == Self::Animated
    }

    pub fn follows_gestures(self) -> bool {
        self != Self::Still
    }
}
