use crate::file_picker::FilePickRequest;
use crate::geometry::Vec2;

pub const MAX_PIXELS: f32 = 8192.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowSize {
    pub size: Vec2,
    pub scale: f32,
}

impl WindowSize {
    pub fn parse(text: &str) -> Option<Self> {
        let (size, scale) = match text.split_once('@') {
            Some((size, scale)) => (
                size,
                scale
                    .parse::<f32>()
                    .ok()
                    .filter(|scale| scale.is_finite() && *scale > 0.0)?,
            ),
            None => (text, 1.0),
        };
        let (width, height) = size.split_once('x')?;
        let size = Vec2::new(width.parse().ok()?, height.parse().ok()?);
        Some(Self { size, scale }).filter(Self::fits)
    }

    pub fn fits(&self) -> bool {
        let fits = |length: f32| {
            (1.0..=MAX_PIXELS).contains(&length)
                && (1.0..=MAX_PIXELS).contains(&(length * self.scale))
        };
        fits(self.size.x) && fits(self.size.y)
    }
}

pub struct Simulation {
    pub window: WindowSize,
    pub screen: Vec2,
    pub fullscreen: bool,
    pub focused: bool,
    pub clipboard: Option<String>,
    pub picks: Vec<FilePickRequest>,
}

impl Simulation {
    pub fn new(window: WindowSize, screen: Option<Vec2>) -> Self {
        Self {
            window,
            screen: screen.unwrap_or(window.size),
            fullscreen: false,
            focused: true,
            clipboard: None,
            picks: Vec::new(),
        }
    }

    pub fn shown(&self) -> Vec2 {
        match self.fullscreen {
            true => self.screen,
            false => self.window.size,
        }
    }

    pub fn physical(&self) -> (u32, u32) {
        let shown = self.shown() * self.window.scale;
        (
            shown.x.round().max(1.0) as u32,
            shown.y.round().max(1.0) as u32,
        )
    }
}
