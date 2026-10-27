use geometry::{Pos2, Rect, Vec2, pos2};

const ELLIPSE_HANDLE: f32 = 0.552_284_8;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PathElement {
    MoveTo(Pos2),
    LineTo(Pos2),
    QuadTo(Pos2, Pos2),
    CubicTo(Pos2, Pos2, Pos2),
    Close,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Path {
    elements: Vec<PathElement>,
}

impl Path {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn move_to(mut self, to: Pos2) -> Self {
        self.elements.push(PathElement::MoveTo(to));
        self
    }

    pub fn line_to(self, to: Pos2) -> Self {
        match self.elements.is_empty() {
            true => self.move_to(to),
            false => self.pushed(PathElement::LineTo(to)),
        }
    }

    pub fn quad_to(self, control: Pos2, to: Pos2) -> Self {
        self.started(control)
            .pushed(PathElement::QuadTo(control, to))
    }

    pub fn cubic_to(self, first: Pos2, second: Pos2, to: Pos2) -> Self {
        self.started(first)
            .pushed(PathElement::CubicTo(first, second, to))
    }

    pub fn close(self) -> Self {
        match self.elements.last() {
            None | Some(PathElement::Close) => self,
            Some(_) => self.pushed(PathElement::Close),
        }
    }

    pub fn polyline(points: impl IntoIterator<Item = Pos2>) -> Self {
        let mut points = points.into_iter();
        let Some(first) = points.next() else {
            return Self::new();
        };
        points.fold(Self::new().move_to(first), Self::line_to)
    }

    pub fn polygon(points: impl IntoIterator<Item = Pos2>) -> Self {
        Self::polyline(points).close()
    }

    pub fn ellipse(center: Pos2, radii: Vec2) -> Self {
        let handle = radii * ELLIPSE_HANDLE;
        let at = |x: f32, y: f32| pos2(center.x + x, center.y + y);
        Self::new()
            .move_to(at(radii.x, 0.0))
            .cubic_to(
                at(radii.x, handle.y),
                at(handle.x, radii.y),
                at(0.0, radii.y),
            )
            .cubic_to(
                at(-handle.x, radii.y),
                at(-radii.x, handle.y),
                at(-radii.x, 0.0),
            )
            .cubic_to(
                at(-radii.x, -handle.y),
                at(-handle.x, -radii.y),
                at(0.0, -radii.y),
            )
            .cubic_to(
                at(handle.x, -radii.y),
                at(radii.x, -handle.y),
                at(radii.x, 0.0),
            )
            .close()
    }

    pub fn elements(&self) -> &[PathElement] {
        &self.elements
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    pub fn translated(&self, by: Vec2) -> Self {
        let elements = self
            .elements
            .iter()
            .map(|element| match *element {
                PathElement::MoveTo(to) => PathElement::MoveTo(to + by),
                PathElement::LineTo(to) => PathElement::LineTo(to + by),
                PathElement::QuadTo(control, to) => PathElement::QuadTo(control + by, to + by),
                PathElement::CubicTo(first, second, to) => {
                    PathElement::CubicTo(first + by, second + by, to + by)
                }
                PathElement::Close => PathElement::Close,
            })
            .collect();
        Self { elements }
    }

    pub fn bounds(&self) -> Rect {
        self.elements
            .iter()
            .fold(Rect::NOTHING, |bounds, element| match *element {
                PathElement::MoveTo(point) | PathElement::LineTo(point) => {
                    bounds.union(Rect::from_min_max(point, point))
                }
                PathElement::QuadTo(control, to) => bounds.union(Rect::from_points(&[control, to])),
                PathElement::CubicTo(first, second, to) => {
                    bounds.union(Rect::from_points(&[first, second, to]))
                }
                PathElement::Close => bounds,
            })
    }

    fn started(self, at: Pos2) -> Self {
        match self.elements.is_empty() {
            true => self.move_to(at),
            false => self,
        }
    }

    fn pushed(mut self, element: PathElement) -> Self {
        self.elements.push(element);
        self
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum FillRule {
    #[default]
    NonZero,
    EvenOdd,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Stroke {
    pub width: f32,
    pub join: LineJoin,
    pub miter_limit: f32,
    pub cap: LineCap,
    pub dashes: Vec<f32>,
    pub dash_offset: f32,
}

impl Stroke {
    pub fn new(width: f32) -> Self {
        Self {
            width,
            join: LineJoin::Miter,
            miter_limit: 4.0,
            cap: LineCap::Butt,
            dashes: Vec::new(),
            dash_offset: 0.0,
        }
    }

    pub fn join(self, join: LineJoin) -> Self {
        Self { join, ..self }
    }

    pub fn miter_limit(self, miter_limit: f32) -> Self {
        Self {
            miter_limit: miter_limit.max(1.0),
            ..self
        }
    }

    pub fn cap(self, cap: LineCap) -> Self {
        Self { cap, ..self }
    }

    pub fn dashed(self, dashes: impl IntoIterator<Item = f32>, offset: f32) -> Self {
        Self {
            dashes: dashes.into_iter().collect(),
            dash_offset: offset,
            ..self
        }
    }

    pub fn reach(&self) -> f32 {
        let half = self.width / 2.0;
        let join = match self.join {
            LineJoin::Miter => self.miter_limit.max(1.0),
            LineJoin::Round | LineJoin::Bevel => 1.0,
        };
        let cap = match self.cap {
            LineCap::Square => std::f32::consts::SQRT_2,
            LineCap::Butt | LineCap::Round => 1.0,
        };
        half * join.max(cap)
    }

    pub fn is_dashed(&self) -> bool {
        self.dashes
            .iter()
            .all(|dash| dash.is_finite() && *dash >= 0.0)
            && self.dashes.iter().sum::<f32>() > 0.0
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum PathStyle {
    Fill(FillRule),
    Stroke(Stroke),
}

impl PathStyle {
    pub fn reach(&self) -> f32 {
        match self {
            Self::Fill(_) => 0.0,
            Self::Stroke(stroke) => stroke.reach(),
        }
    }
}
