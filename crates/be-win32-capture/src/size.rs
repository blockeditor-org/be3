#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub fn of(width: i32, height: i32) -> Option<Size> {
        (width > 0 && height > 0).then(|| Size {
            width: width as u32,
            height: height as u32,
        })
    }
}

pub fn copied(content: Size, surface: Size) -> Size {
    Size {
        width: content.width.min(surface.width),
        height: content.height.min(surface.height),
    }
}

pub fn pool_after(pool: Size, content: Option<Size>) -> Option<Size> {
    content.filter(|&content| content != pool)
}
