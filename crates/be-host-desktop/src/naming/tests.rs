use super::*;

mod a_window_is_named_after_its_program_then_its_title;

pub(super) fn named(program_id: &str, name: &str, title: &str) -> HostWindow {
    HostWindow {
        id: be_sdk_api::HostWindowId(1),
        kind: be_sdk_api::WindowKind::Window,
        shown: true,
        title: title.to_owned(),
        program_id: program_id.to_owned(),
        name: name.to_owned(),
        parent: None,
        modal: false,
        size: be_sdk_api::Vec2::new(320.0, 200.0),
        min_size: be_sdk_api::Vec2::ZERO,
        max_size: be_sdk_api::Vec2::ZERO,
        fullscreen: None,
        responding: true,
        focused: false,
        icon: None,
        raised: 0,
        rung: 0,
        origin: be_sdk_api::WindowOrigin::default(),
        note: None,
        menu: None,
    }
}
