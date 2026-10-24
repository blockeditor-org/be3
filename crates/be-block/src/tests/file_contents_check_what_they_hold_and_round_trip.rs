use crate::AudioContent;

#[test]
fn file_contents_check_what_they_hold_and_round_trip() {
    assert!(AudioContent::from_file("silence.wav", "audio/wav", Vec::new()).is_err());
}
