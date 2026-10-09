use crate::geometry::{Pos2, Rect};

#[derive(Clone, Debug, PartialEq)]
pub struct Screen {
    pub id: String,
    pub name: String,
    pub rect: Rect,
}

impl Screen {
    pub const WINDOW: &'static str = "window";

    pub fn new(id: impl Into<String>, name: impl Into<String>, rect: Rect) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            rect,
        }
    }

    pub fn window(rect: Rect) -> Self {
        Self::new(Self::WINDOW, "Window", rect)
    }

    pub fn scaled(&self, factor: f32) -> Self {
        Self {
            rect: self.rect.scaled(factor),
            ..self.clone()
        }
    }
}

pub fn shown_in(screens: &[Screen], area: Rect) -> Vec<Screen> {
    let shown: Vec<Screen> = screens
        .iter()
        .filter_map(|screen| {
            let rect = screen.rect.intersect(area);
            rect.is_positive().then(|| Screen {
                rect,
                ..screen.clone()
            })
        })
        .collect();
    match shown.is_empty() {
        true => vec![Screen::window(area)],
        false => shown,
    }
}

pub fn at(screens: &[Screen], pos: Pos2) -> Option<&Screen> {
    screens
        .iter()
        .find(|screen| screen.rect.contains(pos))
        .or_else(|| {
            screens.iter().min_by(|a, b| {
                distance(a.rect, pos).total_cmp(&distance(b.rect, pos))
            })
        })
}

pub fn under(screens: &[Screen], rect: Rect) -> Option<&Screen> {
    let overlap = |screen: &Screen| {
        let shared = screen.rect.intersect(rect);
        match shared.is_positive() {
            true => shared.width() * shared.height(),
            false => 0.0,
        }
    };
    screens
        .iter()
        .filter(|screen| overlap(screen) > 0.0)
        .max_by(|a, b| overlap(a).total_cmp(&overlap(b)))
        .or_else(|| at(screens, rect.center()))
}

fn distance(rect: Rect, pos: Pos2) -> f32 {
    let x = (rect.left() - pos.x).max(pos.x - rect.right()).max(0.0);
    let y = (rect.top() - pos.y).max(pos.y - rect.bottom()).max(0.0);
    x.hypot(y)
}

pub fn bounds(screens: &[Screen]) -> Rect {
    screens
        .iter()
        .map(|screen| screen.rect)
        .reduce(|all, rect| all.union(rect))
        .unwrap_or(Rect::ZERO)
}
