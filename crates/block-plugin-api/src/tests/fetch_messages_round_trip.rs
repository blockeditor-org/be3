use super::*;

#[test]
fn fetch_messages_round_trip() {
    for message in [
        request(
            2,
            9,
            HostRequest::Fetch(
                "https://api.github.com/repos/pfgithub/be3/git/trees/dev:snapshots".into(),
            ),
        ),
        reply(
            2,
            9,
            HostReply::Fetched(FetchResult::Body(vec![4; MAX_STRING_BYTES + 1])),
        ),
        reply(
            2,
            10,
            HostReply::Fetched(FetchResult::Failed("api.github.com answered 404".into())),
        ),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
