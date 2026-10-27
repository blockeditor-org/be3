pub use crate::core::root::Root;
use crate::reactive::Child;
use crate::reactive::Prop;
use crate::types::Color;
pub use crate::types::SystemBars;
use beui_macros::component;

#[component]
pub fn Root(
    clear_color: Prop<Color>,
    title: Option<Prop<String>>,
    #[prop(default = SystemBars::default())] system_bars: Prop<SystemBars>,
    children: Child,
) -> Root {
    crate::core::root(clear_color, title, system_bars, children)
}
