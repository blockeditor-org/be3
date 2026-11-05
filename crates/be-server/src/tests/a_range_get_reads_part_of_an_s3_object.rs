use super::{
    an_object_uploads_and_downloads_with_presigned_s3_urls::{EXPIRES, endpoint, send, upload},
    *,
};
use crate::s3::presign_get;

#[tokio::test]
async fn a_range_get_reads_part_of_an_s3_object() {
    let harness = Harness::start().await;
    let (bucket, credentials) = endpoint(&harness);
    let bytes: Vec<u8> = (0..=255).collect();
    assert_eq!(upload(&bucket, &credentials, &bytes).await.status, 200);
    let url = presign_get(&bucket, &credentials, Hash::of(&bytes), EXPIRES);

    let part = send(
        "GET",
        url.clone(),
        vec![("range", "bytes=10-19".to_owned())],
        None,
    )
    .await;
    assert_eq!(part.status, 206);
    assert_eq!(part.body, bytes[10..20]);
    assert_eq!(part.headers["content-range"], "bytes 10-19/256");

    let tail = send(
        "GET",
        url.clone(),
        vec![("range", "bytes=-6".to_owned())],
        None,
    )
    .await;
    assert_eq!(tail.status, 206);
    assert_eq!(tail.body, bytes[250..]);

    let past = send("GET", url, vec![("range", "bytes=300-".to_owned())], None).await;
    assert_eq!(past.status, 416);
}
