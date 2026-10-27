use crate::window::WindowId;

#[derive(Clone, Debug, PartialEq)]
pub struct ActionInfo {
    pub id: String,
    pub label: String,
    pub shortcut: Option<String>,
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
