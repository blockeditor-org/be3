use rusty_s3::S3Action;

use super::{
    an_object_uploads_and_downloads_with_presigned_s3_urls::{
        EXPIRES, endpoint, refused_with, send,
    },
    *,
};
use crate::s3::{checksum, presign_put};

#[tokio::test]
async fn an_s3_upload_that_is_not_its_key_stores_nothing() {
    let harness = Harness::start().await;
    let (bucket, credentials) = endpoint(&harness);
    let named = b"the object the key names".to_vec();
    let other = b"another object, same len".to_vec();
    assert_eq!(named.len(), other.len());
    let hash = Hash::of(&named);

    // A body other than the one the key and checksum name.
    let url = presign_put(&bucket, &credentials, hash, other.len(), EXPIRES);
    let wrong_body = send(
        "PUT",
        url,
        vec![("x-amz-checksum-sha256", checksum(hash))],
        Some(other.clone()),
    )
    .await;
    assert_eq!(wrong_body.status, 400);
    assert!(refused_with(&wrong_body, "BadDigest"));

    // A checksum, signed, that is not the key's.
    let key = hash.to_hex();
    let length = named.len().to_string();
    let wrong_checksum = checksum(Hash::of(&other));
    let mut action = bucket.put_object(Some(&credentials), &key);
    action
        .headers_mut()
        .insert("content-length", length.as_str());
    action
        .headers_mut()
        .insert("x-amz-checksum-sha256", wrong_checksum.as_str());
    let mismatched = send(
        "PUT",
        action.sign(EXPIRES),
        vec![("x-amz-checksum-sha256", wrong_checksum.clone())],
        Some(other.clone()),
    )
    .await;
    assert_eq!(mismatched.status, 400);
    assert!(refused_with(&mismatched, "BadDigest"));

    // No checksum at all.
    let unchecked = send(
        "PUT",
        bucket.put_object(Some(&credentials), &key).sign(EXPIRES),
        Vec::new(),
        Some(named.clone()),
    )
    .await;
    assert_eq!(unchecked.status, 400);
    assert!(refused_with(&unchecked, "InvalidRequest"));

    // A key that is not a hash.
    let mut action = bucket.put_object(Some(&credentials), "notes.txt");
    let checksum_of_named = checksum(hash);
    action
        .headers_mut()
        .insert("x-amz-checksum-sha256", checksum_of_named.as_str());
    let unnamed = send(
        "PUT",
        action.sign(EXPIRES),
        vec![("x-amz-checksum-sha256", checksum_of_named.clone())],
        Some(named.clone()),
    )
    .await;
    assert_eq!(unnamed.status, 400);
    assert!(refused_with(&unnamed, "InvalidArgument"));

    let store = ServerStore::open(&harness.directory).unwrap();
    assert!(!store.objects().has(hash).unwrap());
    assert!(!store.objects().has(Hash::of(&other)).unwrap());
}
