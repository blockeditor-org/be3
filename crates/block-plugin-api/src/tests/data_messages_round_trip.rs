use super::*;

#[test]
fn data_messages_round_trip() {
    for message in [
        request(2, 9, HostRequest::ListData),
        request(2, 10, HostRequest::ReadData("games/chess.wasm".into())),
        reply(
            2,
            9,
            HostReply::DataListed(DataListing::Files(vec![
                "games/chess.wasm".into(),
                "games/tic_tac_toe.wasm".into(),
            ])),
        ),
        reply(
            2,
            9,
            HostReply::DataListed(DataListing::Failed("no data was staged".into())),
        ),
        reply(
            2,
            10,
            HostReply::DataRead(FetchResult::Body(vec![0, 97, 115, 109])),
        ),
        reply(
            2,
            11,
            HostReply::DataRead(FetchResult::Failed("no such file".into())),
        ),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
