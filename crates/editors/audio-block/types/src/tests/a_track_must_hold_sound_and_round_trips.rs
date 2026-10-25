use super::*;

#[test]
fn a_track_must_hold_sound_and_round_trips() {
    assert!(AudioFile::from_file("silence.wav", "audio/wav", Vec::new()).is_err());

    let track = AudioFile::from_file("song.flac", "audio/flac", vec![1, 2, 3]).unwrap();
    let read = AudioContent::decode(&track.encode()).unwrap();
    assert_eq!(read, track);
    assert_eq!(read.header().media_type, "audio/flac");
    assert_eq!(BlockContent::name(&read).as_deref(), Some("song.flac"));
}
