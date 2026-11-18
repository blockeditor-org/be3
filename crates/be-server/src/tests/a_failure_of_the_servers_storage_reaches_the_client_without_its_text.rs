use super::*;

/// A call the server fails on its own storage is refused with the code that tells the client to
/// try again, and none of the storage's text, which may name its tables, paths or backend; a
/// refusal keeps its reason.
#[test]
fn a_failure_of_the_servers_storage_reaches_the_client_without_its_text() {
    let failures = [
        ServerError::Database(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT),
            Some("database disk image is malformed in table accounts".into()),
        )),
        ServerError::Io(std::io::Error::other("/srv/be-server/data/objects: no space")),
        ServerError::Corrupt,
    ];
    for failure in failures {
        let told = crate::client::wire(failure);
        assert_eq!(told.code, ErrorCode::Storage);
        assert_eq!(told.message, "the request failed");
    }
    let refused = crate::client::wire(ServerError::Refused(
        ErrorCode::PermissionDenied,
        "this account may not read the block".into(),
    ));
    assert_eq!(refused.code, ErrorCode::PermissionDenied);
    assert_eq!(refused.message, "this account may not read the block");
}
