use super::*;

#[test]
fn file_pick_messages_round_trip() {
    for message in [
        request(
            7,
            3,
            HostRequest::PickFile(FileFilter {
                name: "Images".into(),
                default_file_name: "Image".into(),
                extensions: vec!["png".into(), "jpg".into()],
                mime_types: vec!["image/*".into()],
            }),
        ),
        reply(
            7,
            3,
            HostReply::FilePicked(FilePick::Chosen {
                name: "photo.png".into(),
                data: vec![7; MAX_STRING_BYTES + 1],
            }),
        ),
        reply(7, 4, HostReply::FilePicked(FilePick::Cancelled)),
        reply(
            7,
            5,
            HostReply::FilePicked(FilePick::Failed("Could not read photo.png".into())),
        ),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
