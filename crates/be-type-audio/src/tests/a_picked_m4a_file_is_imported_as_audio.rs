use super::*;
use be_sdk_type_guest::FileForm;

/// "Import a file…" offers a file to the types that claim it (`FileForm::claims`): an .m4a,
/// which a file picker for audio accepts too, makes an audio block.
#[test]
fn a_picked_m4a_file_is_imported_as_audio() {
    assert!(AudioFile::FILE_EXTENSIONS.contains(&"m4a"));
    assert!(<AudioContent as FileForm>::claims(
        "voice memo.m4a",
        &[0, 0, 0, 32]
    ));
    let imported =
        <AudioContent as FileForm>::from_file(None, "voice memo.m4a", vec![0, 0, 0, 32], 1)
            .expect("an audio file imports");
    assert_eq!(imported.header().media_type, "audio/mp4");
    assert_eq!(imported.data(), [0, 0, 0, 32]);
}
