pub use be_lib_geometry as geometry;

be_wasm_ops_bindgen::generate!({
    path: "wit",
    world: "ui",
    deps: ["../be-lib-ui-keys/wit"],
    uses: {
        "be3:keys/types@0.1.0": ::be_lib_ui_keys::types,
    },
    with: {
        "vec2": be_lib_geometry::Vec2,
        "pos2": be_lib_geometry::Pos2,
        "rect": be_lib_geometry::Rect,
        "size": usize,
        "range": core::ops::Range<usize>,
        "window-id": crate::window::WindowId,
        "block-ref": uuid::Uuid,
        "host-placement": crate::window::HostPlacement,
    },
    copy: ["vec2", "pos2", "rect", "size", "window-id", "block-ref"],
    default: [
        "file-filter", "back-edge", "ime-text", "cursor-icon", "pen", "keystroke",
        "window-requests", "window-role",
    ],
    inline: [
        "back-gesture-started", "ime-event-replace-text", "ime-event-delete-surrounding",
        "ime-event-set-selection", "keyboard-shown", "input-pointer-button", "input-touch",
        "input-pen", "input-intercepted-key", "touchpad-gesture-pinched", "window-placement-over", "window-request-open",
        "window-request-begin-resize", "window-request-resize", "window-request-place", "window-report-resized",
        "window-report-placed", "window-report-rastered", "window-report-moved",
        "window-report-focused",
    ],
});

pub mod clipboard;
pub mod files;
pub mod input;
pub mod window;

pub use be_lib_ui_preferences::{
    CanvasTouch, DisplayMode, InputDevice, Motion, PointerSetting, PointerSettings, PointerSpeed,
    PreferenceLayer, Preferences, Rotation, ThemeKind,
};
pub use clipboard::ClipItem;
pub use files::{FileFilter, FilePick, FileSave, PickedFile, SavedFile};
pub use input::{
    BackEdge, BackGesture, CursorIcon, ImeEvent, ImeText, Input, Key, KeyChord, KeyPress, Keyboard,
    Keystroke, Modifiers, Pen, PenPhase, PointerButton, ScrollDelta, TouchId, TouchPhase,
};
pub use window::{
    HostPlacement, ImeArea, WindowEdge, WindowId, WindowLevel, WindowPlacement, WindowReport,
    WindowRequest, WindowRequests, WindowRole,
};
