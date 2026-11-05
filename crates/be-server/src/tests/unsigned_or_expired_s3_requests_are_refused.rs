use rusty_s3::{Credentials, S3Action};

use super::{
    an_object_uploads_and_downloads_with_presigned_s3_urls::{EXPIRES, endpoint, refused_with, send, upload},
    *,
};
use crate::s3::presign_get;

#[tokio::test]
async fn unsigned_or_expired_s3_requests_are_refused() {
    let harness = Harness::start().await;
    let (bucket, credentials) = endpoint(&harness);
    let bytes = b"kept private".to_vec();
    let hash = Hash::of(&bytes);
    assert_eq!(upload(&bucket, &credentials, &bytes).await.status, 200);

    let unsigned = send("GET", bucket.object_url(&hash.to_hex()), Vec::new(), None).await;
    assert_eq!(unsigned.status, 403);

    let an_hour_ago = jiff::Timestamp::now() - jiff::SignedDuration::from_hours(1);
    let key = hash.to_hex();
    let expired = bucket
        .get_object(Some(&credentials), &key)
        .sign_with_time(std::time::Duration::from_secs(60), &an_hour_ago);
    let expired = send("GET", expired, Vec::new(), None).await;
    assert_eq!(expired.status, 403);
    assert!(String::from_utf8_lossy(&expired.body).contains("expired"));

    let stranger = Credentials::new(credentials.key(), "not the server's secret");
    let forged = presign_get(&bucket, &stranger, hash, EXPIRES);
    let forged = send("GET", forged, Vec::new(), None).await;
    assert_eq!(forged.status, 403);
    assert!(refused_with(&forged, "SignatureDoesNotMatch"));

    // Nothing is listable, even signed.
    let listing = bucket.list_objects_v2(Some(&credentials)).sign(EXPIRES);
    let listed = send("GET", listing, Vec::new(), None).await;
    assert_ne!(listed.status, 200);
    assert!(!String::from_utf8_lossy(&listed.body).contains(&key));
}
