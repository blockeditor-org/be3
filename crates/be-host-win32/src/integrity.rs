#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IntegrityLevel(pub u32);

impl IntegrityLevel {
    pub const UNTRUSTED: Self = Self(0x0000);
    pub const LOW: Self = Self(0x1000);
    pub const MEDIUM: Self = Self(0x2000);
    pub const MEDIUM_PLUS: Self = Self(0x2100);
    pub const HIGH: Self = Self(0x3000);
    pub const SYSTEM: Self = Self(0x4000);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Control {
    #[default]
    Full,
    Elevated,
}

pub fn control(own: Option<IntegrityLevel>, theirs: Option<IntegrityLevel>) -> Control {
    let own = own.unwrap_or(IntegrityLevel::MEDIUM);
    match theirs {
        Some(theirs) if theirs <= own => Control::Full,
        _ => Control::Elevated,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum InputDesktop {
    #[default]
    Default,
    Other(String),
    Unavailable,
}

impl InputDesktop {
    pub fn from_name(name: Option<&str>) -> Self {
        match name {
            Some(name) if name.eq_ignore_ascii_case("Default") => Self::Default,
            Some(name) => Self::Other(name.to_owned()),
            None => Self::Unavailable,
        }
    }

    pub fn secure(&self) -> bool {
        *self != Self::Default
    }
}
