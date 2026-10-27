use crate::reactive::{Get, Memo, create_effect, with_document};
use crate::types::ColorDefinitions;

pub fn apply(definitions: Memo<ColorDefinitions>) {
    create_effect(move || {
        let definitions = definitions.get();
        with_document(|document| document.set_color_definitions(definitions));
    });
}

pub fn scheme() -> crate::types::ColorScheme {
    with_document(|document| document.watch_theme()).get().into()
}
