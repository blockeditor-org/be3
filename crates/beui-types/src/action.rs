use crate::input::KeyChord;
use crate::window::WindowId;

/// An action as a document describes it outside itself: to a driver, a screen reader, or the
/// host that lists it in its palette and runs it by id.
#[derive(Clone, Debug, PartialEq)]
pub struct ActionInfo {
    pub id: String,
    pub label: String,
    /// The icon's name (`Glyph::as_str`).
    pub glyph: Option<String>,
    pub shortcut: Option<KeyChord>,
    pub enabled: bool,
    pub checked: Option<bool>,
    pub live: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActionGroup {
    pub name: String,
    pub window: Option<WindowId>,
    pub actions: Vec<ActionInfo>,
}
