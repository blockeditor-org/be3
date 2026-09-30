use super::*;

#[test]
fn file_save_messages_round_trip() {
    for message in [
        request(
            7,
            3,
            HostRequest::SaveFile(SavedFile {
                name: "notes.md".into(),
                mime_type: "text/markdown".into(),
                data: vec![7; MAX_STRING_BYTES + 1],
            }),
        ),
        reply(7, 3, HostReply::FileSaved(FileSave::Saved)),
        reply(7, 4, HostReply::FileSaved(FileSave::Cancelled)),
        reply(
            7,
            5,
            HostReply::FileSaved(FileSave::Failed("Could not write notes.md".into())),
        ),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
