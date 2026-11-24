use super::*;

#[test]
fn the_media_type_follows_the_file_name() {
    let media_type = |name: &str| AudioFile::header(name, &[1]).media_type;
    assert_eq!(media_type("song.wav"), "audio/wav");
    assert_eq!(media_type("song.OGA"), "audio/ogg");
    assert_eq!(media_type("song.m4a"), "audio/mp4");
    assert_eq!(media_type("song"), "audio/mpeg");
}
