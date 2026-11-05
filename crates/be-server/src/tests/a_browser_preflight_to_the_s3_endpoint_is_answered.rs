use super::{an_object_uploads_and_downloads_with_presigned_s3_urls::send, *};

#[tokio::test]
async fn a_browser_preflight_to_the_s3_endpoint_is_answered() {
    let harness = Harness::start().await;
    let url = format!(
        "{}/api/be/s3/objects/{}",
        harness.url,
        Hash::of(b"to upload").to_hex()
    )
    .parse()
    .unwrap();
    let preflight = send(
        "OPTIONS",
        url,
        vec![
            ("origin", "https://be3.pfg.pw".to_owned()),
            ("access-control-request-method", "PUT".to_owned()),
            (
                "access-control-request-headers",
                "x-amz-checksum-sha256".to_owned(),
            ),
        ],
        None,
    )
    .await;
    assert_eq!(preflight.status, 204);
    assert_eq!(preflight.headers["access-control-allow-origin"], "*");
    assert!(preflight.headers["access-control-allow-methods"].contains("PUT"));
    assert!(preflight.headers["access-control-allow-headers"].contains("x-amz-checksum-sha256"));
    assert_eq!(
        preflight.headers["cross-origin-resource-policy"],
        "cross-origin"
    );
}
