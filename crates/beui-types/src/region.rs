use beui_tree::reactive::{IntoProp, Memo, Prop, ReadSignal, Signal};
use geometry::{Pos2, Rect, Vec2};

use crate::input::Modifiers;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HitRegion(Vec<Rect>);

impl HitRegion {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn rect(rect: Rect) -> Self {
        Self::new().with(rect)
    }

    pub fn with(mut self, rect: Rect) -> Self {
        if rect.is_positive() {
            self.0.push(rect);
        }
        self
    }

    pub fn union(mut self, other: HitRegion) -> Self {
        self.0.extend(other.0);
        self
    }

    pub fn without(self, cut: Rect) -> Self {
        if !cut.is_positive() {
            return self;
        }
        let mut pieces = Vec::with_capacity(self.0.len());
        for rect in self.0 {
            if !rect.intersects(cut) {
                pieces.push(rect);
                continue;
            }
            let inner = rect.intersect(cut);
            let bands = [
                Rect::from_min_max(rect.min, Pos2::new(rect.max.x, inner.min.y)),
                Rect::from_min_max(Pos2::new(rect.min.x, inner.max.y), rect.max),
                Rect::from_min_max(
                    Pos2::new(rect.min.x, inner.min.y),
                    Pos2::new(inner.min.x, inner.max.y),
                ),
                Rect::from_min_max(
                    Pos2::new(inner.max.x, inner.min.y),
                    Pos2::new(rect.max.x, inner.max.y),
                ),
            ];
            pieces.extend(bands.into_iter().filter(|band| band.is_positive()));
        }
        Self(pieces)
    }

    pub fn translate(self, by: Vec2) -> Self {
        Self(self.0.into_iter().map(|rect| rect.translate(by)).collect())
    }

    pub fn contains(&self, pos: Pos2) -> bool {
        self.0.iter().any(|rect| rect.contains_half_open(pos))
    }

    pub fn rects(&self) -> &[Rect] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Area {
    #[default]
    Nowhere,
    Whole,
    Within(HitRegion),
    CaretHandles,
}

impl Area {
    pub fn when(on: bool) -> Self {
        match on {
            true => Area::Whole,
            false => Area::Nowhere,
        }
    }
}

impl From<bool> for Area {
    fn from(on: bool) -> Self {
        Area::when(on)
    }
}

impl From<HitRegion> for Area {
    fn from(region: HitRegion) -> Self {
        Area::Within(region)
    }
}

impl IntoProp<Area> for bool {
    fn into_prop(self) -> Prop<Area> {
        Prop::Static(Area::when(self))
    }
}

impl IntoProp<Area> for HitRegion {
    fn into_prop(self) -> Prop<Area> {
        Prop::Static(Area::Within(self))
    }
}

impl IntoProp<Area> for Prop<bool> {
    fn into_prop(self) -> Prop<Area> {
        self.map(Area::when)
    }
}

impl IntoProp<Area> for Memo<bool> {
    fn into_prop(self) -> Prop<Area> {
        IntoProp::<bool>::into_prop(self).map(Area::when)
    }
}

impl IntoProp<Area> for Signal<bool> {
    fn into_prop(self) -> Prop<Area> {
        IntoProp::<bool>::into_prop(self).map(Area::when)
    }
}

impl IntoProp<Area> for ReadSignal<bool> {
    fn into_prop(self) -> Prop<Area> {
        IntoProp::<bool>::into_prop(self).map(Area::when)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Claim {
    pub modifiers: Modifiers,
    pub area: Area,
}

impl Claim {
    pub fn always(area: impl Into<Area>) -> Self {
        Self {
            modifiers: Modifiers::NONE,
            area: area.into(),
        }
    }

    pub fn holding(modifiers: Modifiers, area: impl Into<Area>) -> Self {
        Self {
            modifiers,
            area: area.into(),
        }
    }
}
