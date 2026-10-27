use beui_tree::reactive::Callback;
use geometry::Vec2;
pub use ui_types::window::{
    WindowEdge, WindowId, WindowPlacement, WindowReport, WindowRequest, WindowRole,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum SystemBars {
    Light,
    #[default]
    Dark,
}

#[derive(Clone)]
pub struct WindowSpec {
    pub id: WindowId,
    pub size: Vec2,
    pub placement: WindowPlacement,
    pub title: Option<String>,
    pub notice: Option<String>,
    pub role: WindowRole,
    pub on_report: Callback<WindowReport>,
}

impl PartialEq for WindowSpec {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.size == other.size
            && self.placement == other.placement
            && self.title == other.title
            && self.notice == other.notice
            && self.role == other.role
    }
}
