#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum LayerPolicy {
    Inert,
    Pass,
    BlocksParent,
    #[default]
    BlocksAll,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum LayerLevel {
    #[default]
    Normal,
    Top,
    Lock,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dismiss {
    Escape,
    Back,
    OutsidePress,
    WithParent,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Placement {
    At,
    Above,
    Over(u16),
    Around,
    BelowStart,
    RightStart,
    Center,
    Fill,
    InsideTop,
    InsideTopEnd,
    InsideBottom,
    InsideBottomEnd,
    CenterOn,
    HangFrom(u16),
    Popover { gap: u16, margin: u16 },
}
