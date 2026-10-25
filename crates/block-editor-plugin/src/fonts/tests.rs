use super::*;
use crate::headless::HeadlessPlugin;
use block_plugin_api::{
    HelloAccepted,
    embed::host::Call,
    ops::{Connection, Incoming, Side},
};

fn accepted(plugin: &mut HeadlessPlugin) {
    plugin
        .accept(HelloAccepted {
            host_name: "test host".into(),
            surface: None,
            preferences: block_plugin_api::Preferences::default(),
        })
        .unwrap();
}

/// The characters the plugin asked the host for since the last call.
fn asked(plugin: &mut HeadlessPlugin) -> Vec<Vec<char>> {
    plugin.outbound();
    let mut host = Connection::new(Side::Host);
    for frame in plugin.take_frames() {
        host.receive(&frame).unwrap();
    }
    std::iter::from_fn(|| host.next_event())
        .filter_map(|event| match event {
            Incoming::Called(called) => match Call::accept_direct(&called) {
                Some(Call::MissingCharacters { characters }) => Some(characters),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

mod a_character_is_asked_for_once_and_only_after_the_hosts_fonts_arrive;
mod fonts_from_the_host_reach_the_plugin_and_missing_characters_go_back;
