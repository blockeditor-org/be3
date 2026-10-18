use super::*;

#[test]
fn initial_and_live_motion_preferences_reach_the_plugin() {
    let mut plugin = HeadlessPlugin::new("motion", "Motion", "1");
    let accept = Message::HelloAccepted(HelloAccepted {
        host_name: "test host".to_owned(),
        surface: None,
        theme: Theme::Dark,
        motion: Motion::Instant,
    });
    plugin.receive(decode_frame(&encode_frame(&accept).unwrap()).unwrap());
    assert_eq!(motion(), Motion::Instant);
    let changed = Message::Motion(Motion::Animated);
    plugin.receive(decode_frame(&encode_frame(&changed).unwrap()).unwrap());
    assert_eq!(motion(), Motion::Animated);
}
