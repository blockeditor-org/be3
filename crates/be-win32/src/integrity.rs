/// A token's mandatory integrity level: the last subauthority of its label's
/// SID.
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

/// Whether block-app can hook and inject into a window: UIPI stops both for
/// windows of a higher integrity level than its own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Control {
    #[default]
    Full,
    /// The window runs as administrator, or its process cannot be read.
    Elevated,
}

/// What the window's process can be told to do. A process whose level cannot
/// be read is one block-app was refused a look at, so cannot control either;
/// block-app's own level, unread, is taken as medium.
pub fn control(own: Option<IntegrityLevel>, theirs: Option<IntegrityLevel>) -> Control {
    let own = own.unwrap_or(IntegrityLevel::MEDIUM);
    match theirs {
        Some(theirs) if theirs <= own => Control::Full,
        _ => Control::Elevated,
    }
}

/// The desktop that receives input. Anything but `Default` is the secure
/// desktop (a UAC prompt, Ctrl+Alt+Del) or the lock screen: captures freeze
/// and input cannot reach it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum InputDesktop {
    #[default]
    Default,
    Other(String),
    /// `OpenInputDesktop` failed, which it does while the secure desktop is up.
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
