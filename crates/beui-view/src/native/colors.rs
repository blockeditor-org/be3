use crate::reactive::{Get, Memo, Set, Signal, create_effect, with_document};
use crate::types::{ColorDefinitions, ColorScheme};

pub fn apply(definitions: Memo<ColorDefinitions>, scheme: Signal<ColorScheme>) {
    create_effect(move || {
        let definitions = definitions.get();
        with_document(|document| document.set_color_definitions(definitions));
    });
    create_effect(move || {
        let theme = with_document(|document| document.watch_theme());
        scheme.set(theme.get().into());
    });
}
