use std::path::PathBuf;

use smithay::input::keyboard::xkb;

use crate::state::KeyboardConfig;

const DEFAULT_LAYOUT: &str = "us";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedLayout {
    pub name: String,
    pub label: String,
}

pub fn named_layouts(keyboard: &KeyboardConfig) -> Option<Vec<NamedLayout>> {
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    let keymap = xkb::Keymap::new_from_names(
        &context,
        "",
        "",
        &keyboard.layout,
        &keyboard.variant,
        Some(keyboard.options.clone()).filter(|options| !options.is_empty()),
        xkb::KEYMAP_COMPILE_NO_FLAGS,
    )?;
    let rules = std::fs::read_to_string(rules_path()).unwrap_or_default();
    let codes: Vec<&str> = keyboard.layout.split(',').map(str::trim).collect();
    let variants: Vec<&str> = keyboard.variant.split(',').map(str::trim).collect();
    let short: Vec<String> = (0..keymap.num_layouts())
        .map(|index| {
            let code = codes
                .get(index as usize)
                .copied()
                .filter(|code| !code.is_empty())
                .unwrap_or(DEFAULT_LAYOUT);
            let variant = variants.get(index as usize).copied().unwrap_or_default();
            short_label(&rules, code, variant)
        })
        .collect();
    Some(
        (0..keymap.num_layouts())
            .map(|index| NamedLayout {
                name: keymap.layout_get_name(index).to_owned(),
                label: numbered(&short, index as usize),
            })
            .collect(),
    )
}

fn rules_path() -> PathBuf {
    let root = std::env::var_os("XKB_CONFIG_ROOT")
        .map_or_else(|| PathBuf::from("/usr/share/X11/xkb"), PathBuf::from);
    root.join("rules").join("evdev.xml")
}

fn numbered(labels: &[String], index: usize) -> String {
    let label = &labels[index];
    let same = labels.iter().filter(|other| *other == label).count();
    match same {
        1 => label.clone(),
        _ => {
            let nth = labels[..=index]
                .iter()
                .filter(|other| *other == label)
                .count();
            format!("{label}{nth}")
        }
    }
}

pub(crate) fn short_label(rules: &str, code: &str, variant: &str) -> String {
    let fallback = || code.chars().take(3).collect::<String>();
    let described = rules.split("<layout>").skip(1).find_map(|layout| {
        let (own, variants) = layout.split_once("<variantList>").unwrap_or((layout, ""));
        if item_tag(own, "name") != Some(code) {
            return None;
        }
        let of_variant = (!variant.is_empty())
            .then(|| {
                variants
                    .split("<variant>")
                    .skip(1)
                    .filter(|item| item_tag(item, "name") == Some(variant))
                    .find_map(|item| item_tag(item, "shortDescription"))
            })
            .flatten();
        Some(of_variant.or_else(|| item_tag(own, "shortDescription")))
    });
    let label = match described.flatten() {
        Some(short) if !short.is_empty() => short.to_owned(),
        _ => fallback(),
    };
    label.to_uppercase()
}

fn item_tag<'a>(item: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = item.find(&open)? + open.len();
    let length = item[start..].find(&close)?;
    Some(item[start..start + length].trim())
}

#[cfg(test)]
mod tests;
