use beui_core::font::{FontFamily, FontId};

#[cfg(target_arch = "wasm32")]
mod measure;

#[cfg(target_arch = "wasm32")]
pub use measure::{BrowserFonts, watch};

const PROPORTIONAL: &str =
    "Ubuntu, system-ui, -apple-system, \"Segoe UI\", Roboto, \"Helvetica Neue\", Arial, sans-serif";
const MONOSPACE: &str = "Hack, ui-monospace, SFMono-Regular, Menlo, Consolas, \"DejaVu Sans Mono\", \"Liberation Mono\", monospace";
const ICONS: &str = "beui-icons";

pub fn css_family(family: FontFamily) -> &'static str {
    match family {
        FontFamily::Proportional => PROPORTIONAL,
        FontFamily::Monospace => MONOSPACE,
        FontFamily::Icons => ICONS,
    }
}

pub fn css_font(font: FontId) -> String {
    let style = match font.italic {
        true => "italic",
        false => "normal",
    };
    let weight = match (font.family, font.bold) {
        (FontFamily::Proportional, false) => 300,
        (_, false) => 400,
        (_, true) => 700,
    };
    format!(
        "{style} {weight} {}px {}",
        font.size,
        css_family(font.family)
    )
}

#[cfg(test)]
mod tests;
