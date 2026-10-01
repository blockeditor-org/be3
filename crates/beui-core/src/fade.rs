use crate::geometry::{Pos2, Rect, Vec2};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Fade {
    pub rect: Rect,
    pub widths: [f32; 4],
}

impl Fade {
    pub const NONE: Self = Self {
        rect: Rect::EVERYTHING,
        widths: [0.0; 4],
    };

    pub fn new(rect: Rect, widths: [f32; 4]) -> Self {
        let widths = widths.map(|width| match width.is_finite() {
            true => width.max(0.0),
            false => 0.0,
        });
        match widths == [0.0; 4] {
            true => Self::NONE,
            false => Self { rect, widths },
        }
    }

    pub fn is_none(self) -> bool {
        self.widths == [0.0; 4]
    }

    pub fn translate(self, by: Vec2) -> Self {
        match self.is_none() {
            true => self,
            false => Self {
                rect: self.rect.translate(by),
                widths: self.widths,
            },
        }
    }

    pub fn scaled(self, factor: f32, origin: Vec2) -> Self {
        match self.is_none() {
            true => self,
            false => Self {
                rect: Rect::from_min_max(
                    Pos2::new(self.rect.min.x * factor, self.rect.min.y * factor) + origin,
                    Pos2::new(self.rect.max.x * factor, self.rect.max.y * factor) + origin,
                ),
                widths: self.widths.map(|width| width * factor),
            },
        }
    }

    pub fn within(self, inner: Self) -> Self {
        if inner.is_none() {
            return self;
        }
        if self.is_none() {
            return inner;
        }
        let outer = self.sides();
        let inner = inner.sides();
        let mut sides = [(0.0, 0.0); 4];
        for (index, side) in sides.iter_mut().enumerate() {
            let inward = match index < 2 {
                true => 1.0,
                false => -1.0,
            };
            let reach = |(edge, width): (f32, f32)| (edge + width * inward) * inward;
            *side = match (outer[index], inner[index]) {
                (outer, inner) if outer.1 == 0.0 => inner,
                (outer, inner) if inner.1 == 0.0 => outer,
                (outer, inner) if reach(outer) > reach(inner) => outer,
                (_, inner) => inner,
            };
        }
        Self {
            rect: Rect::from_min_max(
                Pos2::new(sides[0].0, sides[1].0),
                Pos2::new(sides[2].0, sides[3].0),
            ),
            widths: sides.map(|(_, width)| width),
        }
    }

    pub fn bands(self) -> Vec<Rect> {
        let Self { rect, widths } = self;
        let [left, top, right, bottom] = widths;
        let bands = [
            (
                left,
                Rect::from_min_max(rect.min, Pos2::new(rect.min.x + left, rect.max.y)),
            ),
            (
                top,
                Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.min.y + top)),
            ),
            (
                right,
                Rect::from_min_max(Pos2::new(rect.max.x - right, rect.min.y), rect.max),
            ),
            (
                bottom,
                Rect::from_min_max(Pos2::new(rect.min.x, rect.max.y - bottom), rect.max),
            ),
        ];
        bands
            .into_iter()
            .filter(|(width, _)| *width > 0.0)
            .map(|(_, band)| band)
            .collect()
    }

    fn sides(self) -> [(f32, f32); 4] {
        [
            (self.rect.min.x, self.widths[0]),
            (self.rect.min.y, self.widths[1]),
            (self.rect.max.x, self.widths[2]),
            (self.rect.max.y, self.widths[3]),
        ]
    }
}
