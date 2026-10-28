use crate::components::root::Root;
use crate::reactive::Child;
use crate::reactive::Get;
use crate::reactive::Prop;
use crate::reactive::create_effect;
use beui_core::color::Color;
use beui_core::current::with_document;
use beui_core::window::SystemBars;

pub(crate) fn root(
    clear_color: Prop<Color>,
    title: Option<Prop<String>>,
    system_bars: Prop<SystemBars>,
    children: Child,
) -> Root {
    let handle = crate::window::use_window();
    let window = move || match handle {
        None => Some(None),
        Some(handle) if handle.is_window() => Some(Some(handle.id)),
        Some(_) => None,
    };
    create_effect(move || {
        let color = crate::colors::resolve_color(clear_color.get());
        let Some(window) = window() else {
            return;
        };
        with_document(|document| {
            document.update_appearance(window, |appearance| appearance.clear_color = color)
        });
    });
    if let Some(title) = title {
        create_effect(move || {
            let title = title.get();
            let Some(window) = window() else {
                return;
            };
            with_document(|document| {
                document.update_appearance(window, |appearance| appearance.title = Some(title))
            });
        });
    }
    create_effect(move || {
        let bars = system_bars.get();
        let Some(window) = window() else {
            return;
        };
        with_document(|document| {
            document.update_appearance(window, |appearance| appearance.system_bars = bars)
        });
    });
    Root::holding_child(children)
}
