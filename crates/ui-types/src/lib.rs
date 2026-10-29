pub use geometry;

be_ops_bindgen::generate!({
    path: "wit",
    world: "ui",
    with: {
        "vec2": geometry::Vec2,
        "pos2": geometry::Pos2,
        "rect": geometry::Rect,
        "size": usize,
        "range": core::ops::Range<usize>,
        "window-id": crate::window::WindowId,
        "host-placement": crate::window::HostPlacement,
    },
    copy: ["vec2", "pos2", "rect", "size", "window-id"],
    default: [
        "file-filter", "modifiers", "back-edge", "ime-text", "cursor-icon", "pen", "keystroke",
        "window-requests", "window-role",
    ],
    inline: [
        "back-gesture-started", "ime-event-replace-text", "ime-event-delete-surrounding",
        "ime-event-set-selection", "keyboard-shown", "input-pointer-button", "input-touch",
        "input-pen", "input-intercepted-key", "window-placement-over", "window-request-open",
        "window-request-begin-resize", "window-request-resize", "window-report-resized",
        "window-report-placed", "window-report-rastered", "window-report-moved",
        "window-report-focused",
    ],
});

pub mod clipboard;
pub mod files;
pub mod input;
pub mod window;

pub use clipboard::{PasteKind, Pasted};
pub use files::{FileFilter, FilePick, FileSave, PickedFile, SavedFile};
pub use input::{
    BackEdge, BackGesture, CursorIcon, ImeEvent, ImeText, Input, Key, KeyChord, KeyPress, Keyboard,
    Keystroke, Modifiers, Pen, PenPhase, PointerButton, ScrollDelta, TouchId, TouchPhase,
};
pub use ui_preferences::{
    CanvasTouch, DisplayMode, InputDevice, Motion, PointerSetting, PointerSettings, PointerSpeed,
    PreferenceLayer, Preferences, Rotation, ThemeKind,
};
pub use window::{
    HostPlacement, ImeArea, WindowEdge, WindowId, WindowPlacement, WindowReport, WindowRequest,
    WindowRequests, WindowRole,
};
