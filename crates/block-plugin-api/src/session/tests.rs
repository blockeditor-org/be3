use super::*;
use crate::{
    Hello, HelloAccepted, Modifiers, PluginIdentity, PointerButton, Pos2, SurfaceFormat,
    SurfaceSpec, SurfaceSupport, Vec2, encode_frame,
};

fn session() -> HostSession {
    HostSession::new(
        "BE3",
        Some(SurfaceSpec {
            format: SurfaceFormat::Rgba8Unorm,
            render: true,
        }),
        crate::Preferences::default(),
    )
}

fn hello() -> Message {
    Message::Hello(Hello {
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

fn input(event: impl Into<TimedInput>) -> Message {
    batch(vec![event.into()], false)
}

fn timed(input: Input, time_micros: u64) -> TimedInput {
    TimedInput {
        input,
        time_micros: Some(time_micros),
    }
}

fn batch(events: Vec<TimedInput>, every_sample: bool) -> Message {
    Message::Input(InputBatch {
        screen: crate::ScreenId(7),
        events,
        every_sample,
    })
}

fn pen(x: f32, time_micros: u64) -> TimedInput {
    timed(
        Input::Pen {
            phase: crate::PenPhase::Move,
            pos: crate::Pos2::new(x, 0.0),
            pen: crate::Pen {
                pressure: 0.5,
                ..crate::Pen::default()
            },
        },
        time_micros,
    )
}

mod a_plugin_that_draws_nothing_is_granted_no_surface;
mod a_superseded_request_is_forgotten;
mod acceptance_carries_the_initial_preferences;
mod coalesced_zoom_gestures_multiply;
mod disconnect_fails_the_session;
mod every_sample_of_a_backed_up_stroke_is_kept;
mod input_merged_while_queued_keeps_its_order;
mod malformed_payload_fails_the_session;
mod repeated_start_and_shutdown_are_clean;
mod request_timeout_fails_the_session;
mod superseded_events_are_coalesced;
mod the_next_deadline_is_the_earliest_pending_one;
mod touch_moves_are_coalesced;
