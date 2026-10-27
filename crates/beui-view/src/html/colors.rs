use crate::reactive::{Get, Memo, Set, Signal, create_effect};
use crate::types::{ColorDefinitions, ColorScheme};

pub(crate) fn apply(definitions: Memo<ColorDefinitions>, scheme: Signal<ColorScheme>) {
    create_effect(move || beui_core_html::colors::set_definitions(definitions.get()));
    create_effect(move || scheme.set(beui_core_html::page::color_scheme()));
}
