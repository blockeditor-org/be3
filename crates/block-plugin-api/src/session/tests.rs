use super::*;
use crate::{
    DEFAULT_SURFACE_SIDE, Hello, HelloAccepted, Modifiers, PluginIdentity, PointerButton,
    SurfaceFormat, SurfaceSpec, SurfaceSupport, Theme, WheelUnit, encode_frame,
};

fn session() -> HostSession {
    HostSession::new(
        "BE3",
        Some(SurfaceSpec {
            format: SurfaceFormat::Rgba8Unorm,
            max_side: DEFAULT_SURFACE_SIDE,
        }),
        Theme { dark: true },
    )
}

fn hello() -> Message {
    Message::Hello(Hello {
        version: PROTOCOL_VERSION,
        plugin: PluginIdentity {
            id: "demo".into(),
            name: "Plugin Demo".into(),
            version: "1".into(),
        },
        surface: SurfaceSupport::Texture,
    })
}

fn running_session() -> HostSession {
    let mut session = session();
    session.start(0);
    session.receive_frame(&encode_frame(&hello()).unwrap(), 1);
    session.next_outbound();
    session
}

fn screens(request_id: u64) -> Message {
    Message::Screens(crate::ScreenSet {
        request_id,
        screens: Vec::new(),
    })
}

fn input(event: InputEvent) -> Message {
    Message::Input(InputBatch {
        screen: crate::ScreenId(7),
        events: vec![event],
    })
}

mod a_plugin_that_draws_nothing_is_granted_no_surface;
mod a_superseded_request_is_forgotten;
mod coalesced_zoom_gestures_multiply;
mod disconnect_fails_the_session;
mod malformed_payload_fails_the_session;
mod queue_saturation_preserves_ordered_input;
mod repeated_start_and_shutdown_are_clean;
mod request_timeout_fails_the_session;
mod superseded_events_are_coalesced;
mod touch_moves_are_coalesced;
