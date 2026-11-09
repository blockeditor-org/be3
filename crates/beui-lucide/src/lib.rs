/// An icon: its codepoint in the Lucide font. Only the generated list makes one,
/// so an icon prop cannot be handed text. Save icons by name ([`named`]), never by
/// codepoint: Lucide renumbers icons between releases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Glyph(&'static str);

impl Glyph {
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NamedIcon {
    pub name: &'static str,
    pub glyph: Glyph,
    pub tags: &'static str,
}

macro_rules! icons {
    ($($constant:ident $name:literal $codepoint:literal $tags:literal;)*) => {
        $(pub const $constant: $crate::Glyph = $crate::Glyph($codepoint);)*

        pub const ALL: &[$crate::NamedIcon] = &[$(
            $crate::NamedIcon { name: $name, glyph: $constant, tags: $tags },
        )*];

        impl $crate::Glyph {
            pub fn from_codepoint(codepoint: &str) -> Option<Self> {
                match codepoint {
                    $($codepoint => Some($constant),)*
                    _ => None,
                }
            }

            /// Its Lucide name, as saved data and the app protocol hold it ([`named`] reads
            /// it back).
            pub fn name(self) -> Option<&'static str> {
                match self.0 {
                    $($codepoint => Some($name),)*
                    _ => None,
                }
            }
        }
    };
}

mod list;

pub use list::*;

/// Looks an icon up by its Lucide name, as saved data holds it.
pub fn named(name: &str) -> Option<Glyph> {
    ALL.binary_search_by(|icon| icon.name.cmp(name))
        .ok()
        .map(|found| ALL[found].glyph)
}

#[cfg(test)]
mod tests;
