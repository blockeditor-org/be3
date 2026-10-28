use beui_tree::reactive::{Get, IntoProp, Memo, Prop, ReadSignal, Signal};
use geometry::{Pos2, Rect, Vec2, pos2, vec2};

use crate::color::Color;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Horizontal,
    Vertical,
}

impl Direction {
    pub fn axes(self, main: f32, cross: f32) -> Vec2 {
        match self {
            Direction::Horizontal => vec2(main, cross),
            Direction::Vertical => vec2(cross, main),
        }
    }

    pub fn main(self, size: Vec2) -> f32 {
        self.main_and_cross(size).0
    }

    pub fn main_and_cross(self, size: Vec2) -> (f32, f32) {
        match self {
            Direction::Horizontal => (size.x, size.y),
            Direction::Vertical => (size.y, size.x),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Align {
    Start,
    Center,
    End,
    Stretch,
    Baseline,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum ItemSize {
    #[default]
    Intrinsic,
    Fixed(f32),
    Percent(f32),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Sizing {
    pub size: ItemSize,
    pub min: f32,
    pub max: f32,
    pub shrink: f32,
    pub align: Option<Align>,
    pub gap: Option<f32>,
}

impl Default for Sizing {
    fn default() -> Self {
        Self {
            size: ItemSize::Intrinsic,
            min: 0.0,
            max: f32::INFINITY,
            shrink: 0.0,
            align: None,
            gap: None,
        }
    }
}

impl From<ItemSize> for Sizing {
    fn from(size: ItemSize) -> Self {
        Self {
            size,
            ..Self::default()
        }
    }
}

impl Sizing {
    pub fn min(self, min: f32) -> Self {
        Self { min, ..self }
    }

    pub fn max(self, max: f32) -> Self {
        Self { max, ..self }
    }

    pub fn shrink(self, shrink: f32) -> Self {
        Self { shrink, ..self }
    }

    pub fn align(self, align: Align) -> Self {
        Self {
            align: Some(align),
            ..self
        }
    }

    pub fn gap(self, gap: f32) -> Self {
        Self {
            gap: Some(gap),
            ..self
        }
    }

    pub fn clamp(self, length: f32) -> f32 {
        length.min(self.max).max(self.min)
    }

    pub fn yields(self) -> bool {
        matches!(self.size, ItemSize::Percent(_)) || self.shrink > 0.0
    }
}

impl ItemSize {
    pub fn min(self, min: f32) -> Sizing {
        Sizing::from(self).min(min)
    }

    pub fn max(self, max: f32) -> Sizing {
        Sizing::from(self).max(max)
    }

    pub fn shrink(self, shrink: f32) -> Sizing {
        Sizing::from(self).shrink(shrink)
    }

    pub fn align(self, align: Align) -> Sizing {
        Sizing::from(self).align(align)
    }

    pub fn gap(self, gap: f32) -> Sizing {
        Sizing::from(self).gap(gap)
    }
}

impl IntoProp<Sizing> for ItemSize {
    fn into_prop(self) -> Prop<Sizing> {
        Prop::Static(self.into())
    }
}

impl IntoProp<Sizing> for Prop<ItemSize> {
    fn into_prop(self) -> Prop<Sizing> {
        self.map(Sizing::from)
    }
}

impl IntoProp<Sizing> for ReadSignal<ItemSize> {
    fn into_prop(self) -> Prop<Sizing> {
        Prop::dynamic(move || self.get().into())
    }
}

impl IntoProp<Sizing> for Signal<ItemSize> {
    fn into_prop(self) -> Prop<Sizing> {
        Prop::dynamic(move || self.get().into())
    }
}

impl IntoProp<Sizing> for Memo<ItemSize> {
    fn into_prop(self) -> Prop<Sizing> {
        Prop::dynamic(move || self.get().into())
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Outline {
    pub color: Color,
    pub width: f32,
    pub offset: f32,
}

impl Outline {
    pub fn new(color: impl Into<Color>, width: f32) -> Self {
        Self {
            color: color.into(),
            width,
            offset: 0.0,
        }
    }

    pub const fn offset(self, offset: f32) -> Self {
        Self { offset, ..self }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Align2 {
    pub horizontal: Align,
    pub vertical: Align,
}

impl Align2 {
    pub const STRETCH: Align2 = Align2::all(Align::Stretch);
    pub const CENTER: Align2 = Align2::all(Align::Center);
    pub const START: Align2 = Align2::all(Align::Start);

    pub const fn new(horizontal: Align, vertical: Align) -> Self {
        Self {
            horizontal,
            vertical,
        }
    }

    pub const fn all(align: Align) -> Self {
        Self::new(align, align)
    }

    pub const fn horizontal(horizontal: Align) -> Self {
        Self::new(horizontal, Align::Stretch)
    }

    pub const fn vertical(vertical: Align) -> Self {
        Self::new(Align::Stretch, vertical)
    }
}

impl Default for Align2 {
    fn default() -> Self {
        Self::STRETCH
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Sides {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl std::ops::Neg for Sides {
    type Output = Sides;

    fn neg(self) -> Sides {
        Sides::new(-self.left, -self.top, -self.right, -self.bottom)
    }
}

impl Sides {
    pub const ZERO: Sides = Sides::all(0.0);

    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub const fn all(amount: f32) -> Self {
        Self::symmetric(amount, amount)
    }

    pub const fn symmetric(horizontal: f32, vertical: f32) -> Self {
        Self::new(horizontal, vertical, horizontal, vertical)
    }

    pub const fn horizontal(horizontal: f32) -> Self {
        Self::symmetric(horizontal, 0.0)
    }

    pub const fn vertical(vertical: f32) -> Self {
        Self::symmetric(0.0, vertical)
    }

    pub const fn left(self, left: f32) -> Self {
        Self { left, ..self }
    }

    pub const fn top(self, top: f32) -> Self {
        Self { top, ..self }
    }

    pub const fn right(self, right: f32) -> Self {
        Self { right, ..self }
    }

    pub const fn bottom(self, bottom: f32) -> Self {
        Self { bottom, ..self }
    }

    pub fn start(self) -> Vec2 {
        vec2(self.left, self.top)
    }

    pub fn amount(self) -> Vec2 {
        vec2(self.left + self.right, self.top + self.bottom)
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Extent {
    pub exact: Option<f32>,
    pub fraction: Option<f32>,
    pub min: Option<f32>,
    pub max: Option<f32>,
}

impl Extent {
    pub const INTRINSIC: Extent = Extent {
        exact: None,
        fraction: None,
        min: None,
        max: None,
    };

    pub const fn px(length: f32) -> Self {
        Self {
            exact: Some(length),
            ..Self::INTRINSIC
        }
    }

    pub const fn fraction(fraction: f32) -> Self {
        Self {
            fraction: Some(fraction),
            ..Self::INTRINSIC
        }
    }

    pub const fn at_least(min: f32) -> Self {
        Self::INTRINSIC.min(min)
    }

    pub const fn at_most(max: f32) -> Self {
        Self::INTRINSIC.max(max)
    }

    pub const fn min(self, min: f32) -> Self {
        Self {
            min: Some(min),
            ..self
        }
    }

    pub const fn max(self, max: f32) -> Self {
        Self {
            max: Some(max),
            ..self
        }
    }

    pub fn scales_with(self, available: f32) -> bool {
        self.exact.is_none() && self.fraction.is_some() && available.is_finite()
    }

    pub fn within(self, available: f32, intrinsic: f32) -> f32 {
        let fraction = self
            .fraction
            .filter(|_| available.is_finite())
            .map(|fraction| fraction * available);
        let length = match (self.exact.or(fraction), self.max) {
            (Some(length), _) => length,
            (None, Some(_)) => available,
            (None, None) => intrinsic,
        };
        let length = self.max.map_or(length, |max| length.min(max));
        self.min.map_or(length, |min| length.max(min))
    }

    pub fn offered(self, available: f32) -> f32 {
        let length = self.exact.unwrap_or(available);
        self.max.map_or(length, |max| length.min(max))
    }

    pub fn offered_to_content(self, available: f32) -> f32 {
        let fraction = self
            .fraction
            .filter(|_| available.is_finite())
            .map(|fraction| fraction * available);
        let length = self.exact.or(fraction).unwrap_or(available);
        self.max.map_or(length, |max| length.min(max))
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Track {
    Fixed(f32),
    Intrinsic,
    Fraction(f32),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ScrollPosition {
    pub offset: f32,
    pub content: f32,
    pub viewport: f32,
}

impl ScrollPosition {
    pub const ZERO: Self = Self {
        offset: 0.0,
        content: 0.0,
        viewport: 0.0,
    };

    pub fn max_offset(&self) -> f32 {
        (self.content - self.viewport).max(0.0)
    }

    pub fn clamped(&self) -> f32 {
        self.offset.clamp(0.0, self.max_offset())
    }

    pub fn overscroll(&self) -> f32 {
        self.offset - self.clamped()
    }
}

impl Default for ScrollPosition {
    fn default() -> Self {
        Self::ZERO
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Corners {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl Corners {
    pub const ZERO: Corners = Corners::all(0.0);

    pub const fn new(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    pub const fn all(radius: f32) -> Self {
        Self::new(radius, radius, radius, radius)
    }

    pub const fn top(radius: f32) -> Self {
        Self::new(radius, radius, 0.0, 0.0)
    }

    pub const fn bottom(radius: f32) -> Self {
        Self::new(0.0, 0.0, radius, radius)
    }

    pub const fn left(radius: f32) -> Self {
        Self::new(radius, 0.0, 0.0, radius)
    }

    pub const fn right(radius: f32) -> Self {
        Self::new(0.0, radius, radius, 0.0)
    }

    pub fn fitted(self, rect: Rect) -> Self {
        let limit = (rect.width().min(rect.height()) / 2.0).max(0.0);
        Self::new(
            self.top_left.min(limit),
            self.top_right.min(limit),
            self.bottom_right.min(limit),
            self.bottom_left.min(limit),
        )
    }

    pub fn to_array(self) -> [f32; 4] {
        [
            self.top_left,
            self.top_right,
            self.bottom_right,
            self.bottom_left,
        ]
    }

    pub fn largest(self) -> f32 {
        self.to_array().into_iter().fold(0.0, f32::max)
    }

    pub fn scaled(self, scale: f32) -> Self {
        let [top_left, top_right, bottom_right, bottom_left] =
            self.to_array().map(|radius| radius * scale);
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }
}

impl From<f32> for Corners {
    fn from(radius: f32) -> Self {
        Self::all(radius)
    }
}

macro_rules! into_prop_via {
    ($target:ty, $value:ty, $convert:expr) => {
        impl IntoProp<$target> for $value {
            fn into_prop(self) -> Prop<$target> {
                Prop::Static(self).map($convert)
            }
        }

        impl IntoProp<$target> for Prop<$value> {
            fn into_prop(self) -> Prop<$target> {
                self.map($convert)
            }
        }

        impl IntoProp<$target> for Memo<$value> {
            fn into_prop(self) -> Prop<$target> {
                IntoProp::<$value>::into_prop(self).map($convert)
            }
        }

        impl IntoProp<$target> for ReadSignal<$value> {
            fn into_prop(self) -> Prop<$target> {
                IntoProp::<$value>::into_prop(self).map($convert)
            }
        }

        impl IntoProp<$target> for Signal<$value> {
            fn into_prop(self) -> Prop<$target> {
                IntoProp::<$value>::into_prop(self).map($convert)
            }
        }
    };
}

into_prop_via!(Extent, f32, Extent::px);
into_prop_via!(Extent, Option<f32>, |exact| Extent {
    exact,
    ..Extent::INTRINSIC
});
into_prop_via!(Sides, f32, Sides::all);
into_prop_via!(Corners, f32, Corners::all);
into_prop_via!(Align2, Align, Align2::all);

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CanvasView {
    pub origin: Pos2,
    pub scale: f32,
}

impl CanvasView {
    pub fn new(origin: Pos2, scale: f32) -> Self {
        Self { origin, scale }
    }

    pub fn to_screen(self, point: Pos2) -> Pos2 {
        pos2(
            self.origin.x + point.x * self.scale,
            self.origin.y + point.y * self.scale,
        )
    }

    pub fn to_canvas(self, point: Pos2) -> Pos2 {
        pos2(
            (point.x - self.origin.x) / self.scale,
            (point.y - self.origin.y) / self.scale,
        )
    }

    pub fn rect_to_screen(self, rect: Rect) -> Rect {
        Rect::from_min_max(self.to_screen(rect.min), self.to_screen(rect.max))
    }

    pub fn rect_to_canvas(self, rect: Rect) -> Rect {
        Rect::from_min_max(self.to_canvas(rect.min), self.to_canvas(rect.max))
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Measured {
    pub rect: Rect,
    pub size: Vec2,
}

pub const GLIDE_SPRING: crate::animation::Spring = crate::animation::Spring {
    stiffness: 500.0,
    damping: 42.0,
};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct GlideHold {
    pub at: Pos2,
    pub along: Option<Direction>,
}

#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub struct GlideMotion {
    pub displacement: Vec2,
    pub velocity: Vec2,
}

impl GlideMotion {
    pub fn moving(self) -> bool {
        self.displacement != Vec2::ZERO || self.velocity != Vec2::ZERO
    }

    pub fn step(&mut self, seconds: f32) -> bool {
        GLIDE_SPRING.advance(&mut self.displacement.x, &mut self.velocity.x, seconds);
        GLIDE_SPRING.advance(&mut self.displacement.y, &mut self.velocity.y, seconds);
        self.moving()
    }

    pub fn settle(&mut self) {
        *self = Self::default();
    }

    pub fn hold(&mut self, along: Option<Direction>, moved: Vec2) {
        self.displacement = match along {
            Some(Direction::Horizontal) => Vec2::new(moved.x, 0.0),
            Some(Direction::Vertical) => Vec2::new(0.0, moved.y),
            None => moved,
        };
        self.velocity = Vec2::ZERO;
    }
}
