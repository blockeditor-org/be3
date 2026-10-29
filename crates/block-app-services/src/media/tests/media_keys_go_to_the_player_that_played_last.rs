use super::*;
use players::Tracker;

#[test]
fn media_keys_go_to_the_player_that_played_last() {
    let mut players = Tracker::default();
    assert_eq!(
        players.target(),
        None,
        "with no player there is nobody to tell"
    );

    players.appeared("org.mpris.MediaPlayer2.vlc", ":1.5");
    players.appeared("org.mpris.MediaPlayer2.firefox.instance_1_42", ":1.9");
    assert_eq!(
        players.target(),
        Some(":1.9"),
        "before any player plays, the first one is told"
    );

    players.status(":1.5", "Playing");
    players.status(":1.9", "Paused");
    assert_eq!(
        players.target(),
        Some(":1.5"),
        "the player that played last is told"
    );

    players.status(":1.7", "Playing");
    assert_eq!(
        players.target(),
        Some(":1.5"),
        "a name that is no player is ignored"
    );

    players.vanished("org.mpris.MediaPlayer2.vlc");
    assert_eq!(
        players.target(),
        Some(":1.9"),
        "once it is gone the first one left is told"
    );
}
