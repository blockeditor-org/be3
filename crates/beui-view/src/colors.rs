use std::cell::Cell;

use crate::reactive::{
    Get, IntoProp, Memo, Set, Signal, create_effect, create_memo, create_signal, on_cleanup,
    tree_local,
};
use crate::types::{Color, Color32, ColorDefinitions, ColorScheme};

#[derive(Clone, Copy)]
struct Registry {
    defined: Signal<Vec<(u64, ColorDefinitions)>>,
    merged: Memo<ColorDefinitions>,
}

thread_local! {
    static NEXT: Cell<u64> = const { Cell::new(0) };
}

fn registry() -> Registry {
    tree_local(|| {
        let defined = create_signal(Vec::<(u64, ColorDefinitions)>::new());
        let merged = create_memo(move || {
            defined.with(|defined| {
                let mut merged = ColorDefinitions::new();
                for (_, definitions) in defined {
                    merged.extend(definitions.iter().map(|(var, colors)| (*var, *colors)));
                }
                merged
            })
        });
        crate::core::colors::apply(merged);
        Registry { defined, merged }
    })
}

pub fn define_colors(definitions: impl IntoProp<ColorDefinitions>) {
    let definitions = definitions.into_prop();
    let registry = registry();
    let key = NEXT.with(|next| next.replace(next.get() + 1));
    create_effect(move || {
        let definitions = definitions.get();
        registry.defined.update(|defined| {
            match defined.iter_mut().find(|(held, _)| *held == key) {
                Some((_, held)) => *held = definitions,
                None => defined.push((key, definitions)),
            }
        });
    });
    on_cleanup(move || {
        if registry.defined.is_alive() {
            registry
                .defined
                .update(|defined| defined.retain(|(held, _)| *held != key));
        }
    });
}

pub fn color_definitions() -> Memo<ColorDefinitions> {
    registry().merged
}

pub fn color_scheme() -> ColorScheme {
    crate::core::colors::scheme()
}

pub fn resolve_color(color: Color) -> Color32 {
    if let Some(color) = color.as_literal() {
        return color;
    }
    let scheme = color_scheme();
    registry()
        .merged
        .with(|definitions| color.resolve_in(definitions, scheme))
}
