use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

pub use crate::types::{
    ImeArea, WindowEdge, WindowPlacement, WindowReport, WindowRequest, WindowRequests, WindowRole,
};

impl ImeArea {
    pub fn translated(self, by: geometry::Vec2) -> Self {
        Self {
            rect: self.rect.translate(by),
            cursor: self.cursor.translate(by),
            ..self
        }
    }
}

#[derive(
    Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct WindowId(u64);

static NEXT_WINDOW: AtomicU64 = AtomicU64::new(1);

impl WindowId {
    pub const MAIN: Self = Self(0);

    pub fn next() -> Self {
        Self(NEXT_WINDOW.fetch_add(1, Ordering::Relaxed))
    }

    pub fn is_main(self) -> bool {
        self == Self::MAIN
    }

    pub fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HostPlacement(pub String);

impl WindowRequest {
    pub fn window(&self) -> WindowId {
        match self {
            Self::Open { window, .. }
            | Self::BeginResize { window, .. }
            | Self::Resize { window, .. } => *window,
            Self::BeginMove(window) | Self::Close(window) => *window,
        }
    }
}

impl WindowReport {
    pub fn window(&self) -> WindowId {
        match self {
            Self::Resized { window, .. }
            | Self::Placed { window, .. }
            | Self::Rastered { window, .. }
            | Self::Moved { window, .. }
            | Self::Focused { window, .. } => *window,
            Self::Closed(window) => *window,
        }
    }
}
