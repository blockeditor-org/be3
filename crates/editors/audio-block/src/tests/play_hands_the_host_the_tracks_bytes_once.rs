use super::*;

use audio_block_types::AudioFile;
use block_editor_beui::{AudioCommand, AudioSource};

fn played(test: &mut BeuiTest<AudioApp>) -> Vec<AudioCommand> {
    test.take_audio_commands()
}

#[test]
fn play_hands_the_host_the_tracks_bytes_once() {
    let (mut test, _) = editor();
    played(&mut test);

    test.click("audio.play");
    test.run();
    assert_eq!(
        played(&mut test),
        [
            AudioCommand::Load(AudioSource {
                media_type: "audio/flac".into(),
                bytes: vec![1, 2, 3],
            }),
            AudioCommand::Toggle,
        ]
    );

    test.click("audio.play");
    test.run();
    assert_eq!(played(&mut test), [AudioCommand::Toggle]);

    let replaced = AudioFile::from_file("other.wav", "audio/wav", vec![4, 5]).unwrap();
    test.hold(None, replaced.clone());
    test.run();
    assert_eq!(played(&mut test), [AudioCommand::Reset]);

    test.click("audio.play");
    test.run();
    assert_eq!(
        played(&mut test),
        [
            AudioCommand::Load(AudioSource {
                media_type: "audio/wav".into(),
                bytes: replaced.data().to_vec(),
            }),
            AudioCommand::Toggle,
        ]
    );
}
