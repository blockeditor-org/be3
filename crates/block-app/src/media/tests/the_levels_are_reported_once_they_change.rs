use super::*;

#[test]
fn the_levels_are_reported_once_they_change() {
    let (mut media, _recorder, events) = recorded();
    assert_eq!(
        media.frame(),
        None,
        "nothing is reported before a backend speaks"
    );

    let speakers = AudioOutput {
        id: "speakers".to_owned(),
        name: "Speakers".to_owned(),
    };
    let audio = AudioState {
        output: Some(MediaLevel {
            level: 0.4,
            muted: false,
        }),
        input: None,
        outputs: vec![speakers.clone()],
        default_output: Some(speakers.id.clone()),
    };
    events.send(MediaEvent::Audio(audio.clone())).unwrap();
    events.send(MediaEvent::Brightness(0.25)).unwrap();
    assert_eq!(
        media.frame(),
        Some(MediaLevels {
            output: audio.output,
            input: None,
            brightness: Some(0.25),
            outputs: vec![speakers.clone()],
            default_output: Some(speakers.id.clone()),
        })
    );

    events.send(MediaEvent::Audio(audio.clone())).unwrap();
    assert_eq!(
        media.frame(),
        None,
        "levels that did not change are not sent again"
    );

    let headphones = AudioOutput {
        id: "headphones".to_owned(),
        name: "Headphones".to_owned(),
    };
    events
        .send(MediaEvent::Audio(AudioState {
            outputs: vec![speakers, headphones.clone()],
            default_output: Some(headphones.id.clone()),
            ..audio
        }))
        .unwrap();
    assert_eq!(
        media.frame().and_then(|levels| levels.default_output),
        Some(headphones.id),
        "a new output and a new default are reported"
    );
}
