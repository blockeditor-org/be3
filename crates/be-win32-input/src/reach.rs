#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InjectError {
    SecureDesktop,
    Elevated,
    Unreachable,
    Gone,
    NotForeground,
    Refused { code: u32 },
}

impl std::fmt::Display for InjectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SecureDesktop => formatter.write_str("the secure desktop has the input"),
            Self::Elevated => formatter.write_str("the window runs as administrator"),
            Self::Unreachable => formatter.write_str("the window's process can't be opened"),
            Self::Gone => formatter.write_str("the window is gone"),
            Self::NotForeground => formatter.write_str("the window could not be brought forward"),
            Self::Refused { code } => write!(formatter, "Windows refused the input ({code})"),
        }
    }
}

impl std::error::Error for InjectError {}

impl InjectError {
    pub fn needs_administrator(self) -> bool {
        matches!(self, Self::Elevated | Self::Unreachable)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputDesktop {
    Default,
    Secure,
}

impl InputDesktop {
    pub fn named(name: Option<&str>) -> Self {
        match name {
            Some(name) if name.eq_ignore_ascii_case("Default") => Self::Default,
            _ => Self::Secure,
        }
    }

    pub fn reachable(self) -> Result<(), InjectError> {
        match self {
            Self::Default => Ok(()),
            Self::Secure => Err(InjectError::SecureDesktop),
        }
    }
}

pub const INTEGRITY_UNTRUSTED: u32 = 0x0000;
pub const INTEGRITY_LOW: u32 = 0x1000;
pub const INTEGRITY_MEDIUM: u32 = 0x2000;
pub const INTEGRITY_HIGH: u32 = 0x3000;
pub const INTEGRITY_SYSTEM: u32 = 0x4000;

pub fn reachable(ours: u32, theirs: Option<u32>) -> Result<(), InjectError> {
    match theirs {
        None => Err(InjectError::Unreachable),
        Some(theirs) if theirs > ours => Err(InjectError::Elevated),
        Some(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests;
