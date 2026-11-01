use beui::{Pos2, Vec2, pos2, vec2};

pub use ui_types::Rotation;

pub(crate) trait Turn {
    fn turned(self, size: (u32, u32)) -> (u32, u32);
    fn to_panel(self, picture: Vec2, at: Pos2) -> Pos2;
    fn to_picture(self, picture: Vec2, at: Pos2) -> Pos2;
    fn direction_to_picture(self, direction: Vec2) -> Vec2;
    fn panel_size(self, picture: Vec2) -> Vec2;
    fn rect_to_panel(self, picture: Vec2, rect: [f32; 4]) -> [f32; 4];
}

impl Turn for Rotation {
    fn turned(self, (width, height): (u32, u32)) -> (u32, u32) {
        match self.sideways() {
            true => (height, width),
            false => (width, height),
        }
    }

    fn to_panel(self, picture: Vec2, at: Pos2) -> Pos2 {
        let (width, height) = (picture.x, picture.y);
        match self {
            Self::Normal => at,
            Self::Clockwise90 => pos2(height - at.y, at.x),
            Self::Clockwise180 => pos2(width - at.x, height - at.y),
            Self::Clockwise270 => pos2(at.y, width - at.x),
        }
    }

    fn to_picture(self, picture: Vec2, at: Pos2) -> Pos2 {
        let (width, height) = (picture.x, picture.y);
        match self {
            Self::Normal => at,
            Self::Clockwise90 => pos2(at.y, height - at.x),
            Self::Clockwise180 => pos2(width - at.x, height - at.y),
            Self::Clockwise270 => pos2(width - at.y, at.x),
        }
    }

    fn direction_to_picture(self, direction: Vec2) -> Vec2 {
        match self {
            Self::Normal => direction,
            Self::Clockwise90 => vec2(direction.y, -direction.x),
            Self::Clockwise180 => -direction,
            Self::Clockwise270 => vec2(-direction.y, direction.x),
        }
    }

    fn panel_size(self, picture: Vec2) -> Vec2 {
        match self.sideways() {
            true => vec2(picture.y, picture.x),
            false => picture,
        }
    }

    fn rect_to_panel(self, picture: Vec2, rect: [f32; 4]) -> [f32; 4] {
        let [left, top, right, bottom] = rect;
        let a = self.to_panel(picture, pos2(left, top));
        let b = self.to_panel(picture, pos2(right, bottom));
        [a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y)]
    }
}

#[cfg(test)]
mod tests;
