use super::*;

mod a_window_is_named_after_its_program_then_its_title;

pub(super) fn named(program_id: &str, name: &str, title: &str) -> KnownWindow {
    KnownWindow {
        id: be_sdk_api::HostWindowId(1),
        title: title.to_owned(),
        program_id: program_id.to_owned(),
        name: name.to_owned(),
        size: be_sdk_api::Vec2::new(320.0, 200.0),
        ..KnownWindow::default()
    }
}
