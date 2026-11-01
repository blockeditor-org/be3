use super::*;

#[test]
fn the_levels_follow_what_the_backends_report() {
    let mut levels = MediaLevels::default();
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
    heard(&mut levels, MediaEvent::Audio(audio.clone()));
    heard(&mut levels, MediaEvent::Brightness(0.25));
    assert_eq!(
        levels,
        MediaLevels {
            output: audio.output,
            input: None,
            brightness: Some(0.25),
            outputs: vec![speakers.clone()],
            default_output: Some(speakers.id.clone()),
        }
    );

    let headphones = AudioOutput {
        id: "headphones".to_owned(),
        name: "Headphones".to_owned(),
    };
    heard(
        &mut levels,
        MediaEvent::Audio(AudioState {
            outputs: vec![speakers, headphones.clone()],
            default_output: Some(headphones.id.clone()),
            ..audio
        }),
    );
    assert_eq!(
        levels.default_output,
        Some(headphones.id),
        "a new output and a new default are reported"
    );
    assert_eq!(levels.brightness, Some(0.25), "the brightness is kept");
}
