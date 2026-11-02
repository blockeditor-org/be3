use super::*;
use crate::headless::HeadlessApp;
use be_sdk_api::{
    HelloAccepted,
    embed::host::Call,
    ops::{Connection, Incoming, Side},
};

fn accepted(app: &mut HeadlessApp) {
    app
        .accept(HelloAccepted {
            host_name: "test host".into(),
            surface: None,
            preferences: be_sdk_api::Preferences::default(),
        })
        .unwrap();
}

fn asked(app: &mut HeadlessApp) -> Vec<Vec<char>> {
    app.outbound();
    let mut host = Connection::new(Side::Host);
    for frame in app.take_frames() {
        host.receive(&frame).unwrap();
    }
    let mut asked = Vec::new();
    while let Some(event) = host.next_event() {
        if let Incoming::Started(started) = event
            && let Some(Call::MissingCharacters { reply, characters }) =
                Call::accept(&mut host, started)
        {
            reply.send(&mut host, &());
            asked.push(characters);
        }
    }
    asked
}

mod a_character_is_asked_for_once_and_only_after_the_hosts_fonts_arrive;
mod fonts_from_the_host_reach_the_app_and_missing_characters_go_back;
