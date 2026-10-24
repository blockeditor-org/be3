#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NamedIcon {
    pub name: &'static str,
    pub glyph: &'static str,
    pub tags: &'static str,
}

macro_rules! icons {
    ($($constant:ident $name:literal $codepoint:literal $tags:literal;)*) => {
        $(pub const $constant: &str = $codepoint;)*

        pub const ALL: &[$crate::icons::NamedIcon] = &[$(
            $crate::icons::NamedIcon { name: $name, glyph: $constant, tags: $tags },
        )*];
    };
}

mod list;

pub use list::*;

pub fn named(name: &str) -> Option<&'static str> {
    ALL.binary_search_by(|icon| icon.name.cmp(name))
        .ok()
        .map(|found| ALL[found].glyph)
}

#[cfg(test)]
mod tests;
