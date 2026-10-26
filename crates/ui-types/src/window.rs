pub use crate::types::{ImeArea, WindowRequests};

impl ImeArea {
    pub fn translated(self, by: geometry::Vec2) -> Self {
        Self {
            rect: self.rect.translate(by),
            cursor: self.cursor.translate(by),
            ..self
        }
    }
}
