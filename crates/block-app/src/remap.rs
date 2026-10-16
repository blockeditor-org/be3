#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux::*;

#[cfg(not(target_os = "linux"))]
mod unsupported {
    use crate::ui::RemapsPrompt;

    pub(crate) fn load_approved(_approved: Option<Vec<u8>>) {}

    pub(crate) fn prompt() -> Option<RemapsPrompt> {
        None
    }

    pub(crate) fn turn_on() -> Option<Vec<u8>> {
        None
    }

    pub(crate) fn keep_off() {}
}
#[cfg(not(target_os = "linux"))]
pub(crate) use unsupported::*;
