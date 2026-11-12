//! What the editor shows of key names, and the names it writes. Every name
//! comes from `be_lib_remap::names`, which the remapper reads too.

use be_lib_remap::edit::modifier_of;
use be_lib_remap::names::{KeyKind, Modifier, Source};
use be_lib_remap::rules::Wheel;

/// What an empty name reads as: a rule not chosen yet, or a key sent as nothing.
pub(super) const NOTHING: &str = "Nothing";

/// The name a person reads for a name as the TOML writes it: `Left Ctrl` for
/// `LeftCtrl`, `Wheel up` for `WheelUp`, `Shift` for `Shift`. A name nothing
/// reads is shown as written, and no name at all is `Nothing`.
pub(super) fn label(name: &str) -> String {
    if name.trim().is_empty() {
        return NOTHING.to_owned();
    }
    if let Some(wheel) = Wheel::named(name) {
        return wheel.label().to_owned();
    }
    if let Some(modifier) = Modifier::named(name.trim()) {
        return modifier.name().to_owned();
    }
    match Source::named(name) {
        Some(source) => source.label(),
        None => name.trim().to_owned(),
    }
}

/// The name to write for what a person typed: a name, an alias or a code. A
/// code keeps the `Key` it was typed with, which also reads codes past the
/// keys'. Only what is pressed can be a wheel turn.
pub(super) fn written(typed: &str, wheel: bool) -> Result<String, String> {
    let typed = typed.trim();
    if let Some(turn) = Wheel::named(typed) {
        return match wheel {
            true => Ok(turn.name().to_owned()),
            false => Err("A wheel turn can only be what you press, not what is sent.".to_owned()),
        };
    }
    let Some(source) = Source::named(typed) else {
        return Err(format!(
            "Nothing is called \u{201c}{typed}\u{201d}. Try a name like F13, or a code like Key 0x9e."
        ));
    };
    let name = source.name();
    let code_typed = typed
        .get(..3)
        .is_some_and(|head| head.eq_ignore_ascii_case("key"));
    Ok(match name.contains(':') && code_typed {
        true => format!("Key:{:#x}", source.code()),
        false => name,
    })
}

/// The keys All keys lists, by kind, matching a search on name or label.
pub(super) fn listed(query: &str) -> Vec<(KeyKind, Vec<String>)> {
    let query = query.trim().to_lowercase();
    KeyKind::ALL
        .into_iter()
        .map(|kind| {
            let mut keys: Vec<Source> = Source::all()
                .filter(|source| source.kind() == kind)
                .filter(|source| {
                    query.is_empty()
                        || source.name().to_lowercase().contains(&query)
                        || source.label().to_lowercase().contains(&query)
                })
                .collect();
            keys.sort_by_key(|source| order(*source));
            (kind, keys.into_iter().map(Source::name).collect::<Vec<_>>())
        })
        .filter(|(_, keys)| !keys.is_empty())
        .collect()
}

/// Letters before digits, digits in keyboard order, and the rest by code.
fn order(source: Source) -> (u8, String, u16) {
    let name = source.name();
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some(letter), None) if letter.is_ascii_alphabetic() => (0, name, 0),
        (Some(digit), None) if digit.is_ascii_digit() => (1, String::new(), source.code()),
        _ if source.kind() == KeyKind::Modifier => (2, String::new(), modifier_rank(source)),
        _ => (2, String::new(), source.code()),
    }
}

fn modifier_rank(source: Source) -> u16 {
    let side = u16::from(source.name().starts_with("Right"));
    let modifier = modifier_of(&source.name()).map_or(0, |modifier| modifier as u16);
    modifier * 2 + side
}

/// The mouse buttons, for the Mouse tab.
pub(super) fn buttons() -> Vec<String> {
    Source::all()
        .filter(|source| matches!(source, Source::Button(_)))
        .map(Source::name)
        .collect()
}

pub(super) fn wheels() -> Vec<String> {
    Wheel::ALL
        .into_iter()
        .map(|wheel| wheel.name().to_owned())
        .collect()
}

/// A part of a test id for a name.
pub(super) fn id_part(name: &str) -> String {
    name.to_lowercase().replace([':', ' '], "-")
}
