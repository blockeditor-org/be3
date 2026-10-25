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
        "pointer-speed": crate::pointer::PointerSpeed,
    },
    copy: ["vec2", "pos2", "rect", "size", "pointer-speed"],
    default: [
        "rotation", "file-filter", "modifiers", "back-edge", "ime-text", "cursor-icon", "pen",
        "keystroke", "pointer-settings", "theme-kind", "motion", "canvas-touch", "preferences",
        "preference-layer", "window-requests",
    ],
    inline: [
        "back-gesture-started", "ime-event-replace-text", "ime-event-delete-surrounding",
        "ime-event-set-selection", "keyboard-shown", "input-pointer-button", "input-touch",
        "input-pen", "input-intercepted-key",
    ],
    lenient: ["preference-layer"],
    ordered: ["input-device"],
});

pub mod clipboard;
pub mod display;
pub mod files;
pub mod input;
pub mod pointer;
pub mod preferences;
pub mod window;

pub use clipboard::{PasteKind, Pasted};
pub use display::{DisplayMode, Rotation};
pub use files::{FileFilter, FilePick, FileSave, PickedFile, SavedFile};
pub use input::{
    BackEdge, BackGesture, CursorIcon, ImeEvent, ImeText, Input, Key, KeyChord, KeyPress, Keyboard,
    Keystroke, Modifiers, Pen, PenPhase, PointerButton, ScrollDelta, TouchId, TouchPhase,
};
pub use pointer::{InputDevice, PointerSetting, PointerSettings, PointerSpeed};
pub use preferences::{CanvasTouch, Motion, PreferenceLayer, Preferences, ThemeKind};
pub use window::{ImeArea, WindowRequests};
