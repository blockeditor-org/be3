use be_win32::{Control, Entry, EntryKind, Snapshot};

use crate::mapping::PixelRect;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InjectError {
    SecureDesktop,
    Elevated,
    Gone,
    NotForeground,
    Refused { code: u32 },
}

impl std::fmt::Display for InjectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SecureDesktop => formatter.write_str("the secure desktop has the input"),
            Self::Elevated => formatter.write_str("the window runs as administrator"),
            Self::Gone => formatter.write_str("the window is gone"),
            Self::NotForeground => formatter.write_str("the window could not be brought forward"),
            Self::Refused { code } => write!(formatter, "Windows refused the input ({code})"),
        }
    }
}

impl std::error::Error for InjectError {}

impl InjectError {
    pub fn needs_administrator(self) -> bool {
        self == Self::Elevated
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Window { hwnd: u64, frame: PixelRect },
    Display { rect: PixelRect },
}

impl Target {
    pub fn frame(&self) -> PixelRect {
        match self {
            Self::Window { frame, .. } => *frame,
            Self::Display { rect } => *rect,
        }
    }
}

pub fn target(snapshot: &Snapshot, entry: &Entry) -> Result<Target, InjectError> {
    if snapshot.secure_desktop() {
        return Err(InjectError::SecureDesktop);
    }
    let frame = PixelRect::from_edges(
        entry.frame.left,
        entry.frame.top,
        entry.frame.right,
        entry.frame.bottom,
    );
    match entry.kind {
        EntryKind::Display => Ok(Target::Display { rect: frame }),
        EntryKind::Window if entry.control == Control::Elevated => Err(InjectError::Elevated),
        EntryKind::Window => Ok(Target::Window {
            hwnd: entry.handle.0,
            frame,
        }),
    }
}

#[cfg(test)]
mod tests;
