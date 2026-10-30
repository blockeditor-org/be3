use std::cell::Cell;
use std::rc::Rc;

use crate::reactive::{
    Get, IntoProp, Memo, Set, Signal, create_effect, create_memo, create_signal, on_cleanup,
    tree_local, untrack,
};
use crate::types::{Color, Color32, ColorDefinitions, ColorScheme};

#[derive(Clone, Copy)]
struct Registry {
    defined: Signal<Vec<(u64, ColorDefinitions)>>,
    merged: Memo<ColorDefinitions>,
    scheme: Signal<ColorScheme>,
}

thread_local! {
    static NEXT: Cell<u64> = const { Cell::new(0) };
    static CURRENT: Cell<Option<Registry>> = const { Cell::new(None) };
    static CHOSEN: Signal<u64> = {
        let owned = ::reactive::OwnedSignal::new(0);
        let handle = owned.handle();
        std::mem::forget(owned);
        handle
    };
}

fn registry() -> Registry {
    let registry = tree_local(|| {
        let defined = create_signal(Vec::<(u64, ColorDefinitions)>::new());
        let merged = create_memo(move || {
            defined.with(|defined| {
                let mut merged = ColorDefinitions::default();
                for (_, definitions) in defined {
                    merged.extend(definitions.iter().map(|(var, colors)| (*var, *colors)));
                }
                merged
            })
        });
        let scheme = create_signal(ColorScheme::default());
        crate::core::colors::apply(merged, scheme);
        Registry {
            defined,
            merged,
            scheme,
        }
    });
    let changed = CURRENT.with(|current| {
        current
            .replace(Some(registry))
            .is_none_or(|held| !held.defined.ptr_eq(&registry.defined))
    });
    if changed {
        CHOSEN.with(|chosen| chosen.update(|chosen| *chosen += 1));
    }
    registry
}

fn current() -> Option<Registry> {
    CHOSEN.with(|chosen| chosen.get());
    CURRENT
        .with(Cell::get)
        .filter(|registry| registry.defined.is_alive() && registry.scheme.is_alive())
}

pub fn define_colors(definitions: impl IntoProp<ColorDefinitions>) {
    let definitions = definitions.into_prop();
    let key = NEXT.with(|next| next.replace(next.get() + 1));
    let held = Rc::new(Cell::new(None::<Registry>));
    let defined = held.clone();
    create_effect(move || {
        let definitions = definitions.get();
        let registry = match defined.get() {
            Some(registry) => registry,
            None => {
                let registry = untrack(registry);
                defined.set(Some(registry));
                registry
            }
        };
        registry.defined.update(|defined| {
            match defined.iter_mut().find(|(held, _)| *held == key) {
                Some((_, held)) => *held = definitions,
                None => defined.push((key, definitions)),
            }
        });
    });
    on_cleanup(move || {
        if let Some(registry) = held.get()
            && registry.defined.is_alive()
        {
            registry
                .defined
                .update(|defined| defined.retain(|(held, _)| *held != key));
        }
    });
}

pub fn color_scheme() -> ColorScheme {
    current().map_or_else(ColorScheme::default, |registry| registry.scheme.get())
}

pub fn resolve_color(color: Color) -> Color32 {
    if let Some(color) = color.as_literal() {
        return color;
    }
    let Some(registry) = current() else {
        return color.resolve(|_| None);
    };
    let scheme = registry.scheme.get();
    registry
        .merged
        .with(|definitions| color.resolve_in(definitions, scheme))
}
