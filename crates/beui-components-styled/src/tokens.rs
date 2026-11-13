use beui_view::reactive::into_prop_via;
use beui_view::types::Corners;
use beui_view::types::{Extent, Sides};

/// A spacing step. `Custom` is for lengths that come from content (a measured
/// indent, a canvas), not for a gap the scale already has.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Space {
    None,
    Xs2,
    Xs,
    Sm,
    Md,
    Lg,
    Xl,
    Xl2,
    Xl3,
    Custom(f32),
}

impl Space {
    pub const ALL: [Space; 9] = [
        Space::None,
        Space::Xs2,
        Space::Xs,
        Space::Sm,
        Space::Md,
        Space::Lg,
        Space::Xl,
        Space::Xl2,
        Space::Xl3,
    ];

    pub const fn px(self) -> f32 {
        match self {
            Space::None => 0.0,
            Space::Xs2 => 2.0,
            Space::Xs => 4.0,
            Space::Sm => 6.0,
            Space::Md => 8.0,
            Space::Lg => 12.0,
            Space::Xl => 16.0,
            Space::Xl2 => 20.0,
            Space::Xl3 => 24.0,
            Space::Custom(px) => px,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Space::None => "none",
            Space::Xs2 => "2xs",
            Space::Xs => "xs",
            Space::Sm => "sm",
            Space::Md => "md",
            Space::Lg => "lg",
            Space::Xl => "xl",
            Space::Xl2 => "2xl",
            Space::Xl3 => "3xl",
            Space::Custom(_) => "custom",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Padding {
    pub left: Space,
    pub top: Space,
    pub right: Space,
    pub bottom: Space,
}

impl Padding {
    pub const ZERO: Padding = Padding::all(Space::None);

    pub const fn new(left: Space, top: Space, right: Space, bottom: Space) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub const fn all(space: Space) -> Self {
        Self::symmetric(space, space)
    }

    pub const fn symmetric(horizontal: Space, vertical: Space) -> Self {
        Self::new(horizontal, vertical, horizontal, vertical)
    }

    pub const fn horizontal(horizontal: Space) -> Self {
        Self::symmetric(horizontal, Space::None)
    }

    pub const fn vertical(vertical: Space) -> Self {
        Self::symmetric(Space::None, vertical)
    }

    pub const fn left(self, left: Space) -> Self {
        Self { left, ..self }
    }

    pub const fn top(self, top: Space) -> Self {
        Self { top, ..self }
    }

    pub const fn right(self, right: Space) -> Self {
        Self { right, ..self }
    }

    pub const fn bottom(self, bottom: Space) -> Self {
        Self { bottom, ..self }
    }

    pub const fn px(self) -> Sides {
        Sides::new(
            self.left.px(),
            self.top.px(),
            self.right.px(),
            self.bottom.px(),
        )
    }

    pub const fn custom(sides: Sides) -> Self {
        Self::new(
            Space::Custom(sides.left),
            Space::Custom(sides.top),
            Space::Custom(sides.right),
            Space::Custom(sides.bottom),
        )
    }
}

impl Default for Padding {
    fn default() -> Self {
        Self::ZERO
    }
}

/// A size step. `Custom` is for sizes from content.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Size {
    Track,
    Xs,
    Sm,
    Md,
    Lg,
    Xl,
    Xl2,
    Xl3,
    Row,
    Toolbar,
    Field,
    TouchRow,
    Menu,
    Popover,
    Dialog,
    Custom(f32),
}

impl Size {
    pub const ALL: [Size; 15] = [
        Size::Track,
        Size::Xs,
        Size::Sm,
        Size::Md,
        Size::Lg,
        Size::Xl,
        Size::Xl2,
        Size::Xl3,
        Size::Row,
        Size::Toolbar,
        Size::Field,
        Size::TouchRow,
        Size::Menu,
        Size::Popover,
        Size::Dialog,
    ];

    pub const fn px(self) -> f32 {
        match self {
            Size::Track => 6.0,
            Size::Xs => 16.0,
            Size::Sm => 20.0,
            Size::Md => 24.0,
            Size::Lg => 32.0,
            Size::Xl => 40.0,
            Size::Xl2 => 48.0,
            Size::Xl3 => 64.0,
            Size::Row => 22.0,
            Size::Toolbar => 30.0,
            Size::Field => 34.0,
            Size::TouchRow => 52.0,
            Size::Menu => 200.0,
            Size::Popover => 320.0,
            Size::Dialog => 360.0,
            Size::Custom(px) => px,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Size::Track => "track",
            Size::Xs => "xs",
            Size::Sm => "sm",
            Size::Md => "md",
            Size::Lg => "lg",
            Size::Xl => "xl",
            Size::Xl2 => "2xl",
            Size::Xl3 => "3xl",
            Size::Row => "row",
            Size::Toolbar => "toolbar",
            Size::Field => "field",
            Size::TouchRow => "touch row",
            Size::Menu => "menu",
            Size::Popover => "popover",
            Size::Dialog => "dialog",
            Size::Custom(_) => "custom",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Length {
    Extent(Extent),
    Size(Size),
    AtLeast(Size),
}

impl Length {
    pub const INTRINSIC: Length = Length::Extent(Extent::INTRINSIC);
    pub const FILL: Length = Length::Extent(Extent::fraction(1.0));

    pub const fn px(self) -> Extent {
        match self {
            Length::Extent(extent) => extent,
            Length::Size(size) => Extent::px(size.px()),
            Length::AtLeast(size) => Extent::at_least(size.px()),
        }
    }
}

impl Default for Length {
    fn default() -> Self {
        Self::INTRINSIC
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Stroke {
    None,
    Hairline,
    Focus,
    Custom(f32),
}

impl Stroke {
    pub const fn px(self) -> f32 {
        match self {
            Stroke::None => 0.0,
            Stroke::Hairline => 1.0,
            Stroke::Focus => 2.0,
            Stroke::Custom(px) => px,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CornerSet {
    pub top_left: bool,
    pub top_right: bool,
    pub bottom_right: bool,
    pub bottom_left: bool,
}

impl CornerSet {
    pub const ALL: CornerSet = CornerSet::new(true, true, true, true);
    pub const NONE: CornerSet = CornerSet::new(false, false, false, false);
    pub const TOP: CornerSet = CornerSet::new(true, true, false, false);
    pub const BOTTOM: CornerSet = CornerSet::new(false, false, true, true);
    pub const LEFT: CornerSet = CornerSet::new(true, false, false, true);
    pub const RIGHT: CornerSet = CornerSet::new(false, true, true, false);

    pub const fn new(
        top_left: bool,
        top_right: bool,
        bottom_right: bool,
        bottom_left: bool,
    ) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    pub const fn of(self, corners: Corners) -> Corners {
        const fn keep(kept: bool, radius: f32) -> f32 {
            if kept { radius } else { 0.0 }
        }
        Corners::new(
            keep(self.top_left, corners.top_left),
            keep(self.top_right, corners.top_right),
            keep(self.bottom_right, corners.bottom_right),
            keep(self.bottom_left, corners.bottom_left),
        )
    }
}

impl Default for CornerSet {
    fn default() -> Self {
        Self::ALL
    }
}

into_prop_via!(Padding, Space, Padding::all);
into_prop_via!(Length, Size, Length::Size);
into_prop_via!(Length, Extent, Length::Extent);
