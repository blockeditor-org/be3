be_ops_bindgen::generate!({
    path: "wit",
    world: "preferences",
    with: {
        "pointer-speed": crate::pointer::PointerSpeed,
    },
    copy: ["pointer-speed"],
    default: [
        "rotation", "pointer-settings", "theme-kind", "motion", "canvas-touch", "preferences",
        "preference-layer",
    ],
    lenient: ["preference-layer"],
    ordered: ["input-device"],
});

pub mod display;
pub mod pointer;
pub mod preferences;

pub use display::{DisplayMode, Rotation};
pub use pointer::{
    InputDevice, MAX_POINTER_SPEED, MIN_POINTER_SPEED, PointerSetting, PointerSettings, PointerSpeed,
};
pub use preferences::{CanvasTouch, Motion, PreferenceLayer, Preferences, ThemeKind};
