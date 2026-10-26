pub use crate::core::root::Root;
use crate::reactive::Child;
use crate::reactive::Prop;
use beui_core::color::Color32;
pub use beui_core::window::SystemBars;
use beui_macros::component;

#[component]
pub fn Root(
    clear_color: Prop<Color32>,
    title: Option<Prop<String>>,
    #[prop(default = SystemBars::default())] system_bars: Prop<SystemBars>,
    children: Child,
) -> Root {
    crate::core::root(clear_color, title, system_bars, children)
}
