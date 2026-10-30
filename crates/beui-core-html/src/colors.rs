use beui_types::{Color, Color32, ColorDefinitions, ColorScheme};
use reactive::{Get, ReadSignal, Set, Signal};

use crate::css::literal_color;

thread_local! {
    static DEFINED: Signal<ColorDefinitions> = crate::global_signal(ColorDefinitions::default());
}

fn defined() -> Signal<ColorDefinitions> {
    DEFINED.with(|defined| *defined)
}

pub fn set_definitions(definitions: ColorDefinitions) {
    let defined = defined();
    if defined.get_untracked() != definitions {
        defined.set(definitions);
    }
}

pub fn definitions() -> ReadSignal<ColorDefinitions> {
    defined().read_only()
}

pub fn resolve(color: Color) -> Color32 {
    if let Some(color) = color.as_literal() {
        return color;
    }
    let scheme = crate::page::color_scheme();
    defined().with(|definitions| color.resolve_in(definitions, scheme))
}

pub fn stylesheet(definitions: &ColorDefinitions) -> String {
    let mut sorted: Vec<_> = definitions.iter().collect();
    sorted.sort_by_key(|(var, _)| var.css_name());
    debug_assert!(
        sorted
            .windows(2)
            .all(|pair| pair[0].0.css_name() != pair[1].0.css_name()),
        "two colour variables hash to the same CSS name"
    );
    let block = |selector: &str, scheme: ColorScheme| {
        let mut out = format!("{selector} {{");
        for (var, colors) in &sorted {
            out.push_str(&format!(
                " --{}: {};",
                var.css_name(),
                literal_color(colors.get(scheme))
            ));
        }
        out.push_str(" }");
        out
    };
    format!(
        "{}\n@media (prefers-color-scheme: dark) {{ {} }}\n{}\n{}\n",
        block(".beui-root", ColorScheme::Light),
        block(".beui-root:not([data-scheme=light])", ColorScheme::Dark),
        block(".beui-root[data-scheme=light]", ColorScheme::Light),
        block(".beui-root[data-scheme=dark]", ColorScheme::Dark),
    )
}
