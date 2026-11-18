use super::*;

/// The secrets the server derives from its own (the S3 endpoint's keys) are HKDF-SHA256 of it,
/// one purpose each, as every other key derivation in be3 is.
#[tokio::test]
async fn a_secret_derived_from_the_servers_own_is_hkdf_of_it() {
    let harness = Harness::start().await;
    harness.member("ada@example.com").await;
    let database = metadata_database(&harness.directory);
    let secret: Vec<u8> = database
        .query_row("SELECT secret FROM server_account WHERE id = 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    let key = crate::organization::ServerKey::load_or_create(&database).unwrap();
    for purpose in ["s3 access key", "s3 secret key"] {
        let mut expected = [0u8; 32];
        hkdf::Hkdf::<sha2::Sha256>::new(Some(b"be3 server derived"), &secret)
            .expand(purpose.as_bytes(), &mut expected)
            .unwrap();
        assert_eq!(key.derived(purpose), expected, "{purpose}");
    }
    assert_ne!(key.derived("s3 access key"), key.derived("s3 secret key"));
}
