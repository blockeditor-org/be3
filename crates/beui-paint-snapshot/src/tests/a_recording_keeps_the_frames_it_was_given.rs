use super::*;

#[test]
fn a_recording_keeps_the_frames_it_was_given() {
    let red = [255, 0, 0, 255];
    let blue = [0, 0, 255, 255];
    let mut recording = triangle(red);
    let alone = triangle(blue);
    recording.append(alone.clone());

    assert_eq!(recording.frames.len(), 2);
    assert_eq!(recording.frames[1], alone.frames[0]);
    for (key, texture) in &alone.textures {
        assert_eq!(recording.textures.get(key), Some(texture));
    }
    assert_eq!(recording.textures.len(), 1);

    let bytes = recording.encode().unwrap();
    assert_eq!(Snapshot::decode(&bytes).unwrap(), recording);
}
