use beui_core::geometry::Pos2;
use beui_core::geometry::Vec2;
use beui_core::layer::LayerLevel;
use beui_core::node::NodeId;
use beui_core::window::WindowId;
use beui_core::window::WindowPlacement;
use beui_core::window::WindowReport;
use beui_core::window::WindowRole;
use beui_macros::component;
use crate::reactive::Callback;
use crate::reactive::Prop;
use crate::reactive::RenderFn;
pub use crate::core::window::{WindowHandle, use_window, opens_windows};

#[component]
pub fn Window(
    id: Option<WindowId>,
    size: Prop<Vec2>,
    #[prop(default = WindowPlacement::AtPointer)] placement: Prop<WindowPlacement>,
    #[prop(default = Pos2::ZERO)] at: Prop<Pos2>,
    #[prop(default = None)] title: Prop<Option<String>>,
    #[prop(default = WindowRole::Normal)] role: Prop<WindowRole>,
    #[prop(default = true)] open: Prop<bool>,
    #[prop(default = LayerLevel::Normal)] level: Prop<LayerLevel>,
    #[prop(default = false)] keep_alive: bool,
    on_report: Callback<WindowReport>,
    #[prop(children)] content: RenderFn<WindowHandle>
) -> NodeId {
    crate::core::window(id, size, placement, at, title, role, open, level, keep_alive, on_report, content)
}
