use crate::components::root::Root;
use crate::reactive::Child;
use crate::reactive::Get;
use crate::reactive::Prop;
use crate::reactive::create_effect;
use crate::types::Color;
use crate::types::SystemBars;

pub(crate) fn root(
    clear_color: Prop<Color>,
    title: Option<Prop<String>>,
    system_bars: Prop<SystemBars>,
    children: Child,
) -> Root {
    let _ = system_bars;
    create_effect(move || beui_core_html::run::set_background(clear_color.get()));
    if let Some(title) = title {
        create_effect(move || beui_core_html::page::set_title(&title.get()));
    }
    Root::holding_child(children)
}
