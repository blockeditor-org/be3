use crate::reactive::{Get, Memo, create_effect};
use crate::types::ColorDefinitions;

pub fn apply(definitions: Memo<ColorDefinitions>) {
    create_effect(move || beui_core_html::colors::set_definitions(definitions.get()));
}

pub fn scheme() -> crate::types::ColorScheme {
    beui_core_html::page::color_scheme()
}
